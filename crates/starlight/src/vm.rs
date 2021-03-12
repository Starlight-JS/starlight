use crate::{
    heap::{cell::GcPointer, cell::Trace, Heap, SimpleMarkingConstraint, SlotVisitor},
    jsrt::object::{object_constructor, object_to_string},
};
#[macro_use]
pub mod class;
#[macro_use]
pub mod method_table;
pub mod arguments;
pub mod array;
pub mod array_storage;
pub mod attributes;
pub mod bigint;
pub mod code_block;
pub mod error;
pub mod function;
pub mod global;
pub mod indexed_elements;
pub mod interpreter;
pub mod object;
pub mod property_descriptor;
pub mod slot;
pub mod string;
pub mod structure;
pub mod symbol_table;
pub mod thread;
pub mod value;
use attributes::*;
use object::*;
use property_descriptor::*;
use value::*;
pub struct Runtime {
    pub(crate) heap: Box<Heap>,
    pub(crate) stack: Stack,
    pub(crate) global_data: GlobalData,
    pub(crate) global_object: Option<GcPointer<JsObject>>,
    pub(crate) external_references: Option<Box<[usize]>>,
}

impl Runtime {
    pub fn description(&self, sym: Symbol) -> String {
        match sym {
            Symbol::Key(key) => symbol_table::symbol_table().description(key).to_owned(),
            Symbol::Index(x) => x.to_string(),
        }
    }
    pub fn heap(&mut self) -> &mut Heap {
        &mut self.heap
    }
    pub(crate) fn new_empty(
        track_allocations: bool,
        external_references: Option<Box<[usize]>>,
    ) -> Box<Self> {
        let heap = Box::new(Heap::new(track_allocations));
        let mut this = Box::new(Self {
            heap,
            stack: Stack::new(),
            global_object: None,
            global_data: GlobalData::default(),
            external_references,
        });
        let vm = &mut *this as *mut Runtime;
        this.heap.add_constraint(SimpleMarkingConstraint::new(
            "Mark VM roots",
            move |visitor| {
                let rt = unsafe { &mut *vm };
                rt.global_object.trace(visitor);
                rt.global_data.trace(visitor);
            },
        ));

        this
    }
    pub fn new(track_allocations: bool, external_references: Option<Box<[usize]>>) -> Box<Runtime> {
        let heap = Box::new(Heap::new(track_allocations));
        let mut this = Box::new(Self {
            heap,
            stack: Stack::new(),
            global_object: None,
            global_data: GlobalData::default(),
            external_references,
        });
        let vm = &mut *this as *mut Runtime;
        this.heap.defer();
        this.heap.add_constraint(SimpleMarkingConstraint::new(
            "Mark VM roots",
            move |visitor| {
                let rt = unsafe { &mut *vm };
                rt.global_object.trace(visitor);
                rt.global_data.trace(visitor);
            },
        ));
        this.global_data.empty_object_struct = Some(Structure::new_indexed(&mut this, None, false));
        let s = Structure::new_unique_indexed(&mut this, None, false);
        let mut proto = JsObject::new(&mut this, s, JsObject::get_class(), ObjectTag::Ordinary);
        this.global_data.object_prototype = Some(proto.clone());
        this.global_data.function_struct = Some(Structure::new_indexed(&mut this, None, false));
        this.global_data.normal_arguments_structure =
            Some(Structure::new_indexed(&mut this, None, false));
        this.global_object = Some(JsGlobal::new(&mut this));
        this.global_data
            .empty_object_struct
            .as_mut()
            .unwrap()
            .change_prototype_with_no_transition(proto.clone());

        let name = "Object".intern();
        let mut obj_constructor = JsNativeFunction::new(&mut this, name, object_constructor, 1);
        let _ = obj_constructor.define_own_property(
            &mut this,
            "prototype".intern(),
            &*DataDescriptor::new(JsValue::from(proto.clone()), NONE),
            false,
        );
        let _ = proto.define_own_property(
            &mut this,
            "constructor".intern(),
            &*DataDescriptor::new(JsValue::from(obj_constructor.clone()), W | C),
            false,
        );
        let obj_to_string =
            JsNativeFunction::new(&mut this, "toString".intern(), object_to_string, 0);
        let _ = proto.define_own_property(
            &mut this,
            "toString".intern(),
            &*DataDescriptor::new(JsValue::from(obj_to_string), W | C),
            false,
        );
        let name = "Object".intern();
        this.global_data
            .empty_object_struct
            .as_mut()
            .unwrap()
            .change_prototype_with_no_transition(proto.clone());
        this.global_data.number_structure = Some(Structure::new_indexed(&mut this, None, false));
        keep_on_stack!(&mut proto);
        this.init_error(proto.clone());
        keep_on_stack!(&mut proto);
        let _ = this.global_object().define_own_property(
            &mut this,
            name,
            &*DataDescriptor::new(JsValue::from(obj_constructor), W | C),
            false,
        );
        keep_on_stack!(&mut proto);
        this.init_array(proto.clone());
        keep_on_stack!(&mut proto);
        this.init_func(proto);

        this.heap.undefer();

        this
    }

    pub fn global_object(&self) -> GcPointer<JsObject> {
        unwrap_unchecked(self.global_object.clone())
    }

    pub fn global_data(&self) -> &GlobalData {
        &self.global_data
    }
}

use starlight_derive::GcTrace;
use wtf_rs::{keep_on_stack, unwrap_unchecked};

use self::{
    function::JsNativeFunction,
    global::JsGlobal,
    interpreter::stack::Stack,
    object::JsObject,
    structure::Structure,
    symbol_table::{Internable, Symbol},
};

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

impl GlobalData {
    pub fn get_function_struct(&self) -> GcPointer<Structure> {
        unwrap_unchecked(self.function_struct.clone())
    }

    pub fn get_object_prototype(&self) -> GcPointer<JsObject> {
        unwrap_unchecked(self.object_prototype.clone())
    }
}
