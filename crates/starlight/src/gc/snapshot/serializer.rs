/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use crate::{
    bytecode::TypeFeedBack,
    gc::cell::{GcCell, GcPointer, GcPointerBase, WeakRef},
    vm::{
        arguments::JsArguments,
        array_storage::ArrayStorage,
        attributes::AttrSafe,
        code_block::CodeBlock,
        context::Context,
        function::{FuncType, JsFunction},
        global::JsGlobal,
        indexed_elements::*,
        interpreter::SpreadValue,
        object::{JsObject, ObjectTag},
        property_descriptor::{Accessor, StoredSlot},
        slot::*,
        string::JsString,
        structure::{
            DeletedEntry, DeletedEntryHolder, MapEntry, Structure, Transition, TransitionKey,
            TransitionsTable,
        },
        symbol_table::{symbol_table, JsSymbol, Symbol, SymbolID},
        value::*,
        GlobalData,
    },
};
use crate::{jsrt::VM_NATIVE_REFERENCES, vm::VirtualMachine};
use std::{collections::HashMap, io::Write, u8};

pub struct SnapshotSerializer {
    pub(crate) reference_map: Vec<usize>,
    pub(crate) output: Vec<u8>,
    symbol_map: HashMap<Symbol, u32>,
    log: bool,
}

impl SnapshotSerializer {
    pub(super) fn new(log: bool) -> Self {
        Self {
            log,
            reference_map: Vec::new(),
            output: vec![],
            symbol_map: HashMap::new(),
        }
    }
    pub(crate) fn build_reference_map(&mut self, vm: &mut VirtualMachine) {
        let mut indexx = 0;
        unsafe {
            VM_NATIVE_REFERENCES
                .iter()
                .enumerate()
                .for_each(|(_index, reference)| {
                    /*match self.reference_map.insert(*reference, indexx) {
                        Some(p) => {
                            backtrace::resolve(*reference as *mut _, |sym| {
                                if let Some(name) = sym.name() {
                                    panic!(
                                        "duplicate reference #{}: {:x} '{}'",
                                        _index,
                                        *reference,
                                        name.as_str().unwrap()
                                    );
                                } else {
                                    panic!("duplicate reference #{}: {:x}", _index, *reference);
                                }
                            });
                            panic!("duplicate {:x} at {}({})", *reference, _index, p);
                        }
                        _ => (),
                    }*/
                    self.reference_map.push(*reference);
                    indexx += 1;
                });

            let references = &vm.external_references;
            for (_index, reference) in references.iter().enumerate() {
                /* let result = self.reference_map.insert(*reference, indexx);
                indexx += 1;
                match result {
                    Some(_) => {
                        panic!("Reference 0x{:x}", reference);
                    }
                    _ => (),
                }*/
                self.reference_map.push(*reference);
            }
        }
    }
    pub(crate) fn build_symbol_table(&mut self) {
        let symtab = symbol_table();
        let patch_at = self.output.len();
        self.write_u32(0);
        let mut count = 0u32;
        for entry in symtab.symbols.iter() {
            let key = entry.key();
            let index = entry.value();
            let ix = self.symbol_map.len() as u32;
            self.symbol_map.insert(Symbol::Key(SymbolID(*index)), ix);
            self.write_u32(ix);
            self.write_u32(key.len() as u32);
            for byte in key.bytes() {
                self.write_u8(byte);
            }
            count += 1;
        }
        let count = count.to_le_bytes();
        self.output[patch_at] = count[0];
        self.output[patch_at + 1] = count[1];
        self.output[patch_at + 2] = count[2];
        self.output[patch_at + 3] = count[3];
    }
    pub(crate) fn build_heap_reference_map(&mut self, vm: &mut VirtualMachine) {
        let gc = vm.heap();

        /*Heap::walk(gc.mi_heap, |object, _| {
            //let ix = self.reference_map.len() as u32;
            self.reference_map.push(object);
            //self.reference_map.insert(object as usize, ix);
            true
        });*/
        gc.walk(&mut |object, _| {
            self.reference_map.push(object as _);
            true
        });
    }

    pub(crate) fn build_heap_reference_map_in_context(
        &mut self,
        vm: &mut VirtualMachine,
        ctx: GcPointer<Context>,
    ) {
        let gc = vm.heap();
        let callback = &mut |object| {
            self.reference_map.push(object as _);
            true
        };
        gc.walk_in_context(ctx, callback);
        gc.weak_slots(&mut |slot| {
            callback(slot.base.as_ptr());
            let value = slot.value;
            match value {
                Some(pointer) => {
                    callback(pointer.base.as_ptr());
                }
                None => {}
            }
        });
    }

    pub(crate) fn serialize_context(
        &mut self,
        vm: &mut VirtualMachine,
        mut context: GcPointer<Context>,
    ) {
        let gc = vm.heap();
        let patch_at = self.output.len();
        self.write_u32(0);
        let mut count: u32 = 0;

        let callback = &mut |object| unsafe {
            let object = object as usize;
            //Heap::walk(gc.mi_heap, |object, _| unsafe {
            let base = &mut *(object as *mut GcPointerBase);
            self.write_reference(object as *const u8);
            logln_if!(
                self.log,
                "serialize reference {:p} '{}' at index {}",
                base,
                base.get_dyn().type_name(),
                self.reference_map
                    .iter()
                    .enumerate()
                    .find(|x| *x.1 == object)
                    .unwrap()
                    .0,
            );
            self.try_write_reference(base.get_dyn().deser_pair().0 as *const u8)
                .unwrap_or_else(|| {
                    panic!("no deserializer for type '{}'", base.get_dyn().type_name());
                });
            self.write_reference(base.get_dyn().deser_pair().1 as *const u8);
            let patch_at = self.output.len();
            self.write_u32(0);
            base.get_dyn().serialize(self);
            let buf = (self.output.len() as u32).to_le_bytes();
            self.output[patch_at] = buf[0];
            self.output[patch_at + 1] = buf[1];
            self.output[patch_at + 2] = buf[2];
            self.output[patch_at + 3] = buf[3];
            count += 1;
            true
        };

        gc.walk_in_context(context, callback);
        gc.weak_slots(&mut |slot| {
            callback(slot.base.as_ptr());
            let value = slot.value;
            match value {
                Some(pointer) => {
                    // callback();
                }
                None => {}
            }
        });
        let buf = count.to_le_bytes();
        self.output[patch_at] = buf[0];
        self.output[patch_at + 1] = buf[1];
        self.output[patch_at + 2] = buf[2];
        self.output[patch_at + 3] = buf[3];
        let mut count: u32 = 0;
        let patch_at = self.output.len();
        self.write_u32(0);
        gc.weak_slots(&mut |weak_slot| unsafe {
            weak_slot.serialize(self);
            count += 1;
        });
        let buf = count.to_le_bytes();
        self.output[patch_at] = buf[0];
        self.output[patch_at + 1] = buf[1];
        self.output[patch_at + 2] = buf[2];
        self.output[patch_at + 3] = buf[3];
        context.serialize(self);
    }

    pub(crate) fn serialize(&mut self, vm: &mut VirtualMachine) {
        let gc = vm.heap();
        let patch_at = self.output.len();
        self.write_u32(0);
        let mut count: u32 = 0;
        gc.walk(&mut |object, _| unsafe {
            let object = object as usize;
            //Heap::walk(gc.mi_heap, |object, _| unsafe {
            let base = &mut *(object as *mut GcPointerBase);
            self.write_reference(object as *const u8);
            logln_if!(
                self.log,
                "serialize reference {:p} '{}' at index {}",
                base,
                base.get_dyn().type_name(),
                self.reference_map
                    .iter()
                    .enumerate()
                    .find(|x| *x.1 == object)
                    .unwrap()
                    .0,
            );
            self.try_write_reference(base.get_dyn().deser_pair().0 as *const u8)
                .unwrap_or_else(|| {
                    panic!("no deserializer for type '{}'", base.get_dyn().type_name());
                });
            self.write_reference(base.get_dyn().deser_pair().1 as *const u8);
            let patch_at = self.output.len();
            self.write_u32(0);
            base.get_dyn().serialize(self);
            let buf = (self.output.len() as u32).to_le_bytes();
            self.output[patch_at] = buf[0];
            self.output[patch_at + 1] = buf[1];
            self.output[patch_at + 2] = buf[2];
            self.output[patch_at + 3] = buf[3];
            count += 1;
            true
        });
        let buf = count.to_le_bytes();
        self.output[patch_at] = buf[0];
        self.output[patch_at + 1] = buf[1];
        self.output[patch_at + 2] = buf[2];
        self.output[patch_at + 3] = buf[3];
        let mut count: u32 = 0;
        let patch_at = self.output.len();
        self.write_u32(0);
        gc.weak_slots(&mut |weak_slot| unsafe {
            weak_slot.serialize(self);
            count += 1;
        });
        let buf = count.to_le_bytes();
        self.output[patch_at] = buf[0];
        self.output[patch_at + 1] = buf[1];
        self.output[patch_at + 2] = buf[2];
        self.output[patch_at + 3] = buf[3];
        vm.serialize(self);
    }

    pub fn get_gcpointer<T: GcCell + ?Sized>(&self, at: GcPointer<T>) -> u32 {
        self.reference_map
            .iter()
            .enumerate()
            .find(|x| x.1 == &(at.base.as_ptr() as usize))
            .unwrap_or_else(|| {
                panic!("No GC reference at {:p}", at.base.as_ptr());
            })
            .0 as u32
    }
    pub fn write_symbol(&mut self, sym: Symbol) {
        match sym {
            Symbol::Index(index) => {
                self.write_u8(0xff);
                self.write_u32(index);
            }
            Symbol::Key(id) => {
                if id < SymbolID::PUBLIC_START {
                    self.write_u8(0x1f);
                    self.write_u32(id.0);
                } else {
                    self.write_u8(0x2f);
                    let index = self.symbol_map.get(&sym).copied().unwrap();
                    self.write_u32(index);
                }
            }
            Symbol::Private(id) => {
                self.write_u8(0x3f);
                let index = self.symbol_map.get(&Symbol::Key(id)).copied().unwrap();
                self.write_u32(index);
            }
        }
    }
    pub fn write_weakref<T: GcCell + Sized>(&mut self, weak_ref: WeakRef<T>) {
        /*let key = weak_ref.inner.as_ptr() as usize;
        let ix = self
            .reference_map
            .iter()
            .enumerate()
            .find(|x| x.1 == &(key as usize))
            .unwrap()
            .0 as u32;
        self.write_u32(ix);*/
        weak_ref.slot.serialize(self);
    }
    pub fn write_gcpointer<T: GcCell + ?Sized>(&mut self, at: GcPointer<T>) {
        let reference = self.get_gcpointer(at);
        self.output
            .write_all(&reference.to_le_bytes())
            .unwrap_or_else(|_| {
                panic!(
                    "No GC reference for '{}' at {:p}",
                    at.get_dyn().type_name(),
                    at.base.as_ptr()
                );
            });
    }

    pub fn write_u64(&mut self, val: u64) {
        self.output.write_all(&val.to_le_bytes()).unwrap();
    }

    pub fn write_u32(&mut self, val: u32) {
        self.output.write_all(&val.to_le_bytes()).unwrap();
    }

    pub fn write_u16(&mut self, val: u16) {
        self.output.write_all(&val.to_le_bytes()).unwrap();
    }

    pub fn write_u8(&mut self, val: u8) {
        self.output.write_all(&val.to_le_bytes()).unwrap();
    }

    pub fn write_reference<T>(&mut self, ref_: *const T) {
        let ix = self
            .reference_map
            .iter()
            .enumerate()
            .find(|x| x.1 == &(ref_ as usize))
            .unwrap()
            .0 as u32;
        self.write_u32(ix);
    }

    pub fn try_write_reference<T>(&mut self, ref_: *const T) -> Option<()> {
        let ix = self
            .reference_map
            .iter()
            .enumerate()
            .find(|x| x.1 == &(ref_ as usize))?
            .0 as u32;
        self.write_u32(ix);
        Some(())
    }
}

use wtf_rs::segmented_vec::SegmentedVec;

use super::deserializer::Deserializable;

pub trait Serializable {
    fn serialize(&self, serializer: &mut SnapshotSerializer);
}

impl Serializable for JsValue {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        if self.is_object() {
            let object = self.get_object();
            serializer.output.push(0xff);
            serializer.write_gcpointer(object);
        } else {
            serializer.output.push(0x1f);
            serializer.write_u64(unsafe { std::mem::transmute(*self) });
        }
    }
}

impl Serializable for ArrayStorage {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        serializer.write_u32(self.size());
        serializer.write_u32(self.capacity());
        for i in 0..self.size() {
            let item = self.at(i);
            item.serialize(serializer);
        }
    }
}

impl<T: GcCell + ?Sized + 'static> Serializable for GcPointer<T> {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        serializer.write_gcpointer(*self);
    }
}
impl<T: GcCell> Serializable for WeakRef<T> {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        serializer.write_weakref(*self);
    }
}

impl Serializable for JsString {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        serializer.write_u32(self.len());
        for byte in self.as_str().bytes() {
            serializer.write_u8(byte);
        }
    }
}

impl Serializable for Symbol {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        serializer.write_symbol(*self);
    }
}

impl<T: Serializable> Serializable for Vec<T> {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        serializer.write_u64(self.len() as _);
        serializer.write_u64(self.capacity() as _);
        for item in self.iter() {
            item.serialize(serializer);
        }
    }
}

impl<K: Serializable, V: Serializable> Serializable for HashMap<K, V> {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        serializer.write_u64(self.len() as _);
        serializer.write_u64(self.capacity() as _);
        for (key, value) in self.iter() {
            key.serialize(serializer);
            value.serialize(serializer);
        }
    }
}

impl Serializable for String {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        serializer.write_u64(self.len() as _);
        serializer.write_u64(self.capacity() as _);
        for byte in self.bytes() {
            serializer.write_u8(byte);
        }
    }
}

impl Serializable for JsObject {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        serializer.write_u32(self.tag as _);
        serializer.write_reference(self.class);
        serializer.write_gcpointer(self.slots);
        serializer.write_gcpointer(self.structure);
        self.indexed.serialize(serializer);
        //serializer.write_gcpointer(self.indexed);
        serializer.write_u32(self.flags);
        match self.tag {
            ObjectTag::NormalArguments => {
                self.as_arguments().serialize(serializer);
            }
            ObjectTag::Global => {
                self.as_global().serialize(serializer);
            }
            ObjectTag::Function => {
                self.as_function().serialize(serializer);
            }
            ObjectTag::String => {
                self.as_string_object().serialize(serializer);
            }
            _ => (),
        }
        if let Some(ser) = self.class.serialize {
            ser(self, serializer);
        }
    }
}

impl<T: Deserializable + Serializable> Serializable for Option<T> {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        match self {
            Some(item) => {
                serializer.write_u8(0x01);
                item.serialize(serializer);
            }
            None => {
                serializer.write_u8(0x0);
            }
        }
    }
}

impl Serializable for JsFunction {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        self.construct_struct.serialize(serializer);
        self.ctx.serialize(serializer);
        match &self.ty {
            FuncType::User(vm) => {
                serializer.write_u8(0x01);
                vm.scope.serialize(serializer);
                vm.code.serialize(serializer);
            }
            FuncType::Native(native_fn) => {
                serializer.write_u8(0x02);
                serializer.write_reference(native_fn.func as *const u8);
            }
            FuncType::Closure(_) => {
                panic!("Cannot serialize a Function based on a rust closure");
            }
            FuncType::Bound(bound_fn) => {
                serializer.write_u8(0x03);
                bound_fn.args.serialize(serializer);
                bound_fn.target.serialize(serializer);
                bound_fn.this.serialize(serializer);
            }
            FuncType::Generator(gen_fn) => {
                serializer.write_u8(0x04);
                gen_fn.function.serialize(serializer);
            }
        }
    }
}
impl Serializable for bool {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        if *self {
            serializer.write_u8(0x01);
        } else {
            serializer.write_u8(0x00);
        }
    }
}
impl Serializable for u8 {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        serializer.write_u8(*self);
    }
}

impl Serializable for u32 {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        serializer.write_u32(*self);
    }
}

impl Serializable for TypeFeedBack {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        match self {
            TypeFeedBack::PropertyCache {
                structure,
                offset,
                mode,
            } => {
                serializer.write_u8(0x01);
                serializer.write_gcpointer(*structure);
                serializer.write_u32(*offset);
                match mode {
                    &crate::bytecode::GetByIdMode::ArrayLength => serializer.write_u8(0),
                    &crate::bytecode::GetByIdMode::Default => serializer.write_u8(1),
                    &crate::bytecode::GetByIdMode::ProtoLoad(slot) => {
                        serializer.write_u8(2);
                        slot.serialize(serializer);
                    }
                }
            }
            &TypeFeedBack::StructureCache { structure } => {
                serializer.write_u8(0x02);
                serializer.write_gcpointer(structure);
            }
            &TypeFeedBack::PutByIdFeedBack {
                new_structure,
                old_structure,
                offset,
                structure_chain,
            } => {
                serializer.write_u8(0x03);
                new_structure.serialize(serializer);
                old_structure.serialize(serializer);
                offset.serialize(serializer);
                structure_chain.serialize(serializer);
            }
            _ => {
                // other type feedback is ignored
                serializer.write_u8(0x0);
            }
        }
    }
}

impl Serializable for CodeBlock {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        self.name.serialize(serializer);
        self.names.serialize(serializer);
        self.strict.serialize(serializer);

        self.code.serialize(serializer);
        self.feedback.serialize(serializer);
        self.literals.serialize(serializer);

        self.codes.serialize(serializer);
        self.top_level.serialize(serializer);
        self.use_arguments.serialize(serializer);
        self.file_name.serialize(serializer);

        self.rest_at.serialize(serializer);
        self.var_count.serialize(serializer);
        self.param_count.serialize(serializer);
        self.args_at.serialize(serializer);
        self.is_constructor.serialize(serializer);

        (self.loc.len() as u32).serialize(serializer);
        for (range, loc) in self.loc.iter() {
            (range.start as u32).serialize(serializer);
            (range.end as u32).serialize(serializer);
            loc.line.serialize(serializer);
            loc.col.serialize(serializer);
        }
        self.path.to_string().serialize(serializer);
        self.is_generator.serialize(serializer);
        self.is_async.serialize(serializer);
        self.stack_size.serialize(serializer);
    }
}

impl Serializable for AttrSafe {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        self.raw().serialize(serializer);
    }
}

impl Serializable for MapEntry {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        self.offset.serialize(serializer);
        self.attrs.serialize(serializer);
    }
}

impl Serializable for TransitionKey {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        self.name.serialize(serializer);
        self.attrs.serialize(serializer);
    }
}
impl Serializable for Transition {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        match self {
            Self::None => {
                serializer.write_u8(0x0);
            }
            Self::Table(table) => {
                serializer.write_u8(0x01);
                table.serialize(serializer);
            }
            Self::Pair(key, structure) => {
                serializer.write_u8(0x02);
                key.serialize(serializer);
                structure.serialize(serializer);
            }
        }
    }
}

impl Serializable for TransitionsTable {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        self.var.serialize(serializer);
        self.enabled.serialize(serializer);
        self.unique.serialize(serializer);
        self.indexed.serialize(serializer);
    }
}

impl Serializable for DeletedEntry {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        self.prev.serialize(serializer);
        self.offset.serialize(serializer);
    }
}

impl Serializable for DeletedEntryHolder {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        self.entry.serialize(serializer);
        self.size.serialize(serializer);
    }
}

impl Serializable for Structure {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        self.transitions.serialize(serializer);
        self.table.serialize(serializer);
        self.deleted.serialize(serializer);
        self.added.0.serialize(serializer);
        self.added.1.serialize(serializer);
        self.previous.serialize(serializer);
        self.prototype.serialize(serializer);
        self.calculated_size.serialize(serializer);
        self.transit_count.serialize(serializer);
        self.has_been_flattened_before.serialize(serializer);
        self.cached_prototype_chain.serialize(serializer);
    }
}

impl<T: Serializable> Serializable for SegmentedVec<T> {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        serializer.write_u64(self.len() as _);
        for item in self.iter() {
            item.serialize(serializer);
        }
    }
}
impl Serializable for StoredSlot {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        self.value.serialize(serializer);
        self.attributes.serialize(serializer);
    }
}
impl Serializable for JsGlobal {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        self.ctx.serialize(serializer);
        self.sym_map.serialize(serializer);
        self.variables.serialize(serializer);
    }
}

impl<T: Serializable> Serializable for &[T] {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        serializer.write_u64(self.len() as _);
        for x in self.iter() {
            x.serialize(serializer);
        }
    }
}
impl<T: Serializable> Serializable for Box<T> {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        (**self).serialize(serializer);
    }
}
impl Serializable for JsArguments {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        (&*self.mapping).serialize(serializer);
        self.env.serialize(serializer);
    }
}

impl Serializable for IndexedElements {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        self.vector.serialize(serializer);
        self.map.serialize(serializer);
        self.length.serialize(serializer);
        self.flags.serialize(serializer);
    }
}

impl Serializable for f64 {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        serializer.write_u64(self.to_bits());
    }
}

impl Serializable for f32 {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        serializer.write_u32(self.to_bits());
    }
}

impl Serializable for i8 {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        serializer.write_u8(*self as u8);
    }
}

impl Serializable for u16 {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        serializer.write_u16(*self);
    }
}

impl Serializable for i16 {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        serializer.write_u16(*self as u16);
    }
}

impl Serializable for i32 {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        serializer.write_u32(*self as u32);
    }
}

impl Serializable for i64 {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        serializer.write_u64(*self as u64);
    }
}

impl Serializable for u64 {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        serializer.write_u64(*self);
    }
}
/*
impl Serializable for Arguments {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        self.ctor_call.serialize(serializer);
        self.this.serialize(serializer);
        self.values.serialize(serializer);
    }
}
*/
impl Serializable for Accessor {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        self.getter.serialize(serializer);
        self.setter.serialize(serializer);
    }
}

impl Serializable for SpreadValue {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        self.array.serialize(serializer);
    }
}

impl Serializable for Slot {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        self.parent.serialize(serializer);
        self.base.serialize(serializer);
        self.offset.serialize(serializer);
        self.flags.serialize(serializer);
    }
}

impl Serializable for JsSymbol {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        self.symbol().serialize(serializer);
    }
}

impl Serializable for GlobalData {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        self.normal_arguments_structure.serialize(serializer);
        self.empty_object_struct.serialize(serializer);
        self.function_struct.serialize(serializer);
        self.object_prototype.serialize(serializer);
        self.number_prototype.serialize(serializer);
        self.string_prototype.serialize(serializer);
        self.boolean_prototype.serialize(serializer);
        self.symbol_prototype.serialize(serializer);
        self.error.serialize(serializer);
        self.type_error.serialize(serializer);
        self.uri_error.serialize(serializer);
        self.reference_error.serialize(serializer);
        self.range_error.serialize(serializer);
        self.syntax_error.serialize(serializer);
        self.internal_error.serialize(serializer);
        self.eval_error.serialize(serializer);
        self.array_prototype.serialize(serializer);
        self.func_prototype.serialize(serializer);
        self.string_structure.serialize(serializer);
        self.number_structure.serialize(serializer);
        self.array_structure.serialize(serializer);
        self.error_structure.serialize(serializer);
        self.range_error_structure.serialize(serializer);
        self.reference_error_structure.serialize(serializer);
        self.syntax_error_structure.serialize(serializer);
        self.type_error_structure.serialize(serializer);
        self.uri_error_structure.serialize(serializer);
        self.eval_error_structure.serialize(serializer);
        self.map_prototype.serialize(serializer);
        self.map_structure.serialize(serializer);
        self.set_prototype.serialize(serializer);
        self.set_structure.serialize(serializer);
        self.regexp_structure.serialize(serializer);
        self.regexp_prototype.serialize(serializer);
        self.generator_prototype.serialize(serializer);
        self.generator_structure.serialize(serializer);
        self.array_buffer_prototype.serialize(serializer);
        self.array_buffer_structure.serialize(serializer);
        self.data_view_structure.serialize(serializer);
        self.data_view_prototype.serialize(serializer);
        self.spread_builtin.serialize(serializer);
        self.weak_ref_structure.serialize(serializer);
        self.weak_ref_prototype.serialize(serializer);
        self.object_constructor.serialize(serializer);
        self.symbol_structure.serialize(serializer);
        self.date_structure.serialize(serializer);
        self.boolean_structure.serialize(serializer);
        self.date_prototype.serialize(serializer);

        serializer.write_u32(self.custom_structures.len() as u32);
        self.custom_structures.iter().for_each(|(name, structure)| {
            name.serialize(serializer);
            structure.serialize(serializer);
        });
    }
}

impl Serializable for VirtualMachine {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        let ctx_num = self.contexts.len();
        serializer.write_u32(ctx_num as u32);
        self.contexts.iter().for_each(|ctx| {
            ctx.serialize(serializer);
        });
    }
}

impl Serializable for Context {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        self.global_data.serialize(serializer);
        self.global_object.serialize(serializer);
        self.symbol_table.serialize(serializer);
        self.module_loader.serialize(serializer);
        self.modules.serialize(serializer);
    }
}
