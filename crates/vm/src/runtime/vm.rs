use std::collections::{HashMap, VecDeque};

use wtf_rs::stack_bounds::StackBounds;

use super::{context::Context, js_symbol::JsSymbol, symbol::Symbol};
use super::{options::Options, ref_ptr::Ref, symbol_table::SymbolTable};
use crate::{
    gc::{handle::Handle, heap_cell::HeapCell},
    heap::{
        snapshot::HeapSnapshot,
        trace::{Slot, Tracer},
        Heap,
    },
};

pub struct JsVirtualMachine {
    pub(crate) heap: Ref<Heap>,
    pub(crate) sym_table: SymbolTable,
    pub(crate) symbols: HashMap<Symbol, Handle<JsSymbol>>,
    pub(crate) options: Options,
    pub(crate) context: Option<Handle<Context>>,
}
impl Drop for JsVirtualMachine {
    fn drop(&mut self) {
        self.dispose();
    }
}
impl JsVirtualMachine {
    pub fn make_context(&mut self) -> Handle<Context> {
        let ctx = Context::new(self);
        self.context = Some(ctx);
        ctx
    }
    pub fn context(&self) -> Option<Handle<Context>> {
        self.context
    }
    pub fn create(options: Options) -> Box<Self> {
        let mut vm = Ref::new(Box::into_raw(Box::new(Self {
            heap: Ref::new(std::ptr::null_mut()),
            sym_table: SymbolTable::new(),
            symbols: HashMap::new(),
            options,
            context: None,
        })));
        vm.heap = Ref::new(Box::into_raw(Box::new(Heap::new(
            vm,
            vm.options.heap_size,
            vm.options.threshold,
        ))));

        unsafe { Box::from_raw(vm.pointer) }
    }
    pub fn gc(&mut self, evac: bool) {
        self.heap.gc(evac)
    }
    fn dispose(&mut self) {
        unsafe {
            let _ = Box::from_raw(self.heap.pointer);
        }
    }

    pub fn intern(&mut self, key: impl AsRef<str>) -> Symbol {
        self.sym_table.intern(key)
    }

    pub fn intern_i32(&mut self, key: i32) -> Symbol {
        let converted = key as u32;
        if converted as i32 == key {
            return Symbol::Indexed(converted);
        }
        self.intern(key.to_string())
    }

    pub fn intern_i64(&mut self, key: i64) -> Symbol {
        let converted = key as u32;
        if converted as i64 == key {
            return Symbol::Indexed(converted);
        }
        self.intern(key.to_string())
    }

    pub fn intern_u32(&mut self, key: u32) -> Symbol {
        Symbol::Indexed(key)
    }

    pub fn intern_f64(&mut self, key: f64) -> Symbol {
        let converted = key as u32;
        if converted as f64 == key {
            return Symbol::Indexed(converted);
        }
        self.intern(key.to_string())
    }

    /// Get the total size of all of the objects allocated on the VM heap.
    pub fn allocated_heap_memory(&self) -> usize {
        self.heap.allocated
    }

    /// Record a heap snapshot at the current point in execution representing
    /// how much memory is allocated on the heap and each object on it.
    ///
    /// This is a relatively expensive operation as it must trace every individual object
    /// on the heap which is expensive. Therefore you should minimize frequent calls to this method.
    /// To know just the total amount of memory allocated on the heap you can use [`allocated_heap_memory`](Self::allocated_heap_memory).
    pub fn record_heap_snapshot(&mut self) -> HeapSnapshot {
        let mut objects = Vec::with_capacity(10);
        unsafe {
            self.get_all_live_objects(|cell| objects.push((*cell).get_dyn().compute_size()));
        }
        HeapSnapshot {
            object_count: objects.len(),
            total_size: self.heap.allocated,
            objects,
        }
    }

    /// Trace all of the objects on the heap and run a callback on every cell.
    ///
    ///
    /// # Safety
    ///
    /// This function is unsafe because if user decides to change any pointer passed to callback
    /// it could lead to UB or segfaults.
    ///
    pub unsafe fn get_all_live_objects(&mut self, mut callback: impl FnMut(*mut HeapCell)) {
        let mut precise_roots: Vec<*mut HeapCell> = Vec::new();
        for (_, sym) in self.symbols.iter_mut() {
            precise_roots.push(sym.cell.as_ptr());
        }

        {
            #[inline(never)]
            fn get_stack_pointer() -> usize {
                let x = 0x400usize;
                &x as *const usize as usize
            }

            let bounds = StackBounds::current_thread_stack_bounds();
            self.heap.collect_roots(
                bounds.origin as *mut *mut u8,
                get_stack_pointer() as *mut *mut u8,
                &mut precise_roots,
            );
        }

        struct Visitor<'a> {
            mark: bool,
            queue: &'a mut VecDeque<*mut HeapCell>,
        }

        impl<'a> Tracer for Visitor<'a> {
            fn trace(&mut self, slot: Slot) {
                unsafe {
                    let child: &mut HeapCell = &mut *slot.value();
                    if child.get_mark() != self.mark {
                        self.queue.push_back(child);
                    }
                }
            }
        }

        let mut queue = VecDeque::new();
        let mark = !self.heap.current_live_mark;
        {
            for root in precise_roots {
                queue.push_back(root);
            }
            while let Some(object) = queue.pop_front() {
                {
                    //let object_addr = Address::from_ptr(object);
                    if !(&mut *object).mark(mark) {
                        let mut visitor = Visitor {
                            queue: &mut queue,
                            mark,
                        };
                        callback(object);
                        (*object).get_dyn().visit_children(&mut visitor);
                    }
                }
            }
        }
        self.heap.current_live_mark = mark;
        self.heap.los.current_live_mark = mark;
        (*self.heap.immix).set_current_live_mark(mark);
    }
}
