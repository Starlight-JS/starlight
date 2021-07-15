/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use super::{attributes::*, object::JsObject, structure_chain::StructureChain};
use super::{symbol_table::*, Context};
use crate::gc::cell::{GcCell, GcPointer, Trace, WeakRef};
use crate::gc::{cell::Tracer, snapshot::deserializer::Deserializable};
use crate::prelude::*;
use std::{collections::HashMap, intrinsics::likely};
use wtf_rs::unwrap_unchecked;
/// In JavaScript programs, it's common to have multiple objects with the same property keys. Such objects
/// have the same *shape*.
/// ```js
/// let obj1 = {x: 1,y: 2}
/// let obj2 = {x: 3,y: 4}
/// ```
///
/// It's also common to access property on objects with the same shape:
///
/// ```js
/// function f(obj) {
///     return obj.x;
/// }
///
/// f(obj1);
/// f(obj2);
/// ```
///
/// With that in mind, Starlight can optimize object property accesses based on the object's shape or `Structure` how
/// call it.
///
///
/// `Structure` stores property keys, offsets within JSObject and property attributes, structures might be shared between
/// multiple objects. When property is added new structure is created (if does not exist) and transition happens to the
/// new structure. This way we can optimize field load into single `object.slots + field_offset` load.
///
/// More info here: [JavaScript engine fundamentals: Shapes and Inline Caches](https://mathiasbynens.be/notes/shapes-ics)
pub struct Structure {
    pub(crate) id: StructureID,
    pub(crate) transitions: TransitionsTable,
    pub(crate) table: Option<GcPointer<TargetTable>>,
    /// Singly linked list
    pub(crate) deleted: DeletedEntryHolder,
    pub(crate) added: (Symbol, MapEntry),
    pub(crate) previous: Option<GcPointer<Structure>>,
    pub(crate) prototype: Option<GcPointer<JsObject>>,
    pub(crate) calculated_size: u32,
    pub(crate) transit_count: u32,
    pub(crate) has_been_flattened_before: bool,
    pub(crate) cached_prototype_chain: Option<GcPointer<StructureChain>>,
}

pub type StructureID = u32;

#[derive(Copy, Clone)]
pub struct MapEntry {
    pub offset: u32,
    pub attrs: AttrSafe,
}

impl MapEntry {
    pub fn not_found() -> Self {
        Self {
            offset: u32::MAX,
            attrs: AttrSafe::not_found(),
        }
    }

    pub fn is_not_found(&self) -> bool {
        self.attrs.is_not_found()
    }
}

impl GcCell for MapEntry {
    fn deser_pair(&self) -> (usize, usize) {
        (Self::deserialize as _, Self::allocate as _)
    }
}
unsafe impl Trace for MapEntry {}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TransitionKey {
    pub name: Symbol,
    pub attrs: u32,
}

impl GcCell for TransitionKey {
    fn deser_pair(&self) -> (usize, usize) {
        (Self::deserialize as _, Self::allocate as _)
    }
}
unsafe impl Trace for TransitionKey {}

#[derive(Clone)]
pub enum Transition {
    None,
    Table(Option<GcPointer<Table>>),
    Pair(TransitionKey, WeakRef<Structure>),
}

pub struct TransitionsTable {
    pub var: Transition,
    pub enabled: bool,
    pub unique: bool,
    pub indexed: bool,
}

impl TransitionsTable {
    pub fn new(enabled: bool, indexed: bool) -> Self {
        Self {
            var: Transition::None,
            unique: false,
            indexed,
            enabled,
        }
    }
    pub fn set_indexed(&mut self, indexed: bool) {
        self.indexed = indexed;
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled_unique_transition(&self) -> bool {
        self.unique
    }

    pub fn enable_unique_transition(&mut self) {
        self.unique = true;
    }

    pub fn insert(
        &mut self,
        mut ctx: GcPointer<Context>,
        name: Symbol,
        attrs: AttrSafe,
        map: GcPointer<Structure>,
    ) {
        let key = TransitionKey {
            name,
            attrs: attrs.raw(),
        };
        if let Transition::Pair(x, y) = &self.var {
            let mut table = ctx.heap().allocate(HashMap::new());
            table.insert(*x, *y);
            self.var = Transition::Table(Some(table));
        }
        if let Transition::Table(Some(ref mut table)) = self.var {
            table.insert(key, ctx.heap().make_weak(map));
        } else {
            self.var = Transition::Pair(key, ctx.heap().make_weak(map));
        }
    }

    pub fn find(&self, name: Symbol, attrs: AttrSafe) -> Option<GcPointer<Structure>> {
        let key = TransitionKey {
            name,
            attrs: attrs.raw(),
        };
        if let Transition::Table(ref table) = &self.var {
            return table.as_ref().unwrap().get(&key).and_then(|x| x.upgrade());
        } else if let Transition::Pair(key_, map) = &self.var {
            if key == *key_ {
                return map.upgrade();
            }
        }
        None
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    pub fn is_indexed(&self) -> bool {
        self.indexed
    }
}

pub type Table = HashMap<TransitionKey, WeakRef<Structure>>;

unsafe impl Trace for TransitionsTable {
    fn trace(&mut self, tracer: &mut dyn Tracer) {
        match self.var {
            Transition::Pair(_, ref mut x) => x.trace(tracer),
            Transition::Table(ref mut table) => {
                table.trace(tracer);
            }
            _ => (),
        }
    }
}
impl GcCell for Structure {
    fn deser_pair(&self) -> (usize, usize) {
        (Self::deserialize as _, Self::allocate as _)
    }
}
unsafe impl Trace for Structure {
    fn trace(&mut self, tracer: &mut dyn Tracer) {
        self.transitions.trace(tracer);
        self.table.trace(tracer);
        self.prototype.trace(tracer);
        self.deleted.entry.trace(tracer);
        match self.previous.as_mut() {
            Some(x) => {
                x.trace(tracer);
            }
            _ => (),
        }
    }
}

impl Structure {
    pub fn has_mono_proto(&self) -> bool {
        self.prototype.is_some()
    }

    pub fn has_poly_proto(&self) -> bool {
        self.prototype.is_none()
    }

    pub fn id(&self) -> StructureID {
        self.id
    }
    /// Set structure ID.
    ///
    /// # Safety
    ///
    /// It is unsafe to change structure id since it may change program behaviour.
    pub unsafe fn set_id(&mut self, id: StructureID) {
        self.id = id;
    }
}
#[derive(Clone)]
pub struct DeletedEntryHolder {
    pub entry: Option<GcPointer<DeletedEntry>>,
    pub size: u32,
}

impl DeletedEntryHolder {
    pub fn push(&mut self, mut ctx: GcPointer<Context>, offset: u32) {
        let entry = ctx.heap().allocate(DeletedEntry {
            prev: self.entry,
            offset,
        });
        self.entry = Some(entry);
    }
    pub fn pop(&mut self) -> u32 {
        let res = unwrap_unchecked(self.entry.as_ref()).offset;
        self.entry = unwrap_unchecked(self.entry.as_ref()).prev;
        self.size -= 1;
        res
    }

    pub fn size(&self) -> u32 {
        self.size
    }

    pub fn empty(&self) -> bool {
        self.size == 0
    }
}

pub type TargetTable = HashMap<Symbol, MapEntry>;

pub struct DeletedEntry {
    pub prev: Option<GcPointer<DeletedEntry>>,
    pub offset: u32,
}

unsafe impl Trace for DeletedEntry {
    fn trace(&mut self, tracer: &mut dyn Tracer) {
        self.prev.trace(tracer)
    }
}
unsafe impl Trace for DeletedEntryHolder {
    fn trace(&mut self, visitor: &mut dyn Tracer) {
        self.entry.trace(visitor);
    }
}
impl GcCell for DeletedEntryHolder {
    fn deser_pair(&self) -> (usize, usize) {
        (Self::deserialize as _, Self::allocate as _)
    }
}
impl GcCell for DeletedEntry {
    fn deser_pair(&self) -> (usize, usize) {
        (Self::deserialize as _, Self::allocate as _)
    }
}

impl Structure {
    fn ctor(
        mut ctx: GcPointer<Context>,
        previous: GcPointer<Self>,
        unique: bool,
    ) -> GcPointer<Self> {
        let mut this = ctx.heap().allocate(Structure {
            prototype: previous.prototype,
            previous: Some(previous),
            table: if unique && previous.is_unique() {
                previous.table
            } else {
                None
            },
            transitions: TransitionsTable::new(!unique, previous.transitions.is_indexed()),
            deleted: previous.deleted.clone(),
            added: (
                DUMMY_SYMBOL,
                MapEntry {
                    offset: u32::MAX,
                    attrs: AttrSafe::not_found(),
                },
            ),
            id: 0,
            calculated_size: previous.get_slots_size() as _,
            transit_count: 0,
            has_been_flattened_before: previous.has_been_flattened_before,
            cached_prototype_chain: None,
        });
        this.calculated_size = this.get_slots_size() as _;
        assert!(this.previous.is_some());
        this
    }

    fn ctor1(
        mut ctx: GcPointer<Context>,
        prototype: Option<GcPointer<JsObject>>,
        unique: bool,
        indexed: bool,
    ) -> GcPointer<Self> {
        ctx.heap().allocate(Structure {
            prototype,
            cached_prototype_chain: None,
            previous: None,
            table: None,
            has_been_flattened_before: false,
            transitions: TransitionsTable::new(!unique, indexed),
            deleted: DeletedEntryHolder {
                entry: None,
                size: 0,
            },
            added: (
                DUMMY_SYMBOL,
                MapEntry {
                    offset: u32::MAX,
                    attrs: AttrSafe::not_found(),
                },
            ),
            id: 0,
            calculated_size: 0,
            transit_count: 0,
        })
    }
    #[allow(dead_code)]
    fn ctor2(
        ctx: GcPointer<Context>,
        table: Option<GcPointer<TargetTable>>,
        prototype: Option<GcPointer<JsObject>>,
        unique: bool,
        indexed: bool,
    ) -> GcPointer<Self> {
        let mut this = Self::ctor1(ctx, prototype, unique, indexed);
        this.table = table;
        this.calculated_size = this.get_slots_size() as _;
        this
    }

    fn ctor3(mut ctx: GcPointer<Context>, it: &[(Symbol, MapEntry)]) -> GcPointer<Self> {
        let table = it.iter().copied().collect::<TargetTable>();
        let table = ctx.heap().allocate(table);
        let mut this = ctx.heap().allocate(Structure {
            prototype: None,
            previous: None,
            cached_prototype_chain: None,
            has_been_flattened_before: false,
            table: Some(table),
            transitions: TransitionsTable::new(true, false),
            deleted: DeletedEntryHolder {
                entry: None,
                size: 0,
            },
            added: (
                DUMMY_SYMBOL,
                MapEntry {
                    offset: u32::MAX,
                    attrs: AttrSafe::not_found(),
                },
            ),
            id: 0,
            calculated_size: 0,
            transit_count: 0,
        });
        this.calculated_size = this.get_slots_size() as _;
        this
    }

    pub fn new(ctx: GcPointer<Context>, previous: GcPointer<Self>) -> GcPointer<Structure> {
        Self::ctor(ctx, previous, false)
    }

    pub fn new_unique(ctx: GcPointer<Context>, previous: GcPointer<Self>) -> GcPointer<Structure> {
        Self::ctor(ctx, previous, true)
    }
    pub fn new_unique_with_proto(
        ctx: GcPointer<Context>,
        proto: Option<GcPointer<JsObject>>,
        indexed: bool,
    ) -> GcPointer<Self> {
        Self::ctor2(ctx, None, proto, true, indexed)
    }
    pub fn new_(ctx: GcPointer<Context>, it: &[(Symbol, MapEntry)]) -> GcPointer<Self> {
        Self::ctor3(ctx, it)
    }
    pub fn new_from_table(
        mut ctx: GcPointer<Context>,
        table: Option<TargetTable>,
        prototype: Option<GcPointer<JsObject>>,
        unique: bool,
        indexed: bool,
    ) -> GcPointer<Structure> {
        let table = table.map(|table| ctx.heap().allocate(table));

        Self::ctor2(ctx, table, prototype, unique, indexed)
    }
    pub fn new_indexed(
        ctx: GcPointer<Context>,
        prototype: Option<GcPointer<JsObject>>,
        indexed: bool,
    ) -> GcPointer<Self> {
        Self::ctor1(ctx, prototype, false, indexed)
    }
    pub fn new_unique_indexed(
        ctx: GcPointer<Context>,
        prototype: Option<GcPointer<JsObject>>,
        indexed: bool,
    ) -> GcPointer<Self> {
        Self::ctor1(ctx, prototype, true, indexed)
    }

    pub fn new_from_point(ctx: GcPointer<Context>, map: GcPointer<Structure>) -> GcPointer<Self> {
        if map.is_unique() {
            return Self::new_unique(ctx, map);
        }
        map
    }
}

impl GcPointer<Structure> {
    fn is_valid(
        &mut self,
        ctx: GcPointer<Context>,
        cached_prototype_chain: Option<GcPointer<StructureChain>>,
        base: GcPointer<JsObject>,
    ) -> bool {
        if cached_prototype_chain.is_none() {
            return false;
        }
        let cached_prototype_chain = cached_prototype_chain.unwrap();

        let mut prototype = self.stored_prototype(ctx, &base);
        let mut cached_structure;
        let mut i = 0;
        while i < cached_prototype_chain.vector.len() && !prototype.is_null() {
            cached_structure = Some(cached_prototype_chain.vector[i]);
            i += 1;
            if !GcPointer::ptr_eq(
                &prototype.get_jsobject().structure(),
                &cached_structure.unwrap(),
            ) {
                return false;
            }
            prototype = prototype
                .get_jsobject()
                .structure()
                .stored_prototype(ctx, &prototype.get_jsobject());
        }
        prototype.is_null() && i >= cached_prototype_chain.vector.len()
    }
    pub fn prototype_chain(
        &mut self,
        ctx: GcPointer<Context>,
        base: GcPointer<JsObject>,
    ) -> GcPointer<StructureChain> {
        if !self.is_valid(ctx, self.cached_prototype_chain, base) {
            let prototype = self.stored_prototype(ctx, &base);
            self.cached_prototype_chain = Some(StructureChain::create(
                ctx,
                if prototype.is_null() {
                    None
                } else {
                    Some(prototype.get_jsobject())
                },
            ));
        }
        self.cached_prototype_chain.unwrap()
    }
    pub fn flatten_dictionary_structure(
        &mut self,
        _ctx: GcPointer<Context>,
        object: &GcPointer<JsObject>,
    ) -> GcPointer<Structure> {
        assert!(self.is_unique());
        assert!(GcPointer::ptr_eq(&object.structure, self));
        self.has_been_flattened_before = true;
        self.flatten();
        *self
    }
    pub fn stored_prototype(
        &mut self,
        ctx: GcPointer<Context>,
        object: &GcPointer<JsObject>,
    ) -> JsValue {
        if likely(self.has_mono_proto()) {
            return JsValue::new(self.prototype.unwrap());
        }
        let entry = self.get(ctx, "__proto__".intern());
        if entry.is_not_found() {
            return JsValue::encode_null_value();
        }
        *object.direct(entry.offset as usize)
    }
    pub fn delete(&mut self, ctx: GcPointer<Context>, name: Symbol) {
        let it = unwrap_unchecked(self.table.as_mut()).remove(&name).unwrap();
        self.deleted.push(ctx, it.offset);
    }

    pub fn change_attributes(&mut self, name: Symbol, attributes: AttrSafe) {
        let it = unwrap_unchecked(self.table.as_mut())
            .get_mut(&name)
            .unwrap();
        it.attrs = attributes;
    }

    pub fn table(&self) -> Option<&GcPointer<TargetTable>> {
        self.table.as_ref()
    }
    pub fn is_adding_map(&self) -> bool {
        self.added.0 != DUMMY_SYMBOL
    }

    pub fn has_table(&self) -> bool {
        self.table.is_some()
    }
    pub fn allocate_table(&mut self, mut ctx: GcPointer<Context>) {
        let mut stack = ctx.heap().allocate(Vec::with_capacity(8));

        if self.is_adding_map() {
            stack.push(*self);
        }

        let mut current = self.previous;
        loop {
            match current {
                Some(cur) => {
                    if cur.has_table() {
                        self.table =
                            Some(ctx.heap().allocate((**cur.table.as_ref().unwrap()).clone()));
                        break;
                    } else if cur.is_adding_map() {
                        stack.push(cur);
                    }
                    current = cur.previous;
                }
                None => {
                    self.table = Some(ctx.heap().allocate(HashMap::new()));
                    break;
                }
            }
        }
        assert!(self.table.is_some());
        let table = self.table.as_mut().unwrap();

        for it in stack.iter() {
            table.insert((*it).added.0, (*it).added.1);
        }

        self.previous = None;
    }

    pub fn allocate_table_if_needed(&mut self, ctx: GcPointer<Context>) -> bool {
        if !self.has_table() {
            if self.previous.is_none() {
                return false;
            }
            self.allocate_table(ctx);
        }
        true
    }

    pub fn is_indexed(&self) -> bool {
        self.transitions.is_indexed()
    }

    pub fn is_unique(&self) -> bool {
        !self.transitions.is_enabled()
    }

    pub fn is_shaped(&self) -> bool {
        // we can use this map id as shape or not
        !self.is_unique() || self.transitions.is_enabled()
    }

    pub fn prototype(&self) -> Option<&GcPointer<JsObject>> {
        self.prototype.as_ref()
    }

    pub unsafe fn prototype_mut(&mut self) -> Option<&mut GcPointer<JsObject>> {
        self.prototype.as_mut()
    }

    pub fn flatten(&mut self) {
        if self.is_unique() {
            self.transitions.enable_unique_transition();
            self.has_been_flattened_before = true;
        }
    }

    pub fn get_slots_size(&self) -> usize {
        if let Some(table) = self.table.as_ref() {
            table.len() + self.deleted.size as usize
        } else {
            self.calculated_size as _
        }
    }
}

impl GcPointer<Structure> {
    pub fn delete_property_transition(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
    ) -> GcPointer<Structure> {
        let mut map = Structure::new_unique(ctx, *self);
        if !map.has_table() {
            map.allocate_table(ctx);
        }
        map.delete(ctx, name);
        map
    }
    pub fn change_indexed_transition(&mut self, ctx: GcPointer<Context>) -> GcPointer<Structure> {
        if self.is_unique() {
            let mut map = if self.transitions.is_enabled_unique_transition() {
                Structure::new_unique(ctx, *self)
            } else {
                *self
            };
            map.transitions.set_indexed(true);
            map
        } else {
            Structure::new_unique(ctx, *self).change_indexed_transition(ctx)
        }
    }

    pub fn change_prototype_transition(
        &mut self,
        ctx: GcPointer<Context>,
        prototype: Option<GcPointer<JsObject>>,
    ) -> GcPointer<Structure> {
        if self.is_unique() {
            let mut map = if self.transitions.is_enabled_unique_transition() {
                Structure::new_unique(ctx, *self)
            } else {
                *self
            };
            map.prototype = prototype;
            map
        } else {
            let mut map = Structure::new_unique(ctx, *self);
            map.change_prototype_transition(ctx, prototype)
        }
    }

    pub fn change_extensible_transition(
        &mut self,
        ctx: GcPointer<Context>,
    ) -> GcPointer<Structure> {
        Structure::new_unique(ctx, *self)
    }
    pub fn change_attributes_transition(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
        attributes: AttrSafe,
    ) -> GcPointer<Structure> {
        let mut map = Structure::new_unique(ctx, *self);
        if !map.has_table() {
            map.allocate_table(ctx);
        }
        map.change_attributes(name, attributes);
        map
    }

    pub fn get_own_property_names(
        &mut self,
        ctx: GcPointer<Context>,
        include: bool,
        mut collector: impl FnMut(Symbol, u32),
    ) {
        if self.allocate_table_if_needed(ctx) {
            for entry in self.table.as_ref().unwrap().iter() {
                /*if entry.0.is_private() {
                    continue;
                }
                if entry.0.is_public() {
                    continue;
                }*/
                if include || entry.1.attrs.is_enumerable() {
                    collector(*entry.0, entry.1.offset);
                }
            }
        }
    }

    pub fn add_property_transition(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
        attributes: AttrSafe,
        offset: &mut u32,
    ) -> GcPointer<Structure> {
        let mut entry = MapEntry {
            offset: 0,
            attrs: attributes,
        };

        if self.is_unique() {
            if !self.has_table() {
                self.allocate_table(ctx);
            }

            let mut map = if self.transitions.is_enabled_unique_transition() {
                Structure::new_unique(ctx, *self)
            } else {
                *self
            };
            if !map.deleted.empty() {
                entry.offset = map.deleted.pop();
            } else {
                entry.offset = self.get_slots_size() as _;
            }
            unwrap_unchecked(map.table.as_mut()).insert(name, entry);
            *offset = entry.offset;
            return map;
        }

        // existing transition check
        if let Some(map) = self.transitions.find(name, attributes) {
            *offset = map.added.1.offset;

            return map;
        }
        if self.transit_count > 64 {
            // stop transition
            let mut map = Structure::new_unique(ctx, *self);
            // go to above unique path
            return map.add_property_transition(ctx, name, attributes, offset);
        }
        let mut map = Structure::new(ctx, *self);

        if !map.deleted.empty() {
            let slot = map.deleted.pop();
            map.added = (
                name,
                MapEntry {
                    offset: slot,
                    attrs: attributes,
                },
            );
            map.calculated_size = self.get_slots_size() as _;
        } else {
            map.added = (
                name,
                MapEntry {
                    offset: self.get_slots_size() as _,
                    attrs: attributes,
                },
            );
            map.calculated_size = self.get_slots_size() as u32 + 1;
        }
        map.transit_count += 1;
        self.transitions.insert(ctx, name, attributes, map);
        *offset = map.added.1.offset;
        assert!(map.get_slots_size() as u32 > map.added.1.offset);

        map
    }

    pub fn get(&mut self, ctx: GcPointer<Context>, name: Symbol) -> MapEntry {
        if !self.has_table() {
            if self.previous.is_none() {
                return MapEntry::not_found();
            }
            if self.is_adding_map() && self.added.0 == name {
                return self.added.1;
            }

            self.allocate_table(ctx);
        }

        let it = self.table.as_ref().unwrap().get(&name);

        it.copied().unwrap_or_else(MapEntry::not_found)
    }

    pub fn storage_capacity(&self) -> usize {
        let sz = self.get_slots_size();
        if sz == 0 {
            0
        } else if sz < 8 {
            8
        } else {
            fn clp2(number: usize) -> usize {
                let x = number - 1;
                let x = x | (x >> 1);
                let x = x | (x >> 2);
                let x = x | (x >> 4);
                let x = x | (x >> 8);
                let x = x | (x >> 16);
                x + 1
            }
            clp2(sz)
        }
    }
    pub fn change_prototype_with_no_transition(&mut self, prototype: GcPointer<JsObject>) -> Self {
        self.prototype = Some(prototype);
        *self
    }
}
