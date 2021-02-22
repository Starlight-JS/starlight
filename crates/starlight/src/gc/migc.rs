use libmimalloc_sys::*;
use std::{
    collections::VecDeque,
    mem::size_of,
    ptr::{null_mut, NonNull},
};

use hashbrown::HashSet;

use crate::heap::addr::Address;

use super::{
    cell::{object_ty_of, Cell, Gc, Header, Tracer, GC_MARKED, GC_UNMARKED},
    constraint::MarkingConstraint,
    handle::HandleTrait,
};

pub struct Heap {
    constraints: Vec<Box<dyn MarkingConstraint>>,
    heap: *mut mi_heap_t,
    ndefers: u32,
    max_heap_size: usize,
    list: *mut Header,
    allocated: usize,
    pub(super) handles: HashSet<*mut dyn HandleTrait>,
}

#[cfg(not(any(target_os = "windows", feature = "valgrind-gc")))]
impl Heap {
    pub fn new() -> Box<Self> {
        let mut this = unsafe {
            Box::new(Self {
                list: null_mut(),
                constraints: vec![],
                handles: Default::default(),

                ndefers: 0,
                max_heap_size: 64 * 1024,
                allocated: 0,
                heap: mi_heap_new(),
            })
        };
        this.add_core_constraints();
        this.init_arenas();
        this
    }
    fn init_arenas(&mut self) {
        /*    for i in 0..SIZE_CLASSES.len() {
            self.arenas[i] = Box::into_raw(Box::new(SmallArena::new(SIZE_CLASSES[i])));
        }*/
    }
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn test_and_set_marked(cell: *mut Header) -> bool {
        unsafe {
            if (*cell).tag() == GC_UNMARKED {
                (*cell).set_tag(GC_MARKED);
                true
            } else {
                false
            }
        }
    }

    pub fn add_constraint(&mut self, x: impl MarkingConstraint + 'static) {
        self.constraints.push(Box::new(x));
    }
    fn add_core_constraints(&mut self) {
        /* // we do not want to mark stack when running MIRI.
        #[cfg(not(miri))]
        self.add_constraint(SimpleMarkingConstraint::new(
            "Conservative Roots",
            |marking| {
                let origin = marking.gc.stack_bounds.origin;
                marking.add_conservative_roots(origin, marking.gc.sp as _);
            },
        ));*/
    }

    unsafe fn gc_internal(&mut self, dummy: *const usize) {
        if self.ndefers > 0 {
            return;
        }

        let mut task = Marking {
            gc: self,
            bytes_visited: 0,
            worklist: VecDeque::with_capacity(8),

            file: None,
        };

        task.run();

        let visited = task.bytes_visited;
        drop(task);

        self.allocated = 0;

        let mut prev = null_mut();
        let mut cur = self.list;
        self.allocated = 0;
        while !cur.is_null() {
            let sz = mi_usable_size(cur.cast());
            if (*cur).tag() == GC_MARKED {
                prev = cur;
                cur = (*cur).next;
                (*prev).set_tag(GC_UNMARKED);
                self.allocated += sz;
            } else {
                let unreached = cur;
                cur = (*cur).next;
                if !prev.is_null() {
                    (*prev).next = cur;
                } else {
                    self.list = cur;
                }
                std::ptr::drop_in_place((*unreached).get_dyn());
                mi_free(unreached.cast());
                //dealloc(unreached.cast(), Layout::from_size_align_unchecked(sz, 16));
            }
        }
        if self.allocated >= self.max_heap_size {
            self.max_heap_size = (self.allocated as f64 * 1.7) as usize;
        }

        self.allocated = visited;
        self.max_heap_size = (visited as f64 * 1.7) as usize;
    }

    pub fn gc(&mut self) {
        let x = 0;

        unsafe {
            self.gc_internal(&x);
        }
    }

    pub fn collect_if_necessary(&mut self) {
        if self.allocated <= self.max_heap_size {
            return;
        }
        self.gc();
    }
    pub fn defer_gc(&mut self) {
        self.ndefers += 1;
    }
    pub fn undefer_gc(&mut self) {
        self.ndefers -= 1;
    }
    #[inline(never)]
    unsafe fn alloc_slow(&mut self, size: usize) -> Address {
        let p = mi_malloc(size);
        let sz = mi_usable_size(p);
        self.allocated += sz;
        Address::from_ptr(p)
    }

    /// Allocate `size` bytes in GC heap.
    ///
    /// # Safety
    ///
    /// This function is unsafe since it returns partially initialized data.
    /// Only first 8 bytes is initialized with GC object header.
    ///
    ///
    #[inline]
    pub unsafe fn allocate_raw(&mut self, size: usize) -> Address {
        self.collect_if_necessary();
        if size < MI_SMALL_SIZE_MAX {
            let p = Address::from_ptr(mi_heap_malloc_small(self.heap, size));
            let sz = mi_usable_size(p.to_mut_ptr());
            self.allocated += sz;
            p
        } else {
            self.alloc_slow(size)
        }
    }
    pub fn heap_usage(&self) -> usize {
        self.allocated
    }

    pub fn alloc<T: Cell>(&mut self, value: T) -> Gc<T> {
        unsafe {
            fn allocation_size<T: Cell>(val: &T) -> usize {
                /// Align address upwards.
                ///
                /// Returns the smallest x with alignment `align` so that x >= addr.
                /// The alignment must be a power of 2.
                pub fn align_up(addr: u64, align: u64) -> u64 {
                    assert!(align.is_power_of_two(), "`align` must be a power of two");
                    let align_mask = align - 1;
                    if addr & align_mask == 0 {
                        addr // already aligned
                    } else {
                        (addr | align_mask) + 1
                    }
                }
                align_up(val.compute_size() as u64 + size_of::<Header>() as u64, 16) as usize
                // round_up_to_multiple_of(16, val.compute_size() + size_of::<Header>())
            }
            let size = allocation_size(&value);
            let memory = self.allocate_raw(size).to_mut_ptr::<Header>();

            memory.write(Header::new(self, null_mut(), object_ty_of(&value)));
            (*memory).set_tag(GC_UNMARKED);
            let sz = value.compute_size();
            (*memory).data_start().to_mut_ptr::<T>().write(value);
            (*memory).set_next(self.list);
            self.list = memory;
            Gc {
                cell: NonNull::new_unchecked(memory),
                marker: Default::default(),
            }
        }
    }
}
pub struct Marking<'a> {
    pub gc: &'a mut Heap,
    pub worklist: VecDeque<*mut Header>,
    pub bytes_visited: usize,
    #[allow(dead_code)]
    file: Option<&'a mut std::fs::File>,
}

impl<'a> Marking<'a> {
    pub fn run(&mut self) {
        self.process_constraints();
        self.process_roots();
        self.process_worklist();
    }
    fn process_constraints(&mut self) {
        unsafe {
            let mut constraints = vec![];
            std::mem::swap(&mut constraints, &mut self.gc.constraints);
            for c in constraints.iter_mut() {
                c.execute(self);
            }
            std::mem::swap(&mut constraints, &mut self.gc.constraints);
        }
    }
    fn process_roots(&mut self) {
        unsafe {
            let this = self as *mut Self;
            for handle in self.gc.handles.iter().copied() {
                (*handle).trace(&mut *this);
            }
        }
    }
    fn process_worklist(&mut self) {
        while let Some(item) = self.worklist.pop_front() {
            unsafe {
                self.visit_value(item);
            }
        }
    }
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn mark(&mut self, val: *mut Header) {
        unsafe {
            if Heap::test_and_set_marked(val) {
                let obj = val;

                self.worklist.push_back(obj);
            }
        }
    }

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn visit_value(&mut self, val: *mut Header) {
        unsafe {
            (*val).get_dyn().trace(self);
        }
    }
}
impl<'a> Tracer for Marking<'a> {
    fn trace(&mut self, hdr: *mut Header) {
        self.mark(hdr);
    }
}
unsafe extern "C" fn visit_block(
    heap: *const mi_heap_t,
    area: *const mi_heap_area_t,
    block: *mut libc::c_void,
    block_size: usize,
    arg: *mut libc::c_void,
) -> bool {
    if block.is_null() {
        return true;
    }
    let heap = arg as *mut Heap;
    let heap = &mut *heap;
    let cell = block as *mut Header;
    let sz = mi_usable_size(cell.cast());

    if (*cell).tag() == GC_UNMARKED {
        mi_free(cell.cast());
    } else {
        (*cell).set_tag(GC_UNMARKED);
        heap.allocated += sz;
    }
    true
}

unsafe extern "C" fn visit_fin_block(
    heap: *const mi_heap_t,
    area: *const mi_heap_area_t,
    block: *mut libc::c_void,
    block_size: usize,
    arg: *mut libc::c_void,
) -> bool {
    if block.is_null() {
        return true;
    }
    let heap = arg as *mut Heap;
    let heap = &mut *heap;
    let cell = block as *mut Header;
    std::ptr::drop_in_place((*cell).get_dyn());
    true
}
impl Drop for Heap {
    fn drop(&mut self) {
        unsafe {
            mi_heap_visit_blocks(
                self.heap,
                true,
                Some(visit_fin_block),
                self as *mut Self as _,
            );
            self.constraints.clear();
            mi_heap_destroy(self.heap);
        }
    }
}

impl AsMut<Heap> for &mut Heap {
    fn as_mut(&mut self) -> &mut Heap {
        self
    }
}
