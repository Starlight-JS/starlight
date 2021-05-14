//! Large Object Space

use libmimalloc_sys::{mi_free, mi_good_size, mi_heap_malloc, mi_heap_new, mi_heap_t};

use crate::gc::cell::{GcPointerBase, DEFINETELY_WHITE};

pub struct LargeObjectSpace {
    heap: *mut mi_heap_t,
    pub(super) allocated: usize,
}

impl LargeObjectSpace {
    pub fn allocate(&mut self, size: usize) -> *mut u8 {
        self.allocated += unsafe { mi_good_size(size) };
        unsafe { mi_heap_malloc(self.heap, size) as *mut u8 }
    }
    pub fn new() -> Self {
        Self {
            heap: unsafe { mi_heap_new() },
            allocated: 0,
        }
    }
    pub fn walk(&mut self, cb: &mut dyn FnMut(*mut GcPointerBase, usize) -> bool) {
        unsafe extern "C" fn walk(
            _heap: *const libmimalloc_sys::mi_heap_t,
            _area: *const libmimalloc_sys::mi_heap_area_t,
            block: *mut libc::c_void,
            block_sz: usize,
            arg: *mut libc::c_void,
        ) -> bool {
            if block.is_null() {
                return true;
            }
            let closure: *mut dyn FnMut(*mut GcPointerBase, usize) -> bool =
                std::mem::transmute(*arg.cast::<(usize, usize)>());

            (&mut *closure)(block as _, block_sz)
        }

        let f: &mut dyn FnMut(*mut GcPointerBase, usize) -> bool = cb;
        let trait_obj: (usize, usize) = unsafe { std::mem::transmute(f) };
        unsafe {
            libmimalloc_sys::mi_heap_visit_blocks(
                self.heap,
                true,
                Some(walk),
                &trait_obj as *const (usize, usize) as _,
            );
        }
    }

    pub unsafe fn sweep(&mut self) -> usize {
        let this = self as *mut Self;
        let mut allocated = 0;
        self.walk(&mut |object, size| {
            if (*object).state() == DEFINETELY_WHITE {
                core::ptr::drop_in_place((*object).get_dyn());
                mi_free(object.cast());
            } else {
                allocated += size;
            }
            true
        });
        self.allocated = allocated;
        allocated
    }

    pub fn filter(&self, addr: usize) -> bool {
        unsafe {
            if libmimalloc_sys::mi_heap_check_owned(self.heap, addr as _) {
                if libmimalloc_sys::mi_heap_contains_block(self.heap, addr as _) {
                    return true;
                }
            }
            false
        }
    }
}
