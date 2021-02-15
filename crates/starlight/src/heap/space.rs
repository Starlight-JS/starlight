use std::{
    collections::VecDeque,
    mem::{size_of, swap},
    ptr::{null_mut, NonNull},
};

use wtf_rs::{
    keep_on_stack, object_offsetof, segmented_vec::SegmentedVec, stack_bounds::StackBounds,
};

#[cfg(feature = "debug-snapshots")]
use super::freeze_cell_into;
use super::{
    addr::*,
    block::*,
    block_set::BlockSet,
    cell::*,
    constraint::{MarkingConstraint, SimpleMarkingConstraint},
    context::{LocalContext, LocalContextInner, PersistentContext},
    precise_allocation::PreciseAllocation,
};

use intrusive_collections::{intrusive_adapter, LinkedList, LinkedListLink, UnsafeRef};
/// SIZE_STEP is synonym for ATOM_SIZE.
pub const SIZE_STEP: usize = ATOM_SIZE;
/// Sizes up to this amount get a size class for each size step.
pub const PRECISE_CUTOFF: usize = 80;
/// The amount of available payload in a block is the block's size minus the footer.
pub const BLOCK_PAYLOAD: usize = PAYLOAD_SIZE;

/// The largest cell we're willing to allocate in a MarkedBlock the "normal way" (i.e. using size
/// classes, rather than a large allocation) is half the size of the payload, rounded down. This
/// ensures that we only use the size class approach if it means being able to pack two things
/// into one block.
pub const LARGE_CUTOFF: usize = (BLOCK_PAYLOAD / 2) & !(SIZE_STEP - 1);

/// We have an extra size class for size zero.
pub const NUM_SIZE_CLASSES: usize = LARGE_CUTOFF / SIZE_STEP + 1;
/// Converts size class to index
pub const fn size_class_to_index(size_class: usize) -> usize {
    (size_class + SIZE_STEP - 1) / SIZE_STEP
}
/// Converts index to size class
pub const fn index_to_size_class(index: usize) -> usize {
    index * SIZE_STEP
}
/// Return optimal allocation size
pub fn optimal_size_for(bytes: usize) -> usize {
    if bytes <= PRECISE_CUTOFF {
        round_up_to_multiple_of(SIZE_STEP, bytes)
    } else if bytes <= LARGE_CUTOFF {
        SIZE_CLASSES_FOR_SIZE_STEP[size_class_to_index(bytes)]
    } else {
        bytes
    }
}
const GC_LOG: bool = true;
/// Size classes for size step

pub static SIZE_CLASSES_FOR_SIZE_STEP: once_cell::sync::Lazy<[usize; NUM_SIZE_CLASSES]> =
    once_cell::sync::Lazy::new(|| {
        let mut result = [0; NUM_SIZE_CLASSES];
        build_size_class_table(&mut result, |x| x, |x| x);

        result
    });

/// All size classes
pub fn size_classes() -> Vec<usize> {
    let mut result = vec![];
    if false {
        println!("Block size: {}", BLOCK_SIZE);
        println!("Footer size: {}", FOOTER_SIZE);
    }

    let add = |vec: &mut Vec<usize>, size_class| {
        let size_class = round_up_to_multiple_of(ATOM_SIZE, size_class);
        if false {
            println!("--Adding MarkedSpace size class: {}", size_class);
        }
        vec.push(size_class);
    };

    let mut size = SIZE_STEP;
    while size < PRECISE_CUTOFF {
        add(&mut result, size);
        size += SIZE_STEP;
    }

    if false {
        println!("---Marked block payload size: {}", BLOCK_PAYLOAD);
    }

    for i in 0.. {
        let approximate_size = (PRECISE_CUTOFF as f64 * 1.4f64.powi(i)) as usize;

        if approximate_size > LARGE_CUTOFF {
            break;
        }
        let size_class = round_up_to_multiple_of(SIZE_STEP, approximate_size);
        if false {
            println!("---Size class: {}", size_class);
        }

        let cells_per_block = BLOCK_PAYLOAD / size_class;
        let possibly_better_size_class = (BLOCK_PAYLOAD / cells_per_block) & !(SIZE_STEP - 1);
        if false {
            println!(
                "---Possibly better size class: {}",
                possibly_better_size_class
            );
        }
        let original_wastage = BLOCK_PAYLOAD - cells_per_block * size_class;
        let new_wastage = (possibly_better_size_class - size_class) * cells_per_block;
        if false {
            println!(
                "---Original wastage: {}, new wastage: {}",
                original_wastage, new_wastage
            );
        }

        let better_size_class = if new_wastage > original_wastage {
            size_class
        } else {
            possibly_better_size_class
        };
        if false {
            println!("---Choosing size class: {}", better_size_class);
        }
        if better_size_class == *result.last().unwrap() {
            continue;
        }

        if better_size_class > LARGE_CUTOFF || better_size_class > 100000 {
            break;
        }
        add(&mut result, better_size_class);
    }
    add(&mut result, 256);
    result.sort_unstable();
    result.dedup();
    if false {
        println!("--Heap MarkedSpace size class dump: {:?}", result);
    }

    result
}

/// Build size class table
pub fn build_size_class_table(
    table: &mut [usize],
    cons: impl Fn(usize) -> usize,
    dcons: impl Fn(usize) -> usize,
) {
    let mut next_index = 0;
    for size_class in size_classes() {
        let entry = cons(size_class);
        let index = size_class_to_index(size_class);
        for i in next_index..=index {
            table[i] = entry;
        }
        next_index = index + 1;
    }
    for i in next_index..NUM_SIZE_CLASSES {
        table[i] = dcons(index_to_size_class(i));
    }
}
/// Directory of blocks of the same cell size
pub struct Directory {
    blocks: Vec<*mut BlockHeader>,
    cell_size: usize,
}
/// Allocator for single size class
pub struct LocalAllocator {
    link: LinkedListLink,
    directory: *mut Directory,
    current_block: *mut BlockHeader,
    unswept_cursor: usize,
}
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
enum CollectionScope {
    Full,
    Minor,
}

#[allow(clippy::vec_box)]
#[repr(C)]
pub struct Space {
    wbuf: WriteBarrierBuffer,
    allocator_for_size_step: [*mut LocalAllocator; NUM_SIZE_CLASSES],
    directories: Vec<Box<Directory>>,
    sp: usize,
    pub(crate) precise_allocations: Vec<*mut PreciseAllocation>,
    scopes: *mut LocalContextInner,
    persistent: *mut LocalContextInner,
    ndefers: u32,

    blocks: BlockSet,
    constraints: Vec<Box<dyn MarkingConstraint>>,

    collection_scope: Option<CollectionScope>,
    max_eden_size: usize,
    max_heap_size: usize,

    size_after_last_collect: usize,
    size_after_last_full_collect: usize,
    size_before_last_full_collect: usize,
    size_before_last_eden_collect: usize,
    size_after_last_eden_collect: usize,
    bytes_allocated_this_cycle: usize,
    should_do_full_collection: bool,
    total_bytes_visited_this_cycle: usize,
    total_bytes_visited: usize,

    /// Mark stack for write barrier
    mark_stack: SegmentedVec<*mut Header>,

    local_allocators: LinkedList<AllocLink>,
}
intrusive_adapter!(AllocLink = UnsafeRef<LocalAllocator> : LocalAllocator {
    link: LinkedListLink
});
impl Space {
    pub fn wbuf_offset() -> usize {
        object_offsetof!(Self, wbuf)
    }

    pub fn allocator_for_size_step_offset() -> usize {
        object_offsetof!(Self, allocator_for_size_step)
    }
    /// Create new GC instance
    pub fn new(write_barrier_buffer_size: usize) -> Box<Self> {
        let this = Self {
            ndefers: 0,
            sp: 0,
            allocator_for_size_step: [null_mut(); NUM_SIZE_CLASSES],
            directories: vec![],
            precise_allocations: vec![],
            local_allocators: LinkedList::new(AllocLink::new()),
            scopes: core::ptr::null_mut(),
            constraints: vec![],
            persistent: Box::into_raw(Box::new(LocalContextInner {
                next: null_mut(),
                prev: null_mut(),
                space: null_mut(),
                roots: Default::default(),
            })),
            wbuf: WriteBarrierBuffer::new(write_barrier_buffer_size),
            blocks: BlockSet::new(),
            collection_scope: None,
            should_do_full_collection: false,
            max_eden_size: 8 * 1024,
            max_heap_size: 100 * 1024,
            bytes_allocated_this_cycle: 0,
            size_after_last_collect: 0,
            size_after_last_eden_collect: 0,
            size_after_last_full_collect: 0,
            size_before_last_eden_collect: 0,
            size_before_last_full_collect: 0,

            total_bytes_visited: 0,
            total_bytes_visited_this_cycle: 0,
            mark_stack: SegmentedVec::with_chunk_size(32),
        };
        let mut this = Box::new(this);
        this.add_core_constraints();
        unsafe {
            (*this.persistent).space = &mut *this;
        }
        this
    }
    pub fn add_constraint(&mut self, x: impl MarkingConstraint + 'static) {
        self.constraints.push(Box::new(x));
    }
    fn add_core_constraints(&mut self) {
        // we do not want to mark stack when running MIRI.
        #[cfg(not(miri))]
        self.add_constraint(SimpleMarkingConstraint::new(
            "Conservative Roots",
            |marking| {
                let bounds = StackBounds::current_thread_stack_bounds();
                marking.add_conservative_roots(bounds.origin, marking.gc.sp as _);
            },
        ));
    }

    pub fn persistent_context(&self) -> PersistentContext {
        unsafe {
            PersistentContext {
                inner: &mut *self.persistent,
            }
        }
    }

    pub fn new_local_context<'a>(&mut self) -> LocalContext<'a> {
        let scope = Box::into_raw(Box::new(LocalContextInner {
            prev: core::ptr::null_mut(),
            next: self.scopes,
            space: self as *mut Self,
            roots: wtf_rs::list::LinkedList::with_capacity(1),
        }));
        if !self.scopes.is_null() {
            unsafe {
                (*self.scopes).prev = scope;
            }
        }
        unsafe {
            self.scopes = scope;
            LocalContext {
                inner: NonNull::new_unchecked(scope),
                marker: Default::default(),
            }
        }
    }
    fn allocator_for(&mut self, size: usize) -> Option<*mut LocalAllocator> {
        if size <= LARGE_CUTOFF {
            let index = size_class_to_index(size);
            let alloc = self.allocator_for_size_step.get(index);
            if let Some(alloc) = alloc {
                if !alloc.is_null() {
                    Some(*alloc)
                } else {
                    self.allocator_for_slow(size)
                }
            } else {
                self.allocator_for_slow(size)
            }
        } else {
            None
        }
    }
    fn allocator_for_slow(&mut self, size: usize) -> Option<*mut LocalAllocator> {
        let index = size_class_to_index(size);
        let size_class = SIZE_CLASSES_FOR_SIZE_STEP.get(index).copied();
        let size_class = size_class?;
        let alloc = self.allocator_for_size_step[index];
        if !alloc.is_null() {
            return Some(alloc);
        }
        if GC_LOG {
            eprintln!(
                "Creating BlockDirectory/LocalAllocator for size class: {}",
                size_class
            );
        }

        let mut directory = Box::new(Directory {
            cell_size: size_class,
            blocks: Vec::new(),
        });
        let raw = &mut *directory as *mut Directory;
        let local = LocalAllocator {
            directory: raw,
            link: LinkedListLink::new(),
            unswept_cursor: 0,
            current_block: null_mut(),
        };
        self.directories.push(directory);
        self.local_allocators
            .push_back(UnsafeRef::from_box(Box::new(local)));
        let last =
            self.local_allocators.back_mut().get().unwrap() as *const LocalAllocator as *mut _;
        self.allocator_for_size_step[index] = last;

        Some(last)
    }
    #[inline]
    /// Allocate raw memory of `size` bytes.
    ///
    ///
    /// # Safety
    ///
    /// Unsafe because it allocates raw uninitialized memory
    ///
    pub unsafe fn allocate_raw(&mut self, size: usize) -> Address {
        self.collect_if_necessary();
        // this will be executed always if size <= LARGE_CUTOFF
        if let Some(alloc) = self.allocator_for(size) {
            let res = (&mut *alloc).allocate(self);
            //self.bytes_allocated += size;

            return res;
        }

        // should not be executed if size > LARGE_CUTOFF
        let res = self.allocate_slow(size);
        self.bytes_allocated_this_cycle += size;

        res
    }

    /// Allocate raw memory of `size` bytes.
    ///
    /// # Safety
    /// This function is unsafe because it could return null pointer.
    ///
    pub unsafe fn allocate_raw_no_gc(&mut self, size: usize) -> Address {
        // this will be executed always if size <= LARGE_CUTOFF
        if let Some(alloc) = self.allocator_for(size) {
            let res = (&mut *alloc).allocate(self);

            return res;
        }

        // should not be executed if size > LARGE_CUTOFF
        self.allocate_slow(size)
    }
    fn should_do_full_collection(&self) -> bool {
        self.should_do_full_collection
    }
    fn will_start_collection(&mut self) {
        if self.should_do_full_collection() {
            self.collection_scope = Some(CollectionScope::Full);
            self.should_do_full_collection = false;
            if GC_LOG {
                eprintln!("FullCollection");
            }
        } else {
            self.collection_scope = Some(CollectionScope::Minor);
            if GC_LOG {
                eprintln!("EdenCollection");
            }
        }
        if let Some(CollectionScope::Full) = self.collection_scope {
            self.size_before_last_full_collect =
                self.size_after_last_collect + self.bytes_allocated_this_cycle;
        } else {
            self.size_before_last_eden_collect =
                self.size_after_last_collect + self.bytes_allocated_this_cycle;
        }
    }

    fn update_object_counts(&mut self, bytes_visited: usize) {
        if let Some(CollectionScope::Full) = self.collection_scope {
            self.total_bytes_visited = 0;
        }
        self.total_bytes_visited_this_cycle = bytes_visited;
        self.total_bytes_visited += self.total_bytes_visited_this_cycle;
    }

    fn update_allocation_limits(&mut self) {
        // Calculate our current heap size threshold for the purpose of figuring out when we should
        // run another collection. This isn't the same as either size() or capacity(), though it should
        // be somewhere between the two. The key is to match the size calculations involved calls to
        // didAllocate(), while never dangerously underestimating capacity(). In extreme cases of
        // fragmentation, we may have size() much smaller than capacity().
        let mut current_heap_size = 0;
        current_heap_size += self.total_bytes_visited;

        if let Some(CollectionScope::Full) = self.collection_scope {
            self.max_heap_size = proportional_heap_size(current_heap_size).max(32 * 1024);
            self.max_eden_size = self.max_heap_size - current_heap_size;
            self.size_after_last_full_collect = current_heap_size;
            if GC_LOG {
                eprintln!("Full: currentHeapSize = {}", current_heap_size);
                eprintln!("Full: maxHeapSize = {}\nFull: maxEdenSize = {}\nFull: sizeAfterLastFullCollect = {}",self.max_heap_size,self.max_eden_size,self.size_after_last_full_collect);
            }
        } else {
            assert!(current_heap_size >= self.size_after_last_collect);

            // Theoretically, we shouldn't ever scan more memory than the heap size we planned to have.
            // But we are sloppy, so we have to defend against the overflow.
            self.max_eden_size = if current_heap_size > self.max_heap_size {
                0
            } else {
                self.max_heap_size - current_heap_size
            };
            self.size_after_last_eden_collect = current_heap_size;
            let eden_to_old_gen_ratio = self.max_eden_size as f64 / self.max_heap_size as f64;
            let min_eden_to_old_gen_ratio = 1.0 / 3.0;
            if eden_to_old_gen_ratio < min_eden_to_old_gen_ratio {
                self.should_do_full_collection = true;
            }
            // This seems suspect at first, but what it does is ensure that the nursery size is fixed.
            self.max_heap_size += current_heap_size - self.size_after_last_collect;
            self.max_eden_size = self.max_heap_size - current_heap_size;
            if GC_LOG {
                eprintln!(
                    "Eden: eden to old generation ratio: {}\nEden: minimum eden to old generation ratio {}",
                    eden_to_old_gen_ratio,min_eden_to_old_gen_ratio
                );
                eprintln!("Eden: maxEdenSize = {}", self.max_eden_size);
                eprintln!("Eden: maxHeapSize = {}", self.max_heap_size);
                eprintln!(
                    "Eden: shouldDoFullCollection = {}",
                    self.should_do_full_collection
                );
                eprintln!("Eden: currentHeapSize = {}", current_heap_size);
            }
        }
        self.size_after_last_collect = current_heap_size;
        self.bytes_allocated_this_cycle = 0;
    }
    unsafe fn collect_if_necessary(&mut self) {
        if self.bytes_allocated_this_cycle <= self.max_eden_size {
            return;
        }
        let x = 0;
        keep_on_stack!(&x);
        self.collect(false, false, &x, None);
    }

    unsafe fn allocate_slow(&mut self, size: usize) -> Address {
        if size <= LARGE_CUTOFF {
            panic!("FATAL: attampting to allocate small object using large allocation.\nreqested allocation size: {}",size);
        }

        let size = round_up_to_multiple_of(16, size);
        assert_ne!(size, 0);
        let allocation = PreciseAllocation::try_create(size, self.precise_allocations.len() as _);
        self.precise_allocations.push(allocation);
        Address::from_ptr((&*allocation).cell())
    }
    /// Mark if this cell is unmarked.
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn test_and_set_marked(&mut self, cell: *mut Header) -> bool {
        unsafe {
            let c = &mut *cell;
            if c.is_precise_allocation() {
                (&*c.precise_allocation()).test_and_set_marked()
            } else {
                let block = c.block();
                let header = (&*block).header();

                header.test_and_set_marked(Address::from_ptr(cell))
            }
        }
    }

    unsafe fn collect(
        &mut self,
        full: bool,
        unmap: bool,
        dummy: *const usize,
        snapshot: Option<&mut std::fs::File>,
    ) {
        if self.ndefers > 0 {
            return;
        }
        self.sp = dummy as usize;
        self.will_start_collection();
        if full {
            self.collection_scope = Some(CollectionScope::Full);
        }
        self.precise_allocations.iter().for_each(|precise| {
            (&mut **precise).flip();
        });
        if let Some(CollectionScope::Full) = self.collection_scope {
            self.mark_stack.clear();

            for dir in self.directories.iter_mut() {
                for block in dir.blocks.iter() {
                    (**block).bitmap.clear_all();
                }
            }
        }

        let mut task = Marking {
            gc: self,
            bytes_visited: 0,
            worklist: VecDeque::with_capacity(8),
            cons: ConservativeRoots {
                scan: Vec::with_capacity(2),
            },
            file: snapshot,
        };

        task.run();

        let visited = task.bytes_visited;
        drop(task);
        for local in self.local_allocators.iter() {
            let local = local as *const LocalAllocator as *mut LocalAllocator;
            let local = &mut *local;
            local.current_block = null_mut();
            local.unswept_cursor = 0;
        }
        self.update_object_counts(visited);
        #[cfg(feature = "debug-snapshots")]
        if let Some(file) = self.file.as_mut() {

            //freeze_cell_into(item, (*item).get_dyn(), file);
        }

        if let Some(CollectionScope::Full) = self.collection_scope {
            for dir in self.directories.iter_mut() {
                dir.blocks.retain(|block| {
                    let b = &mut **block;
                    let empty = b.sweep(true);
                    let keep = if empty { !unmap } else { true };
                    if !keep {
                        (*b.block).destroy();
                    }
                    keep
                })
            }
        } else {
            for dir in self.directories.iter_mut() {
                dir.blocks.iter().for_each(|block| {
                    let block = &mut **block;
                    block.unswept = true;
                    block.can_allocate = !block.freelist.is_empty();
                });
            }
        }

        self.update_allocation_limits();
    }
    /// Trigger garbage collection cycle.
    ///
    ///
    /// If `full` is true then
    ///
    ///
    pub fn gc(&mut self, full: bool, unmap: bool, snapshot: Option<&mut std::fs::File>) {
        let x = 0;
        keep_on_stack!(&x);
        unsafe { self.collect(full, unmap, &x, snapshot) }
    }
    #[inline]
    pub fn write_barrier<T: Cell, U: Cell>(&mut self, object: Heap<T>, field: Heap<U>) {
        unsafe {
            let obj = &mut *object.cell.as_ptr();
            let fld = &mut *field.cell.as_ptr();
            if obj.tag() != GC_BLACK {
                return;
            }

            if fld.tag() != GC_WHITE {
                return;
            }
            if GC_LOG {
                eprintln!("WriteBarrier: {:p}<-{:p}", obj, fld);
            }
            obj.set_tag(GC_GRAY);
            if self.wbuf.push(obj) {
                self.write_barrier_slowpath(obj);
            }
        }
    }
    #[inline(never)]
    unsafe fn write_barrier_slowpath(&mut self, obj: *mut Header) {
        self.wbuf.reset(&mut self.mark_stack);
        self.wbuf.push(obj);
    }
    #[inline]
    pub fn alloc<T: Cell>(&mut self, value: T) -> Heap<T> {
        unsafe {
            let size = allocation_size(&value);
            let memory = self.allocate_raw(size).to_mut_ptr::<Header>();
            memory.write(Header::new(object_ty_of(&value)));
            let sz = value.compute_size();
            std::ptr::copy_nonoverlapping(
                &value as *const T as *const u8,
                (*memory).data_start().to_mut_ptr::<u8>(),
                sz,
            );
            std::mem::forget(value);
            Heap {
                cell: NonNull::new_unchecked(memory),
                marker: Default::default(),
            }
        }
    }
}

fn allocation_size<T: Cell>(val: &T) -> usize {
    val.compute_size() + size_of::<Header>()
}
fn proportional_heap_size(heap_size: usize) -> usize {
    (heap_size as f64 * 1.27) as usize
}
impl LocalAllocator {
    /// Allocate memory from current block or find unswept block, sweep it
    /// and try to allocate from it, if allocation fails request new block

    pub fn allocate(&mut self, heap: &mut Space) -> Address {
        unsafe {
            if self.current_block.is_null() || (*self.current_block).freelist.is_empty() {
                return self.allocate_slow(heap);
            }
            let result = (&mut *self.current_block).allocate();

            if result.is_null() {
                return self.allocate_slow(heap);
            }
            heap.bytes_allocated_this_cycle += (*self.current_block).cell_size() as usize;
            result
        }
    }
    #[inline(never)]
    #[allow(clippy::mut_range_bound)]
    fn allocate_slow(&mut self, heap: &mut Space) -> Address {
        unsafe {
            let dir = &mut *self.directory;
            let mut ptr = Address::null();
            let mut cursor = self.unswept_cursor;
            let start = cursor;
            for i in start..dir.blocks.len() {
                if dir.blocks[i] == self.current_block {
                    continue;
                }
                (*dir.blocks[i]).sweep(false);
                if !(*dir.blocks[i]).freelist.is_empty() {
                    ptr = (*dir.blocks[i]).allocate();
                    if ptr.is_non_null() {
                        heap.bytes_allocated_this_cycle += (&*dir.blocks[i]).cell_size() as usize;
                        self.current_block = dir.blocks[i];
                        break;
                    }
                }
                cursor = i;
            }
            self.unswept_cursor = cursor;
            if ptr.is_null() {
                let block = Block::new(dir.cell_size);

                dir.blocks.push(block as *mut _);
                self.current_block = block as *mut _;
                heap.blocks.add(block.block);
                let res = block.allocate();
                heap.bytes_allocated_this_cycle += block.cell_size() as usize;

                res
            } else {
                ptr
            }
        }
    }
}

pub struct Marking<'a> {
    pub gc: &'a mut Space,
    pub worklist: VecDeque<*mut Header>,
    pub bytes_visited: usize,
    cons: ConservativeRoots,
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
            let this = &mut *(self as *mut Self);
            for c in self.gc.constraints.iter_mut() {
                c.execute(this);
            }
        }
    }
    fn process_roots(&mut self) {
        unsafe {
            let mut head = self.gc.scopes;
            while !head.is_null() {
                let scope = &mut *head;
                scope.roots.retain(|item| {
                    /*if item.is_null() {
                        false
                    } else {
                        self.mark(*item);
                        true
                    }*/
                    match item {
                        Some(ptr) => {
                            (*ptr.as_ptr()).trace(self);
                            true
                        }
                        None => false,
                    }
                });
                head = (*head).next;
            }
            let scope = self.gc.persistent;
            (*scope).roots.retain(|item| match item {
                Some(ptr) => {
                    (*ptr.as_ptr()).trace(self);

                    true
                }
                None => false,
            });

            while let Some((from, to)) = self.cons.scan.pop() {
                let mut scan = from as *mut *mut u8;
                let mut to = to as *mut *mut u8;
                if scan > to {
                    swap(&mut to, &mut scan);
                }
                while scan < to {
                    let ptr = *scan;
                    if ptr.is_null() {
                        scan = scan.add(1);
                        continue;
                    }
                    self.find_gc_object_pointer_for_marking(ptr, |this, pointer| {
                        this.mark(pointer);
                    });
                    scan = scan.add(1);
                }
            }
        }
    }
    fn process_worklist(&mut self) {
        while let Some(item) = self.gc.mark_stack.pop() {
            self.mark(item);
        }
        while let Some(item) = self.worklist.pop_front() {
            unsafe {
                (*item).set_tag(GC_BLACK);
                self.visit_value(item);
            }
        }
    }
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn mark(&mut self, val: *mut Header) {
        unsafe {
            if !self.gc.test_and_set_marked(val) {
                let obj = &mut *val;
                obj.set_tag(GC_GRAY);
                self.bytes_visited += round_up_to_multiple_of(
                    16,
                    obj.get_dyn().compute_size() + core::mem::size_of::<Header>(),
                );
                self.worklist.push_back(obj);
            }
        }
    }
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn visit_value(&mut self, val: *mut Header) {
        unsafe {
            (&*val).get_dyn().trace(self);
        }
    }
    pub fn add_conservative_roots(&mut self, from: *mut u8, to: *mut u8) {
        self.cons.scan.push((from, to));
    }
    unsafe fn find_gc_object_pointer_for_marking(
        &mut self,
        ptr: *mut u8,
        mut f: impl FnMut(&mut Self, *mut Header),
    ) {
        if !self.gc.precise_allocations.is_empty() {
            if (**self.gc.precise_allocations.first().unwrap()).above_lower_bound(ptr.cast())
                && (**self.gc.precise_allocations.last().unwrap()).below_upper_bound(ptr.cast())
            {
                let result = self
                    .gc
                    .precise_allocations
                    .binary_search(&PreciseAllocation::from_cell(ptr.cast()));
                match result {
                    Ok(ix) => {
                        if (*self.gc.precise_allocations[ix]).has_valid_cell {
                            f(self, ptr.cast());
                        }
                    }
                    _ => (),
                }
            }
        }
        let filter = self.gc.blocks.filter;
        let set = &self.gc.blocks.set;
        let candidate = Block::from_cell(Address::from_ptr(ptr));
        if filter.rule_out(candidate as _) {
            return;
        }

        if !set.contains(&candidate) {
            return;
        }

        let mut try_ptr = |ptr| {
            let is_live = (*candidate).is_live(ptr);
            if is_live {
                f(self, ptr as *mut _);
            }
            is_live
        };

        if try_ptr(ptr) {
            return;
        }

        let aligned = (*candidate).header().cell_align(ptr.cast());
        try_ptr(aligned.cast());
    }
}

impl<'a> Tracer for Marking<'a> {
    fn trace(&mut self, hdr: *mut Header) {
        self.mark(hdr);
    }
}

struct WriteBarrierBuffer {
    start: *mut *mut Header,
    current: *mut *mut Header,
    end: *mut *mut Header,
}

impl WriteBarrierBuffer {
    pub fn new(size: usize) -> Self {
        let mut v = Vec::<*mut Header>::with_capacity(size);
        let ptr = v.as_mut_ptr();

        std::mem::forget(v);
        Self {
            start: ptr,
            current: ptr,

            end: unsafe { ptr.add(size) },
        }
    }

    pub fn push(&mut self, val: *mut Header) -> bool {
        unsafe {
            if self.current == self.end {
                return true;
            }
            let ptr = self.current;
            ptr.write(val);
            self.current = ptr.add(1);
            false
        }
    }

    pub fn reset(&mut self, buffer: &mut SegmentedVec<*mut Header>) {
        unsafe {
            let mut start = self.start;
            while start < self.end {
                buffer.push(start.read());
                start = start.add(1);
            }
            self.current = self.start;
        }
    }
}

pub struct ConservativeRoots {
    pub scan: Vec<(*mut u8, *mut u8)>,
}
