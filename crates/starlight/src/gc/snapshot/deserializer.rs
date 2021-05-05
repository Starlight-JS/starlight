/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
#![allow(unused_variables)]
use vm::function::JsFunction;
use wtf_rs::segmented_vec::SegmentedVec;
macro_rules! unique {
    () => {};
}
use crate::{
    bytecode::TypeFeedBack,
    gc::cell::{vtable_of_type, GcCell, GcPointer, GcPointerBase, WeakRef},
    gc::Heap,
    jsrt::VM_NATIVE_REFERENCES,
    prelude::Class,
    vm::{
        self,
        arguments::JsArguments,
        array_storage::ArrayStorage,
        code_block::{CodeBlock, FileLocation},
        function::{FuncType, JsBoundFunction, JsNativeFunction, JsVMFunction},
        global::JsGlobal,
        indexed_elements::{IndexedElements, SparseArrayMap},
        interpreter::SpreadValue,
        object::{object_size_with_tag, JsObject, ObjectTag},
        property_descriptor::{Accessor, StoredSlot},
        string::{JsString, JsStringObject},
        structure::{
            DeletedEntry, DeletedEntryHolder, MapEntry, Structure, TargetTable, Transition,
            TransitionKey, TransitionsTable,
        },
        structure_chain::StructureChain,
        symbol_table::*,
        symbol_table::{JsSymbol, Symbol, SymbolID},
        value::*,
        Runtime, *,
    },
};
use std::{
    any::TypeId,
    collections::HashMap,
    hash::Hash,
    hint::unreachable_unchecked,
    mem::size_of,
    mem::{transmute, ManuallyDrop},
};

pub struct Deserializer<'a> {
    rt: *mut Runtime,
    reader: &'a [u8],
    pc: usize,
    reference_map: Vec<usize>,
    symbol_map: Vec<Symbol>,
    log_deser: bool,
}

impl<'a> Deserializer<'a> {
    pub fn get_u32(&mut self) -> u32 {
        let mut buf = [0; 4];
        unsafe {
            buf[0] = *self.reader.get_unchecked(self.pc);
            buf[1] = *self.reader.get_unchecked(self.pc + 1);
            buf[2] = *self.reader.get_unchecked(self.pc + 2);
            buf[3] = *self.reader.get_unchecked(self.pc + 3);
        }
        self.pc += 4;
        u32::from_le_bytes(buf)
    }

    pub fn get_u8(&mut self) -> u8 {
        self.pc += 1;
        self.reader[self.pc - 1]
    }

    pub fn get_u16(&mut self) -> u16 {
        let mut buf = [0; 2];
        unsafe {
            buf[0] = *self.reader.get_unchecked(self.pc);
            buf[1] = *self.reader.get_unchecked(self.pc + 1);
        }
        self.pc += 2;
        u16::from_le_bytes(buf)
    }

    pub fn get_u64(&mut self) -> u64 {
        let mut buf = [0; 8];
        unsafe {
            buf[0] = *self.reader.get_unchecked(self.pc);
            buf[1] = *self.reader.get_unchecked(self.pc + 1);
            buf[2] = *self.reader.get_unchecked(self.pc + 2);
            buf[3] = *self.reader.get_unchecked(self.pc + 3);
            buf[4] = *self.reader.get_unchecked(self.pc + 4);
            buf[5] = *self.reader.get_unchecked(self.pc + 5);
            buf[6] = *self.reader.get_unchecked(self.pc + 6);
            buf[7] = *self.reader.get_unchecked(self.pc + 7);
        }
        self.pc += 8;
        u64::from_le_bytes(buf)
    }

    pub fn get_reference(&mut self) -> *const u8 {
        let index = self.get_u32();
        unsafe { (*self.reference_map.get_unchecked(index as usize)) as *const u8 }
        //  unwrap_unchecked(self.reference_map.get(&index).copied()) as *const u8
    }

    unsafe fn build_reference_map(&mut self, rt: &mut Runtime) {
        let mut ix = 0;
        VM_NATIVE_REFERENCES
            .iter()
            .enumerate()
            .for_each(|(index, reference)| {
                //let index = self.reference_map.len();
                *self.reference_map.get_mut(ix).unwrap() = *reference;
                ix += 1;
            });

        if let Some(ref references) = rt.external_references {
            for reference in references.iter() {
                *self.reference_map.get_mut(ix as usize).unwrap() = *reference;
                ix += 1;
                //self.reference_map.insert(ix as u32, *reference);
            }
        }
    }

    unsafe fn build_symbol_table(&mut self) {
        let count = self.get_u32();
        self.symbol_map = vec![DUMMY_SYMBOL; count as usize];
        for _ in 0..count {
            let index = self.get_u32();
            let len = self.get_u32();
            /*let mut bytes = vec![];
            for _ in 0..len {
                bytes.push(self.get_u8());
            }
            let sym = String::from_utf8_unchecked(bytes).intern();*/
            let sym = std::str::from_utf8_unchecked(&self.reader[self.pc..self.pc + len as usize])
                .intern();
            self.pc += len as usize;
            *self.symbol_map.get_mut(index as usize).unwrap() = sym;
        }
    }

    unsafe fn deserialize_internal(&mut self, rt: &mut Runtime) {
        let count = self.get_u32();
        let heap_at = self.pc;
        logln_if!(self.log_deser, "- Object pre-allocation started -");
        for _ in 0..count {
            let ref_id = self.get_u32();
            let _deser = self.get_reference();
            let alloc = transmute::<_, fn(&mut Runtime, &mut Self) -> *mut GcPointerBase>(
                self.get_reference(),
            );
            let offset = self.get_u32();
            let ptr = alloc(rt, self);
            logln_if!(
                self.log_deser,
                "pre allocated reference #{} '{}' at {:p}",
                ref_id,
                (*ptr).get_dyn().type_name(),
                ptr
            );
            self.pc = offset as usize;
            *self.reference_map.get_mut(ref_id as usize).unwrap() = ptr as usize;
        }
        logln_if!(self.log_deser, "- Object pre-allocated completed -");
        let weak_count = self.get_u32();
        logln_if!(self.log_deser, "- Weak slot deserialization started -");
        for _ in 0..weak_count {
            let is_null = self.get_u8() == 0x0;
            let ptr = if is_null {
                0 as *const u8
            } else {
                self.get_reference()
            };

            let index = self.get_u32();

            logln_if!(self.log_deser, "make weak #{} {:p}", index, ptr);
            let slot = rt.heap().make_weak_slot(ptr as *mut _);
            *self.reference_map.get_mut(index as usize).unwrap() = slot as usize;
        }
        logln_if!(self.log_deser, "- Weak slot deserialization completed -");
        let last_stop = self.pc;
        self.pc = heap_at;
        logln_if!(self.log_deser, "- Object deserialization started -");
        for _ in 0..count {
            let ref_id = self.get_u32();
            let base = *self.reference_map.get_mut(ref_id as usize).unwrap();
            logln_if!(
                self.log_deser,
                "deserialize #{}:0x{:x} '{}'",
                ref_id,
                base,
                (*(base as *mut GcPointerBase)).get_dyn().type_name()
            );
            let _deser = transmute::<_, fn(*mut u8, &mut Self)>(self.get_reference());
            let _alloc = self.get_reference();
            let _off = self.get_u32();
            let data = (*(base as *mut GcPointerBase)).data::<u8>();
            _deser(data, self);
        }
        logln_if!(self.log_deser, "- Object deserialization completed -");
        self.pc = last_stop;

        rt.global_data = self.deserialize_global_data();
        rt.global_object = self.read_opt_gc();
        rt.symbol_table = HashMap::<Symbol, GcPointer<JsSymbol>>::deserialize_inplace(self);
        rt.module_loader = self.read_opt_gc();
        rt.modules = HashMap::<String, ModuleKind>::deserialize_inplace(self);
    }
    unsafe fn read_opt_gc<T: GcCell>(&mut self) -> Option<GcPointer<T>> {
        Option::<GcPointer<T>>::deserialize_inplace(self)
    }
    unsafe fn deserialize_global_data(&mut self) -> GlobalData {
        GlobalData {
            normal_arguments_structure: self.read_opt_gc(),
            empty_object_struct: self.read_opt_gc(),
            function_struct: self.read_opt_gc(),
            object_prototype: self.read_opt_gc(),
            number_prototype: self.read_opt_gc(),
            string_prototype: self.read_opt_gc(),
            boolean_prototype: self.read_opt_gc(),
            symbol_prototype: self.read_opt_gc(),
            error: self.read_opt_gc(),
            type_error: self.read_opt_gc(),
            reference_error: self.read_opt_gc(),
            range_error: self.read_opt_gc(),
            syntax_error: self.read_opt_gc(),
            internal_error: self.read_opt_gc(),
            eval_error: self.read_opt_gc(),
            array_prototype: self.read_opt_gc(),
            func_prototype: self.read_opt_gc(),
            string_structure: self.read_opt_gc(),
            number_structure: self.read_opt_gc(),
            array_structure: self.read_opt_gc(),
            error_structure: self.read_opt_gc(),
            range_error_structure: self.read_opt_gc(),
            reference_error_structure: self.read_opt_gc(),
            syntax_error_structure: self.read_opt_gc(),
            type_error_structure: self.read_opt_gc(),
            uri_error_structure: self.read_opt_gc(),
            eval_error_structure: self.read_opt_gc(),
            map_prototype: self.read_opt_gc(),
            map_structure: self.read_opt_gc(),
            set_prototype: self.read_opt_gc(),
            set_structure: self.read_opt_gc(),
            regexp_structure: self.read_opt_gc(),
            regexp_object: self.read_opt_gc(),
        }
    }
    /// Deserialize JS runtime from snapshot buffer. If snapshot has external references that is not part of the VM i.e some native function
    /// was used in snapshot it should be there too.
    /// ```rust,ignore
    ///
    /// fn my_native_fun(rt: &mut Runtime,args: &Arguments) -> Result<JsValue,JsValue> {...}
    /// fn another_native_fun(rt: &mut Runtime,args: &Arguments) -> Result<JsValue,JsValue> {...}
    ///
    /// let native_refs = Box::leak(Box::new([my_naive_fun as usize,another_native_fun as usize]));
    /// let mut rt = Runtime::new(false,Some(native_refs));
    ///
    /// let snapshot = Snapshot::take(&mut rt);
    ///
    /// // Note that native references should be passed in the same order as in serialized runtime instance, otherwise
    /// // this will lead to UB or segfault or wrong function ordering.
    /// let rt2 = Deserializer::deserialize(false,&snapshot.buffer,Some(native_refs));
    ///
    ///
    /// ```
    pub fn deserialize(
        log_deser: bool,
        snapshot: &'a [u8],
        options: RuntimeParams,
        gc: Heap,
        external_refs: Option<&'static [usize]>,
        callback: impl FnOnce(&mut Self, &mut Runtime),
    ) -> Box<Runtime> {
        let mut runtime = Runtime::new_empty(gc, options, external_refs);
        let mut this = Self {
            reader: snapshot,
            pc: 0,
            log_deser,
            symbol_map: Default::default(),
            reference_map: Default::default(),
            rt: &mut *runtime,
        };
        runtime.heap().defer();
        unsafe {
            let ref_count = this.get_u32();

            this.reference_map = vec![0; ref_count as usize];
            this.build_reference_map(&mut runtime);
            this.build_symbol_table();
            this.deserialize_internal(&mut runtime);
            callback(&mut this, &mut runtime);
        }
        runtime.heap().undefer();
        runtime
    }
}

pub trait Deserializable {
    unsafe fn dummy_read(deser: &mut Deserializer) {
        unreachable!()
    }
    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self;
    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer);
    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase;
}

impl Deserializable for JsValue {
    unsafe fn dummy_read(deser: &mut Deserializer) {
        let ty = deser.get_u8();
        if ty == 0xff {
            deser.get_u32();
        } else {
            deser.get_u64();
        }
    }
    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        let ty = deser.get_u8();
        let val = if ty == 0xff {
            JsValue::encode_object_value(std::mem::transmute::<_, GcPointer<u8>>(
                deser.get_reference(),
            ))
        } else {
            std::mem::transmute::<_, JsValue>(deser.get_u64())
        };
        val
    }
    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        let val = Self::deserialize_inplace(deser);
        if !at.is_null() {
            at.cast::<JsValue>().write(val);
        }
    }
    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        //Self::dummy_read(deser);
        rt.gc
            .allocate_raw(vtable_of_type::<Self>() as _, 8, TypeId::of::<Self>())
    }
}

impl Deserializable for ArrayStorage {
    unsafe fn dummy_read(deser: &mut Deserializer) {
        let size = deser.get_u32();
        deser.get_u32();
        for _ in 0..size {
            JsValue::dummy_read(deser);
        }
    }
    unsafe fn deserialize_inplace(_deser: &mut Deserializer) -> Self {
        unreachable!()
    }

    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        let _size = deser.get_u32();
        let capacity = deser.get_u32();
        deser.pc -= 8;
        rt.gc.allocate_raw(
            vtable_of_type::<ArrayStorage>() as _,
            size_of::<ArrayStorage>() + (capacity as usize * 8),
            TypeId::of::<Self>(),
        )
    }

    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        let size = deser.get_u32();
        let capacity = deser.get_u32();
        let raw = ArrayStorage {
            size,
            capacity,
            data: [],
        };
        at.cast::<ArrayStorage>().write(raw);
        let mut array =
            std::mem::transmute::<_, GcPointer<ArrayStorage>>(at.sub(size_of::<GcPointerBase>()));
        for i in 0..size {
            let val = JsValue::deserialize_inplace(deser);
            *array.at_mut(i) = val;
        }
    }
}

impl<T: GcCell + ?Sized> Deserializable for GcPointer<T> {
    unsafe fn dummy_read(deser: &mut Deserializer) {
        deser.get_u32();
    }
    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        std::mem::transmute(deser.get_reference())
    }

    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        //  deser.get_u32();
        rt.heap().allocate_raw(
            vtable_of_type::<GcPointer<T>>() as _,
            size_of::<usize>(),
            TypeId::of::<Self>(),
        )
    }

    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        let this = Self::deserialize_inplace(deser);
        at.cast::<GcPointer<T>>().write(this);
    }
}

impl Deserializable for JsString {
    unsafe fn dummy_read(deser: &mut Deserializer) {
        let sz = deser.get_u32();
        for _ in 0..sz {
            deser.get_u8();
        }
    }
    unsafe fn deserialize_inplace(_deser: &mut Deserializer) -> Self {
        unreachable!()
    }
    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        let size = deser.get_u32();
        let mut bytes = Vec::with_capacity(size as _);
        for _ in 0..size {
            bytes.push(deser.get_u8());
        }

        at.cast::<JsString>().write(JsString {
            string: String::from_utf8_unchecked(bytes),
        })
    }
    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        let size = deser.get_u32();
        for _ in 0..size {
            deser.get_u8();
        }
        rt.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            size_of::<JsString>(),
            TypeId::of::<Self>(),
        )
    }
}

impl Deserializable for Symbol {
    unsafe fn dummy_read(deser: &mut Deserializer) {
        deser.get_u8();
        deser.get_u32();
    }
    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        let ty = deser.get_u8();
        match ty {
            0xff => {
                at.cast::<Symbol>().write(Symbol::Index(deser.get_u32()));
            }
            0x1f => {
                at.cast::<Symbol>()
                    .write(Symbol::Key(SymbolID(deser.get_u32())));
            }
            0x2f => {
                let ix = deser.get_u32();
                at.cast::<Symbol>()
                    .write(*deser.symbol_map.get_unchecked(ix as usize));
            }
            0x3f => {
                let ix = deser.get_u32();
                at.cast::<Symbol>()
                    .write(deser.symbol_map.get_unchecked(ix as usize).private());
            }
            _ => unreachable_unchecked(),
        }
    }
    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        let ty = deser.get_u8();
        match ty {
            0xff => Symbol::Index(deser.get_u32()),
            0x1f => Symbol::Key(SymbolID(deser.get_u32())),
            0x2f => {
                let ix = deser.get_u32();
                *deser.symbol_map.get_unchecked(ix as usize)
            }
            0x3f => {
                let ix = deser.get_u32();
                deser.symbol_map.get_unchecked(ix as usize).private()
            }
            _ => unreachable!("unknown symbol {:x}", ty),
        }
    }

    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        deser.get_u8();
        deser.get_u32();
        rt.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            size_of::<Symbol>(),
            TypeId::of::<Self>(),
        )
    }
}

impl<T: Deserializable + GcCell> Deserializable for Vec<T> {
    unsafe fn dummy_read(deser: &mut Deserializer) {
        let len = deser.get_u64();
        deser.get_u64();
        for _ in 0..len {
            T::dummy_read(deser);
        }
    }
    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        let len = deser.get_u64();
        let cap = deser.get_u64();
        let mut this = Self::with_capacity(cap as _);
        for _ in 0..len {
            this.push(T::deserialize_inplace(deser));
        }

        this
    }

    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        let this = Self::deserialize_inplace(deser);
        at.cast::<Vec<T>>().write(this);
    }

    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        /*      let len = deser.get_u64();
                deser.get_u64();
                for _ in 0..len {
                    T::dummy_read(deser);
                }
        */
        rt.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            size_of::<Self>(),
            TypeId::of::<Self>(),
        )
    }
}

impl<K: Deserializable + GcCell + Eq + Hash, V: Deserializable + GcCell> Deserializable
    for HashMap<K, V>
{
    unsafe fn dummy_read(deser: &mut Deserializer) {
        let len = deser.get_u64();
        let _cap = deser.get_u64();
        for _ in 0..len {
            K::dummy_read(deser);
            V::dummy_read(deser);
        }
    }

    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        let len = deser.get_u64();
        let cap = deser.get_u64();
        let mut this = Self::with_capacity(cap as _);
        for _ in 0..len {
            let key = K::deserialize_inplace(deser);
            let val = V::deserialize_inplace(deser);
            this.insert(key, val);
        }
        this
    }

    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        let this = Self::deserialize_inplace(deser);
        at.cast::<Self>().write(this);
    }

    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        //Self::dummy_read(deser);
        rt.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            size_of::<Self>(),
            TypeId::of::<Self>(),
        )
    }
}
impl Deserializable for String {
    unsafe fn dummy_read(deser: &mut Deserializer) {
        let len = deser.get_u64();
        let _ = deser.get_u64();

        for _ in 0..len {
            deser.get_u8();
        }
    }

    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        let len = deser.get_u64();
        let capacity = deser.get_u64();
        let mut bytes = Vec::with_capacity(capacity as _);
        for _ in 0..len {
            bytes.push(deser.get_u8());
        }
        String::from_utf8_unchecked(bytes)
    }

    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        let this = Self::deserialize_inplace(deser);
        at.cast::<Self>().write(this);
    }

    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        //Self::dummy_read(deser);
        rt.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            size_of::<Self>(),
            TypeId::of::<Self>(),
        )
    }
}

impl Deserializable for StoredSlot {
    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        let value = JsValue::deserialize_inplace(deser);
        let attributes = transmute(deser.get_u32());
        StoredSlot { attributes, value }
    }

    unsafe fn dummy_read(deser: &mut Deserializer) {
        JsValue::dummy_read(deser);
        deser.get_u32();
    }

    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        let this = Self::deserialize_inplace(deser);
        at.cast::<Self>().write(this);
    }

    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        unreachable!()
    }
}

impl<T: Deserializable> Deserializable for SegmentedVec<T> {
    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        let len = deser.get_u64();
        let mut vec = SegmentedVec::new();
        for _ in 0..len {
            vec.push(T::deserialize_inplace(deser));
        }
        vec
    }

    unsafe fn dummy_read(deser: &mut Deserializer) {
        let len = deser.get_u64();
        for _ in 0..len {
            T::dummy_read(deser);
        }
    }

    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        unreachable!();
    }
    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        at.cast::<Self>().write(Self::deserialize_inplace(deser));
    }
}

impl Deserializable for u32 {
    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        at.cast::<u32>().write(deser.get_u32())
    }

    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        deser.get_u32()
    }
    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        //   deser.get_u32();
        rt.heap()
            .allocate_raw(vtable_of_type::<Self>() as _, 4, TypeId::of::<Self>())
    }
    unsafe fn dummy_read(deser: &mut Deserializer) {
        deser.get_u32();
    }
}

impl<T: Deserializable + GcCell> Deserializable for Option<T> {
    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        let is_empty = deser.get_u8();
        if is_empty == 0x0 {
            return None;
        }
        assert!(
            is_empty == 0x01,
            "option tag does not exist '{:x}",
            is_empty
        );
        let val = T::deserialize_inplace(deser);
        Some(val)
    }
    unsafe fn dummy_read(deser: &mut Deserializer) {
        let x = deser.get_u8();
        if x == 0x01 {
            T::dummy_read(deser);
        }
    }
    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        unreachable!()
    }
    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        at.cast::<Self>().write(Self::deserialize_inplace(deser));
    }
}

impl Deserializable for JsObject {
    unsafe fn dummy_read(deser: &mut Deserializer) {}
    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        unreachable!()
    }

    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        let tag = transmute::<_, ObjectTag>(deser.get_u32() as u8);
        let class = deser.get_reference();
        let slots = deser.get_reference();
        let structure = deser.get_reference();
        let indexed = IndexedElements::deserialize_inplace(deser);
        let flags = deser.get_u32();
        let object = at.cast::<JsObject>();
        object.write(Self {
            tag,
            class: transmute(class),
            slots: transmute(slots),
            structure: transmute(structure),
            indexed: transmute(indexed),
            flags,
            object_data_start: 0,
        });

        match tag {
            ObjectTag::NormalArguments => {
                let size = deser.get_u64();
                let mut vec = Vec::with_capacity(size as _);
                for _ in 0..size {
                    vec.push(Symbol::deserialize_inplace(deser));
                }
                let env = deser.get_reference();
                ((*object).data::<JsArguments>() as *mut ManuallyDrop<_> as *mut JsArguments).write(
                    JsArguments {
                        env: transmute(env),
                        mapping: vec.into_boxed_slice(),
                    },
                )
            }
            ObjectTag::Function => {
                let construct_struct = Option::<GcPointer<Structure>>::deserialize_inplace(deser);
                let ty = deser.get_u8();
                let val = match ty {
                    0x01 => {
                        let scope = deser.get_reference();
                        let code = deser.get_reference();

                        FuncType::User(JsVMFunction {
                            scope: transmute(scope),
                            code: transmute(code),
                        })
                    }
                    0x02 => {
                        let func = deser.get_reference();
                        FuncType::Native(JsNativeFunction {
                            func: transmute(func),
                        })
                    }

                    0x03 => {
                        let args = deser.get_reference();
                        let target = deser.get_reference();
                        let this = JsValue::deserialize_inplace(deser);
                        FuncType::Bound(JsBoundFunction {
                            args: transmute(args),
                            target: transmute(target),
                            this,
                        })
                    }
                    _ => unreachable!(),
                };

                ((*object).data::<JsFunction>() as *mut ManuallyDrop<_> as *mut JsFunction).write(
                    JsFunction {
                        construct_struct,
                        ty: val,
                    },
                )
            }
            ObjectTag::Global => {
                let sym_map = HashMap::<Symbol, u32>::deserialize_inplace(deser);
                let variables = SegmentedVec::<StoredSlot>::deserialize_inplace(deser);
                ((*object).data::<JsGlobal>() as *mut ManuallyDrop<_> as *mut JsGlobal).write(
                    JsGlobal {
                        vm: deser.rt,
                        sym_map,
                        variables,
                    },
                )
            }
            ObjectTag::String => {
                let str = GcPointer::<JsString>::deserialize_inplace(deser);

                ((*object).data::<JsStringObject>() as *mut ManuallyDrop<_> as *mut JsStringObject)
                    .write(JsStringObject { value: str })
            }

            _ => (),
        }
        if let Some(deser_fn) = (*object).class.deserialize {
            let rt = deser.rt;
            deser_fn(&mut *object, deser, &mut *rt);
        }
    }

    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        let tag = transmute(deser.get_u32() as u8);
        let class: &'static Class = transmute(deser.get_reference());
        deser.pc -= 8;
        rt.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            object_size_with_tag(tag, class),
            TypeId::of::<Self>(),
        )
    }
}

impl Deserializable for IndexedElements {
    unsafe fn dummy_read(deser: &mut Deserializer) {
        deser.get_u32();
        Option::<GcPointer<SparseArrayMap>>::dummy_read(deser);
        deser.get_u32();
        deser.get_u32();
    }
    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        let this = Self::deserialize_inplace(deser);
        at.cast::<Self>().write(this);
    }

    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        let vector = deser.get_reference();
        let map = Option::<GcPointer<SparseArrayMap>>::deserialize_inplace(deser);
        let length = deser.get_u32();
        let flags = deser.get_u32();
        Self {
            vector: transmute(vector),
            map,
            length,
            flags,
            non_gc: true,
        }
    }

    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        //Self::dummy_read(deser);
        rt.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            size_of::<Self>(),
            TypeId::of::<Self>(),
        )
    }
}

impl Deserializable for Accessor {
    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        let getter = JsValue::deserialize_inplace(deser);
        let setter = JsValue::deserialize_inplace(deser);
        Self { getter, setter }
    }

    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        at.cast::<Self>().write(Self::deserialize_inplace(deser));
    }
    unsafe fn dummy_read(deser: &mut Deserializer) {
        JsValue::dummy_read(deser);
        JsValue::dummy_read(deser);
    }

    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        //Self::dummy_read(deser);
        rt.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            size_of::<Self>(),
            TypeId::of::<Self>(),
        )
    }
}

impl Deserializable for SpreadValue {
    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        let arr = Vec::<JsValue>::deserialize_inplace(deser);
        Self {
            array: transmute(arr),
        }
    }
    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        at.cast::<Self>().write(Self::deserialize_inplace(deser));
    }

    unsafe fn dummy_read(deser: &mut Deserializer) {
        deser.get_u32();
    }
    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        //Self::dummy_read(deser);
        rt.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            size_of::<Self>(),
            TypeId::of::<Self>(),
        )
    }
}

impl Deserializable for bool {
    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        let val = deser.get_u8();
        val != 0
    }
    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        at.cast::<Self>().write(Self::deserialize_inplace(deser));
    }

    unsafe fn dummy_read(deser: &mut Deserializer) {
        deser.get_u8();
    }
    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        //Self::dummy_read(deser);
        rt.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            size_of::<Self>(),
            TypeId::of::<Self>(),
        )
    }
}

impl Deserializable for u8 {
    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        at.cast::<Self>().write(Self::deserialize_inplace(deser));
    }

    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        deser.get_u8()
    }
    unsafe fn dummy_read(deser: &mut Deserializer) {
        deser.get_u8();
    }
    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        //Self::dummy_read(deser);
        rt.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            size_of::<Self>(),
            TypeId::of::<Self>(),
        )
    }
}
impl Deserializable for u16 {
    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        at.cast::<Self>().write(Self::deserialize_inplace(deser));
    }

    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        deser.get_u16()
    }
    unsafe fn dummy_read(deser: &mut Deserializer) {
        deser.get_u16();
    }
    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        //Self::dummy_read(deser);
        rt.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            size_of::<Self>(),
            TypeId::of::<Self>(),
        )
    }
}

impl Deserializable for u64 {
    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        at.cast::<Self>().write(Self::deserialize_inplace(deser));
    }

    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        deser.get_u64()
    }
    unsafe fn dummy_read(deser: &mut Deserializer) {
        deser.get_u64();
    }
    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        //Self::dummy_read(deser);
        rt.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            size_of::<Self>(),
            TypeId::of::<Self>(),
        )
    }
}

impl Deserializable for i8 {
    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        at.cast::<Self>().write(Self::deserialize_inplace(deser));
    }

    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        deser.get_u8() as _
    }
    unsafe fn dummy_read(deser: &mut Deserializer) {
        deser.get_u8();
    }
    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        //Self::dummy_read(deser);
        rt.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            size_of::<Self>(),
            TypeId::of::<Self>(),
        )
    }
}

impl Deserializable for i16 {
    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        at.cast::<Self>().write(Self::deserialize_inplace(deser));
    }

    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        deser.get_u16() as _
    }
    unsafe fn dummy_read(deser: &mut Deserializer) {
        deser.get_u16();
    }
    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        //Self::dummy_read(deser);
        rt.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            size_of::<Self>(),
            TypeId::of::<Self>(),
        )
    }
}

impl Deserializable for i32 {
    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        at.cast::<Self>().write(Self::deserialize_inplace(deser));
    }

    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        deser.get_u32() as _
    }
    unsafe fn dummy_read(deser: &mut Deserializer) {
        deser.get_u32();
    }
    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        //Self::dummy_read(deser);
        rt.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            size_of::<Self>(),
            TypeId::of::<Self>(),
        )
    }
}

impl Deserializable for i64 {
    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        at.cast::<Self>().write(Self::deserialize_inplace(deser));
    }

    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        deser.get_u64() as _
    }
    unsafe fn dummy_read(deser: &mut Deserializer) {
        deser.get_u64();
    }
    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        //Self::dummy_read(deser);
        rt.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            size_of::<Self>(),
            TypeId::of::<Self>(),
        )
    }
}

impl Deserializable for f32 {
    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        at.cast::<Self>().write(Self::deserialize_inplace(deser));
    }

    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        f32::from_bits(deser.get_u32())
    }
    unsafe fn dummy_read(deser: &mut Deserializer) {
        deser.get_u32();
    }
    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        //Self::dummy_read(deser);
        rt.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            size_of::<Self>(),
            TypeId::of::<Self>(),
        )
    }
}

impl Deserializable for f64 {
    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        at.cast::<Self>().write(Self::deserialize_inplace(deser));
    }

    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        f64::from_bits(deser.get_u64())
    }
    unsafe fn dummy_read(deser: &mut Deserializer) {
        deser.get_u64();
    }
    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        //Self::dummy_read(deser);
        rt.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            size_of::<Self>(),
            TypeId::of::<Self>(),
        )
    }
}

impl<T: GcCell> Deserializable for WeakRef<T> {
    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        let ref_ = deser.get_reference();
        Self {
            inner: transmute(ref_),
            marker: Default::default(),
        }
    }

    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        unique!();
        at.cast::<Self>().write(Self::deserialize_inplace(deser));
    }

    unsafe fn dummy_read(deser: &mut Deserializer) {
        deser.get_u32();
    }
    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        //Self::dummy_read(deser);
        rt.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            size_of::<Self>(),
            TypeId::of::<Self>(),
        )
    }
}

impl Deserializable for MapEntry {
    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        at.cast::<Self>().write(Self::deserialize_inplace(deser));
    }
    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        let offset = deser.get_u32();
        let attrs = deser.get_u32();
        Self {
            offset,
            attrs: transmute(attrs),
        }
    }

    unsafe fn dummy_read(deser: &mut Deserializer) {
        Self::deserialize_inplace(deser);
    }
    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        unreachable!()
    }
}

impl Deserializable for TransitionKey {
    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        let name = Symbol::deserialize_inplace(deser);
        let attrs = deser.get_u32();
        Self { name, attrs }
    }

    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        at.cast::<Self>().write(Self::deserialize_inplace(deser));
    }
    unsafe fn dummy_read(deser: &mut Deserializer) {
        Symbol::dummy_read(deser);
        u32::dummy_read(deser);
    }
    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        unreachable!()
    }
}

impl Deserializable for DeletedEntry {
    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        let prev = deser.get_reference();
        let offset = deser.get_u32();
        Self {
            prev: transmute(prev),
            offset,
        }
    }

    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        at.cast::<Self>().write(Self::deserialize_inplace(deser));
    }

    unsafe fn dummy_read(deser: &mut Deserializer) {
        deser.get_u32();
        deser.get_u32();
    }

    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        //Self::dummy_read(deser);
        rt.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            size_of::<Self>(),
            TypeId::of::<Self>(),
        )
    }
}

impl Deserializable for DeletedEntryHolder {
    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        let entry = Option::<GcPointer<DeletedEntry>>::deserialize_inplace(deser);
        let size = deser.get_u32();
        Self { entry, size }
    }

    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        at.cast::<Self>().write(Self::deserialize_inplace(deser));
    }

    unsafe fn dummy_read(deser: &mut Deserializer) {
        Option::<GcPointer<DeletedEntry>>::dummy_read(deser);
        deser.get_u32();
    }
    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        //Self::dummy_read(deser);
        rt.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            size_of::<Self>(),
            TypeId::of::<Self>(),
        )
    }
}

impl Deserializable for Transition {
    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        let ty = deser.get_u8();
        match ty {
            0x0 => Self::None,
            0x1 => {
                let table = Option::<GcPointer<HashMap<TransitionKey,WeakRef<Structure>>>>::deserialize_inplace(deser);
                Self::Table(table)
            }
            0x2 => {
                let key = TransitionKey::deserialize_inplace(deser);
                let structure = WeakRef::<Structure>::deserialize_inplace(deser);
                Self::Pair(key, structure)
            }
            _ => unreachable!(),
        }
    }
    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        at.cast::<Self>().write(Self::deserialize_inplace(deser));
    }

    unsafe fn dummy_read(deser: &mut Deserializer) {
        let ty = deser.get_u8();
        match ty {
            0x0 => (),
            0x1 => {
                Option::<GcPointer<HashMap<TransitionKey, WeakRef<Structure>>>>::dummy_read(deser);
            }
            0x2 => {
                TransitionKey::dummy_read(deser);
                WeakRef::<Structure>::dummy_read(deser);
            }
            _ => unreachable!(),
        }
    }
    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        unreachable!()
    }
}

impl Deserializable for TransitionsTable {
    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        at.cast::<Self>().write(Self::deserialize_inplace(deser));
    }
    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        let var = Transition::deserialize_inplace(deser);
        let enabled = bool::deserialize_inplace(deser);
        let unique = bool::deserialize_inplace(deser);
        let indexed = bool::deserialize_inplace(deser);
        Self {
            var,
            enabled,
            unique,
            indexed,
        }
    }

    unsafe fn dummy_read(deser: &mut Deserializer) {
        Transition::dummy_read(deser);
        bool::dummy_read(deser);
        bool::dummy_read(deser);
        bool::dummy_read(deser);
    }

    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        unreachable!()
    }
}

impl Deserializable for Structure {
    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        let transitions = TransitionsTable::deserialize_inplace(deser);
        let table = Option::<GcPointer<TargetTable>>::deserialize_inplace(deser);
        let deleted = DeletedEntryHolder::deserialize_inplace(deser);
        let key = Symbol::deserialize_inplace(deser);
        let val = MapEntry::deserialize_inplace(deser);
        let previous = Option::<GcPointer<Self>>::deserialize_inplace(deser);
        let prototype = Option::<GcPointer<JsObject>>::deserialize_inplace(deser);

        let calculated_size = deser.get_u32();
        let transit_count = deser.get_u32();
        let has_been_flattened_before = bool::deserialize_inplace(deser);
        let cached_prototype_chain =
            Option::<GcPointer<StructureChain>>::deserialize_inplace(deser);
        Self {
            table,
            transitions,
            deleted,
            added: (key, val),
            previous,
            prototype,
            calculated_size,
            transit_count,
            id: 0,
            has_been_flattened_before,
            cached_prototype_chain,
        }
    }

    unsafe fn dummy_read(deser: &mut Deserializer) {
        let _transitions = TransitionsTable::dummy_read(deser);
        let _table = Option::<GcPointer<TargetTable>>::dummy_read(deser);
        let _deleted = DeletedEntryHolder::dummy_read(deser);
        let _key = Symbol::dummy_read(deser);
        let _val = MapEntry::dummy_read(deser);
        let _previous = Option::<GcPointer<Self>>::dummy_read(deser);
        let _prototype = Option::<GcPointer<JsObject>>::dummy_read(deser);

        let _calculated_size = deser.get_u32();
        let _transit_count = deser.get_u32();
    }

    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        at.cast::<Self>().write(Self::deserialize_inplace(deser));
    }

    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        //Self::dummy_read(deser);
        rt.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            size_of::<Self>(),
            TypeId::of::<Self>(),
        )
    }
}

impl Deserializable for CodeBlock {
    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        let name = Symbol::deserialize_inplace(deser);
        let names = Vec::<Symbol>::deserialize_inplace(deser);
        let strict = bool::deserialize_inplace(deser);
        let code = Vec::<u8>::deserialize_inplace(deser);
        let feedback = Vec::<TypeFeedBack>::deserialize_inplace(deser);
        let literals = Vec::<JsValue>::deserialize_inplace(deser);
        let codes = Vec::<GcPointer<Self>>::deserialize_inplace(deser);
        let top_level = bool::deserialize_inplace(deser);
        let use_arguments = bool::deserialize_inplace(deser);
        let filename = String::deserialize_inplace(deser);
        let rest_at = Option::<u32>::deserialize_inplace(deser);
        let var_count = u32::deserialize_inplace(deser);
        let param_count = u32::deserialize_inplace(deser);
        let args_at = u32::deserialize_inplace(deser);
        let is_constructor = bool::deserialize_inplace(deser);
        let len = u32::deserialize_inplace(deser);
        let mut loc = Vec::with_capacity(len as _);
        for _ in 0..len {
            let start = u32::deserialize_inplace(deser) as usize;
            let end = u32::deserialize_inplace(deser) as usize;
            let line = u32::deserialize_inplace(deser);
            let col = u32::deserialize_inplace(deser);
            loc.push((start..end, FileLocation { line, col }));
        }
        Self {
            loc,
            args_at,
            use_arguments,
            name,
            names,
            top_level,
            strict,
            literals_ptr: literals.as_ptr(),
            code,
            feedback,
            literals,
            codes,
            file_name: filename,
            rest_at,
            var_count,
            param_count,
            is_constructor,
        }
    }

    unsafe fn dummy_read(deser: &mut Deserializer) {
        let _name = Symbol::dummy_read(deser);
        let _names = Vec::<Symbol>::dummy_read(deser);
        let _strict = bool::dummy_read(deser);
        let _variables = Vec::<Symbol>::dummy_read(deser);
        let _code = Vec::<u8>::dummy_read(deser);
        let _feedback = Vec::<TypeFeedBack>::dummy_read(deser);
        let _literals = Vec::<JsValue>::dummy_read(deser);
        let _rest_param = Option::<Symbol>::dummy_read(deser);
        let _params = Vec::<Symbol>::dummy_read(deser);
    }

    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        at.cast::<Self>().write(Self::deserialize_inplace(deser));
    }

    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        //Self::dummy_read(deser);
        rt.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            size_of::<Self>(),
            TypeId::of::<Self>(),
        )
    }
}

impl GcCell for TypeFeedBack {
    fn deser_pair(&self) -> (usize, usize) {
        (Self::deserialize as _, Self::allocate as _)
    }
}

impl Deserializable for TypeFeedBack {
    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        let ty = deser.get_u8();
        match ty {
            0x01 => {
                let structure = deser.get_reference();
                let offset = deser.get_u32();
                Self::PropertyCache {
                    structure: transmute(structure),
                    offset,
                }
            }
            0x02 => {
                let structure = deser.get_reference();
                Self::StructureCache {
                    structure: transmute(structure),
                }
            }
            0x03 => {
                let new_structure = Option::<GcPointer<Structure>>::deserialize_inplace(deser);
                let old_structure = Option::<GcPointer<Structure>>::deserialize_inplace(deser);
                let offset = u32::deserialize_inplace(deser);
                let structure_chain =
                    Option::<GcPointer<StructureChain>>::deserialize_inplace(deser);
                TypeFeedBack::PutByIdFeedBack {
                    new_structure,
                    old_structure,
                    offset,
                    structure_chain,
                }
            }
            0x0 => Self::None,
            _ => unreachable!(),
        }
    }
    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        unreachable!()
    }
    unsafe fn dummy_read(deser: &mut Deserializer) {
        unreachable!()
    }

    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        unreachable!()
    }
}

impl Deserializable for JsSymbol {
    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        Self {
            sym: Symbol::deserialize_inplace(deser),
        }
    }
    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        at.cast::<Self>().write(Self::deserialize_inplace(deser));
    }

    unsafe fn dummy_read(deser: &mut Deserializer) {
        Symbol::dummy_read(deser);
    }
    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        //Self::dummy_read(deser);
        rt.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            size_of::<Self>() as _,
            TypeId::of::<Self>(),
        )
    }
}

impl<A: Deserializable, B: Deserializable> Deserializable for (A, B) {
    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        (A::deserialize_inplace(deser), B::deserialize_inplace(deser))
    }
    unsafe fn deserialize(_at: *mut u8, _deser: &mut Deserializer) {
        unreachable!()
    }

    unsafe fn allocate(_rt: &mut Runtime, _deser: &mut Deserializer) -> *mut GcPointerBase {
        unreachable!()
    }
}
