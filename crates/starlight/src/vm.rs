use crate::heap::{cell::GcPointer, cell::Trace, Heap, SlotVisitor};
#[macro_use]
pub mod class;
#[macro_use]
pub mod method_table;
pub mod array_storage;
pub mod attributes;
pub mod global;
pub mod indexed_elements;
pub mod object;
pub mod property_descriptor;
pub mod slot;
pub mod string;
pub mod structure;
pub mod symbol_table;
pub mod thread;
pub mod value;
pub struct Runtime {
    heap: Box<Heap>,
    global_data: GlobalData,
}

impl Runtime {
    pub fn heap(&mut self) -> &mut Heap {
        &mut self.heap
    }

    pub fn new(track_allocations: bool) -> Box<Runtime> {
        let heap = Box::new(Heap::new(track_allocations));
        Box::new(Self {
            heap,
            global_data: GlobalData::default(),
        })
    }

    pub fn global_data(&self) -> &GlobalData {
        &self.global_data
    }
}

use starlight_derive::GcTrace;

use self::{object::JsObject, structure::Structure};

#[derive(Default, GcTrace)]
pub struct GlobalData {
    pub(crate) normal_arguments_structure: Option<GcPointer<Structure>>,
    pub(crate) empty_object_struct: Option<GcPointer<Structure>>,
    pub(crate) function_struct: Option<GcPointer<Structure>>,
    pub(crate) object_prototype: Option<GcPointer<JsObject>>,
    pub(crate) number_prototype: Option<GcPointer<JsObject>>,
    pub(crate) string_prototype: Option<GcPointer<JsObject>>,
    pub(crate) boolean_prototype: Option<GcPointer<JsObject>>,
    pub(crate) symbol_prototype: Option<GcPointer<JsObject>>,
    pub(crate) error: Option<GcPointer<JsObject>>,
    pub(crate) type_error: Option<GcPointer<JsObject>>,
    pub(crate) reference_error: Option<GcPointer<JsObject>>,
    pub(crate) range_error: Option<GcPointer<JsObject>>,
    pub(crate) syntax_error: Option<GcPointer<JsObject>>,
    pub(crate) internal_error: Option<GcPointer<JsObject>>,
    pub(crate) eval_error: Option<GcPointer<JsObject>>,
    pub(crate) array_prototype: Option<GcPointer<JsObject>>,
    pub(crate) func_prototype: Option<GcPointer<JsObject>>,
    pub(crate) string_structure: Option<GcPointer<Structure>>,
    pub(crate) number_structure: Option<GcPointer<Structure>>,
    pub(crate) array_structure: Option<GcPointer<Structure>>,
    pub(crate) error_structure: Option<GcPointer<Structure>>,
    pub(crate) range_error_structure: Option<GcPointer<Structure>>,
    pub(crate) reference_error_structure: Option<GcPointer<Structure>>,
    pub(crate) syntax_error_structure: Option<GcPointer<Structure>>,
    pub(crate) type_error_structure: Option<GcPointer<Structure>>,
    pub(crate) uri_error_structure: Option<GcPointer<Structure>>,
    pub(crate) eval_error_structure: Option<GcPointer<Structure>>,
}
