use super::{block::*, constants::*};
use crate::gc::Address;
#[cfg(windows)]
pub mod _win {
    use super::*;
    use crate::vm::VirtualMachine;
    use core::{ptr::null_mut, usize};
    use std::mem::size_of;

    use winapi::um::{
        memoryapi::{VirtualAlloc, VirtualFree},
        winnt::{MEM_COMMIT, MEM_DECOMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE},
    };
    pub struct Mmap {
        start: *mut u8,
        end: *mut u8,
        size: usize,
    }
    impl Mmap {
        pub fn new(size: usize) -> Self {
            unsafe {
                let mem = VirtualAlloc(null_mut(), size, MEM_RESERVE, PAGE_READWRITE);
                let mem = mem as *mut u8;

                let end = mem.add(size);

                Self {
                    start: mem,
                    end,
                    size,
                }
            }
        }
        /// Return a `BLOCK_SIZE` aligned pointer to the mmap'ed region.
        pub fn aligned(&self) -> *mut u8 {
            let offset = BLOCK_SIZE - (self.start as usize + size_of::<VirtualMachine>()) % BLOCK_SIZE;
            unsafe { self.start.add(offset) as *mut u8 }
        }

        pub fn start(&self) -> *mut u8 {
            self.start
        }
        pub fn end(&self) -> *mut u8 {
            self.end
        }

        pub fn dontneed(&self, page: *mut u8, size: usize) {
            unsafe {
                //DiscardVirtualMemory(page.cast(), size as _);
                VirtualFree(page.cast(), size, MEM_DECOMMIT);
            }
        }

        pub fn commit(&self, page: *mut u8, size: usize) {
            unsafe {
                VirtualAlloc(page.cast(), size, MEM_COMMIT, PAGE_READWRITE);
            }
        }
    }

    impl Drop for Mmap {
        fn drop(&mut self) {
            unsafe {
                VirtualFree(self.start.cast(), self.size, MEM_RELEASE);
            }
        }
    }
}

#[cfg(unix)]
pub mod _unix {
    use crate::vm::VirtualMachine;
    use std::mem::size_of;

    use super::*;
    pub struct Mmap {
        start: *mut u8,
        end: *mut u8,
        size: usize,
    }

    impl Mmap {
        pub fn new(size: usize) -> Self {
            unsafe {
                let map = libc::mmap(
                    core::ptr::null_mut(),
                    size as _,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_PRIVATE | libc::MAP_ANON,
                    -1,
                    0,
                );
                libc::madvise(map, size, libc::MADV_SEQUENTIAL);
                if map == libc::MAP_FAILED {
                    panic!("mmap failed");
                }
                Self {
                    start: map as *mut u8,
                    end: (map as usize + size) as *mut u8,
                    size,
                }
            }
        }
        /// Return a `BLOCK_SIZE` aligned pointer to the mmap'ed region.
        pub fn aligned(&self) -> *mut u8 {
            let offset = BLOCK_SIZE - (self.start as usize + size_of::<VirtualMachine>()) % BLOCK_SIZE;
            unsafe { self.start.add(offset) as *mut u8 }
        }

        pub fn start(&self) -> *mut u8 {
            self.start
        }
        pub fn end(&self) -> *mut u8 {
            self.end
        }

        pub fn dontneed(&self, page: *mut u8, size: usize) {
            unsafe {
                libc::madvise(page as *mut _, size as _, libc::MADV_DONTNEED);
            }
        }

        pub fn commit(&self, page: *mut u8, size: usize) {
            unsafe {
                libc::madvise(
                    page as *mut _,
                    size as _,
                    libc::MADV_WILLNEED | libc::MADV_SEQUENTIAL,
                );
            }
        }
    }

    impl Drop for Mmap {
        fn drop(&mut self) {
            unsafe {
                libc::munmap(self.start() as *mut _, self.size as _);
            }
        }
    }
}

#[cfg(unix)]
pub use _unix::*;
#[cfg(windows)]
pub use _win::*;
use core::sync::atomic::Ordering;

pub struct BlockAllocator {
    #[cfg(feature = "threaded")]
    lock: ReentrantMutex,
    free_blocks: Vec<*mut Block>,

    //pub bitmap: SpaceBitmap<16>,
    pub data_bound: *mut u8,
    pub data: *mut u8,
    pub mmap: Mmap,
}

impl BlockAllocator {
    pub fn total_blocks(&self) -> usize {
        (self.mmap.end() as usize - self.mmap.aligned() as usize) / BLOCK_SIZE
    }
    pub fn new(size: usize) -> BlockAllocator {
        let map = Mmap::new(size);

        let this = Self {
            #[cfg(feature = "threaded")]
            lock: ReentrantMutex::new(),
            data: map.aligned(),
            data_bound: map.end(),
            free_blocks: Vec::new(),

            mmap: map,
        };
        debug_assert!(this.data as usize % BLOCK_SIZE == 0);
        this
    }

    /// Get a new block aligned to `BLOCK_SIZE`.
    pub fn get_block(&mut self) -> Option<*mut Block> {
        if self.free_blocks.is_empty() {
            return self.build_block();
        }

        let block = self
            .free_blocks
            .pop()
            .map(|x| {
                self.mmap.commit(x as *mut u8, BLOCK_SIZE);
                Block::new(x as *mut u8);
                x
            })
            .or_else(|| self.build_block());
        if block.is_none() {
            panic!("OOM");
        }
        block
    }

    pub fn is_in_space(&self, object: Address) -> bool {
        self.mmap.start() < object.to_mut_ptr() && object.to_mut_ptr() <= self.data_bound
    }
    #[allow(unused_unsafe)]
    fn build_block(&mut self) -> Option<*mut Block> {
        unsafe {
            let data = as_atomic!(&self.data;AtomicUsize);
            let mut old = data.load(Ordering::Relaxed);
            let mut new;
            loop {
                new = old + BLOCK_SIZE;
                if new > self.data_bound as usize {
                    return None;
                }
                let res = data.compare_exchange_weak(old, new, Ordering::SeqCst, Ordering::Relaxed);
                match res {
                    Ok(_) => break,
                    Err(x) => old = x,
                }
            }
            debug_assert!(old % BLOCK_SIZE == 0, "block is not aligned for block_size");
            self.mmap.commit(old as *mut u8, BLOCK_SIZE);
            Some(old as *mut Block)
        }
    }

    /// Return a collection of blocks.
    pub fn return_blocks(&mut self, blocks: impl Iterator<Item = *mut Block>) {
        blocks.for_each(|block| {
            self.mmap.dontneed(block as *mut u8, BLOCK_SIZE); // MADV_DONTNEED or MEM_DECOMMIT
            self.free_blocks.push(block);
        });
    }
    pub fn return_block(&mut self, block: *mut Block) {
        self.mmap.dontneed(block as *mut u8, BLOCK_SIZE); // MADV_DONTNEED or MEM_DECOMMIT
        self.free_blocks.push(block);
    }

    /// Return the number of unallocated blocks.
    pub fn available_blocks(&self) -> usize {
        let nblocks = ((self.data_bound as usize) - (self.data as usize)) / BLOCK_SIZE;

        nblocks + self.free_blocks.len()
    }

    pub fn space_for_vm(&self) -> *mut u8 {
        self.mmap.start()
    }
}
