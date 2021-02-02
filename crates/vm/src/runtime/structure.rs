use super::{
    attributes::AttrSafe,
    context::Context,
    js_cell::{allocate_cell, JsCell},
    js_object::JsObject,
    ref_ptr::{AsRefPtr, Ref},
    symbol::{Symbol, DUMMY_SYMBOL},
    vm::JsVirtualMachine,
};
use crate::{
    gc::{handle::Handle, heap_cell::HeapObject},
    heap::trace::Tracer,
};
use std::{collections::HashMap, mem::size_of};
use wtf_rs::unwrap_unchecked;

/// Map object is like object
/// These structures are used for implementing Polymorphic Inline Cache.
///
///
/// original paper is
///   http://cs.au.dk/~hosc/local/LaSC-4-3-pp243-281.pdf
///

pub struct Structure {
    id: StructureID,
    transitions: Transitions,
    table: Option<Handle<TargetTable>>,
    deleted: DeletedEntryHolder,
    added: (Symbol, MapEntry),
    previous: Option<Handle<Structure>>,
    prototype: Option<Handle<JsObject>>,
    calculated_size: u32,
    transit_count: u32,
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

impl JsCell for MapEntry {}
impl HeapObject for MapEntry {
    fn needs_destruction(&self) -> bool {
        false
    }

    fn visit_children(&mut self, _tracer: &mut dyn Tracer) {}
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TransitionKey {
    name: Symbol,
    attrs: u32,
}

impl JsCell for TransitionKey {}
impl HeapObject for TransitionKey {
    fn visit_children(&mut self, _tracer: &mut dyn Tracer) {}
    fn needs_destruction(&self) -> bool {
        false
    }
}

union U {
    table: Option<Handle<Table>>,
    pair: (TransitionKey, Option<Handle<Structure>>),
}
pub struct Transitions {
    u: U,
    flags: u8,
}

const MASK_ENABLED: u8 = 1;
const MASK_UNIQUE_TRANSITION: u8 = 2;
const MASK_HOLD_SINGLE: u8 = 4;
const MASK_HOLD_TABLE: u8 = 8;
const MASK_INDEXED: u8 = 16;

type Table = HashMap<TransitionKey, Option<Handle<Structure>>>;

impl Transitions {
    pub fn new(enabled: bool, indexed: bool) -> Self {
        let mut this = Self {
            u: U { table: None },
            flags: 0,
        };
        this.set_enabled(enabled);
        this.set_indexed(indexed);
        this
    }
    pub fn set_indexed(&mut self, indexed: bool) {
        if indexed {
            self.flags |= MASK_INDEXED;
        } else {
            self.flags &= !MASK_INDEXED;
        }
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        if enabled {
            self.flags |= MASK_ENABLED;
        } else {
            self.flags &= !MASK_ENABLED;
        }
    }

    pub fn is_enabled_unique_transition(&self) -> bool {
        (self.flags & MASK_UNIQUE_TRANSITION) != 0
    }

    pub fn enable_unique_transition(&mut self) {
        self.flags |= MASK_UNIQUE_TRANSITION;
    }

    pub fn insert(
        &mut self,
        vm: Ref<JsVirtualMachine>,
        name: Symbol,
        attrs: AttrSafe,
        map: Handle<Structure>,
    ) {
        let key = TransitionKey {
            name,
            attrs: attrs.raw(),
        };
        unsafe {
            if (self.flags & MASK_HOLD_SINGLE) != 0 {
                let mut table: Handle<Table> =
                    allocate_cell(vm, size_of::<Table>(), Default::default());
                table.insert(self.u.pair.0, self.u.pair.1);
                self.u.table = Some(table);
                self.flags &= !MASK_HOLD_SINGLE;
                self.flags &= MASK_HOLD_TABLE;
            }
            if (self.flags & MASK_HOLD_TABLE) != 0 {
                self.u.table.unwrap().insert(key, Some(map));
            } else {
                self.u.pair.0 = key;
                self.u.pair.1 = Some(map);
                self.flags |= MASK_HOLD_SINGLE;
            }
        }
    }

    pub fn find(&self, name: Symbol, attrs: AttrSafe) -> Option<Handle<Structure>> {
        let key = TransitionKey {
            name,
            attrs: attrs.raw(),
        };
        unsafe {
            if (self.flags & MASK_HOLD_TABLE) != 0 {
                return self.u.table.unwrap().get(&key).copied().flatten();
            } else if (self.flags & MASK_HOLD_SINGLE) != 0 {
                if self.u.pair.0 == key {
                    return self.u.pair.1;
                }
            }
        }
        None
    }

    pub fn is_enabled(&self) -> bool {
        (self.flags & MASK_ENABLED) != 0
    }

    pub fn is_indexed(&self) -> bool {
        (self.flags & MASK_INDEXED) != 0
    }
}

impl JsCell for Structure {}
impl HeapObject for Structure {
    fn visit_children(&mut self, tracer: &mut dyn Tracer) {
        unsafe {
            if (self.transitions.flags & MASK_HOLD_SINGLE) != 0 {
                if let Some(ref mut map) = self.transitions.u.pair.1 {
                    map.visit_children(tracer);
                }
            } else if (self.transitions.flags & MASK_HOLD_TABLE) != 0 {
                if let Some(ref mut table) = self.transitions.u.table {
                    table.visit_children(tracer);
                }
            }
            self.table.visit_children(tracer);
            self.prototype.visit_children(tracer);
            self.deleted.entry.visit_children(tracer);
        }
    }

    fn needs_destruction(&self) -> bool {
        true
    }
}

impl Structure {
    pub fn id(&self) -> StructureID {
        self.id
    }

    pub unsafe fn set_id(&mut self, id: StructureID) {
        self.id = id;
    }
}
#[derive(Clone, Copy)]
pub struct DeletedEntryHolder {
    entry: Option<Handle<DeletedEntry>>,
    size: u32,
}

impl DeletedEntryHolder {
    pub fn push(&mut self, vm: impl AsRefPtr<JsVirtualMachine>, offset: u32) {
        let entry = allocate_cell(
            vm,
            size_of::<DeletedEntry>(),
            DeletedEntry {
                prev: self.entry,
                offset,
            },
        );
        self.entry = Some(entry);
    }
    pub fn pop(&mut self) -> u32 {
        let res = unwrap_unchecked(self.entry).offset;
        self.entry = unwrap_unchecked(self.entry).prev;
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
    prev: Option<Handle<DeletedEntry>>,
    offset: u32,
}

impl HeapObject for DeletedEntry {
    fn visit_children(&mut self, tracer: &mut dyn Tracer) {
        self.prev.visit_children(tracer)
    }
    fn needs_destruction(&self) -> bool {
        false
    }
}

impl JsCell for DeletedEntry {}

impl Structure {
    pub fn delete(&mut self, vm: impl AsRefPtr<JsVirtualMachine>, name: Symbol) {
        let it = unwrap_unchecked(self.table.as_mut()).remove(&name).unwrap();
        self.deleted.push(vm, it.offset);
    }

    pub fn change_attributes(&mut self, name: Symbol, attributes: AttrSafe) {
        let it = unwrap_unchecked(self.table.as_mut())
            .get_mut(&name)
            .unwrap();
        it.attrs = attributes;
    }

    pub fn table(&self) -> Option<Handle<TargetTable>> {
        self.table
    }
    pub fn is_adding_map(&self) -> bool {
        self.added.0 != DUMMY_SYMBOL
    }

    pub fn has_table(&self) -> bool {
        self.table.is_some()
    }
    pub fn allocate_table(&mut self, vm: impl AsRefPtr<JsVirtualMachine>) {
        let mut stack = Vec::with_capacity(8);

        if self.is_adding_map() {
            stack.push(Ref::new(self));
        }

        let mut current = self.previous;
        loop {
            match current {
                Some(mut cur) => {
                    if cur.has_table() {
                        self.table = Some(allocate_cell(
                            vm.as_ref_ptr(),
                            size_of::<TargetTable>(),
                            HashMap::new(),
                        ));
                        break;
                    } else {
                        if cur.is_adding_map() {
                            stack.push(Ref::new(&mut *cur));
                        }
                    }
                    current = cur.previous;
                }
                None => {
                    self.table = Some(allocate_cell(
                        vm.as_ref_ptr(),
                        size_of::<TargetTable>(),
                        HashMap::new(),
                    ));
                    break;
                }
            }
        }
        assert!(self.table.is_some());
        let mut table = self.table.unwrap();
        for it in stack {
            table.insert(it.added.0, it.added.1);
        }
        self.previous = None;
    }

    pub fn allocate_table_if_needed(&mut self, vm: impl AsRefPtr<JsVirtualMachine>) -> bool {
        if !self.has_table() {
            if self.previous.is_none() {
                return false;
            }
            self.allocate_table(vm);
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

    pub fn prototype(&self) -> Option<Handle<JsObject>> {
        self.prototype
    }

    pub fn flatten(&mut self) {
        if self.is_unique() {
            self.transitions.enable_unique_transition();
        }
    }

    pub fn get_slots_size(&self) -> usize {
        if let Some(table) = self.table {
            table.len() + self.deleted.size as usize
        } else {
            self.calculated_size as _
        }
    }
    fn ctor(
        vm: impl AsRefPtr<JsVirtualMachine>,
        previous: Handle<Self>,
        unique: bool,
    ) -> Handle<Self> {
        let mut this = allocate_cell(
            vm,
            size_of::<Self>(),
            Self {
                prototype: previous.prototype,
                previous: Some(previous),
                table: if unique && previous.is_unique() {
                    previous.table
                } else {
                    None
                },
                transitions: Transitions::new(!unique, previous.transitions.is_indexed()),
                deleted: previous.deleted,
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
            },
        );
        this.calculated_size = this.get_slots_size() as _;
        this
    }

    fn ctor1(
        vm: impl AsRefPtr<JsVirtualMachine>,
        prototype: Option<Handle<JsObject>>,
        unique: bool,
        indexed: bool,
    ) -> Handle<Self> {
        allocate_cell(
            vm,
            size_of::<Self>(),
            Self {
                prototype,
                previous: None,
                table: None,
                transitions: Transitions::new(!unique, indexed),
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
            },
        )
    }

    fn ctor2(
        vm: impl AsRefPtr<JsVirtualMachine>,
        table: Option<Handle<TargetTable>>,
        prototype: Option<Handle<JsObject>>,
        unique: bool,
        indexed: bool,
    ) -> Handle<Self> {
        let mut this = Self::ctor1(vm, prototype, unique, indexed);
        this.table = table;
        this.calculated_size = this.get_slots_size() as _;
        this
    }

    fn ctor3(vm: impl AsRefPtr<JsVirtualMachine>, it: &[(Symbol, MapEntry)]) -> Handle<Self> {
        let table = it.iter().copied().collect::<TargetTable>();
        let table = allocate_cell(vm.as_ref_ptr(), size_of::<TargetTable>(), table);
        let mut this = allocate_cell(
            vm,
            size_of::<Self>(),
            Self {
                prototype: None,
                previous: None,
                table: Some(table),
                transitions: Transitions::new(true, false),
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
            },
        );
        this.calculated_size = this.get_slots_size() as _;
        this
    }

    pub fn new(vm: impl AsRefPtr<JsVirtualMachine>, previous: Handle<Self>) -> Handle<Self> {
        Self::ctor(vm, previous, false)
    }

    pub fn new_unique(vm: impl AsRefPtr<JsVirtualMachine>, previous: Handle<Self>) -> Handle<Self> {
        Self::ctor(vm, previous, true)
    }
    pub fn new_indexed(
        vm: impl AsRefPtr<JsVirtualMachine>,
        prototype: Option<Handle<JsObject>>,
        indexed: bool,
    ) -> Handle<Self> {
        Self::ctor1(vm, prototype, false, indexed)
    }
    pub fn new_unique_indexed(
        vm: impl AsRefPtr<JsVirtualMachine>,
        prototype: Option<Handle<JsObject>>,
        indexed: bool,
    ) -> Handle<Self> {
        Self::ctor1(vm, prototype, !false, indexed)
    }

    pub fn new_from_point(
        vm: impl AsRefPtr<JsVirtualMachine>,
        map: Handle<Structure>,
    ) -> Handle<Self> {
        if map.is_unique() {
            return Self::new_unique(vm, map);
        }
        map
    }
    pub fn delete_property_transition(
        &mut self,
        vm: impl AsRefPtr<JsVirtualMachine>,
        name: Symbol,
    ) -> Handle<Self> {
        let x = vm.as_ref_ptr();
        let mut map = Self::new_unique(x, unsafe { Handle::from_raw(self) });
        if !map.has_table() {
            map.allocate_table(vm);
        }
        map.delete(x, name);
        map
    }

    pub fn change_attributes_transition(
        &mut self,
        vm: impl AsRefPtr<JsVirtualMachine>,
        name: Symbol,
        attributes: AttrSafe,
    ) -> Handle<Self> {
        let x = vm.as_ref_ptr();
        let mut map = Self::new_unique(x, unsafe { Handle::from_raw(self) });
        if !map.has_table() {
            map.allocate_table(vm);
        }
        map.change_attributes(name, attributes);
        map
    }

    pub fn get_own_property_names(
        &mut self,
        vm: impl AsRefPtr<JsVirtualMachine>,
        include: bool,
        mut collector: impl FnMut(Symbol, u32),
    ) {
        if self.allocate_table_if_needed(vm) {
            for entry in self.table.as_ref().unwrap().iter() {
                if entry.0.is_private() {
                    continue;
                }

                if entry.0.is_public() {
                    continue;
                }
                if include || entry.1.attrs.is_enumerable() {
                    collector(*entry.0, entry.1.offset);
                }
            }
        }
    }

    pub fn add_property_transition(
        &mut self,
        vm: impl AsRefPtr<JsVirtualMachine>,
        name: Symbol,
        attributes: AttrSafe,
        offset: &mut u32,
    ) -> Handle<Self> {
        let mut entry = MapEntry {
            offset: 0,
            attrs: attributes,
        };
        let vm = vm.as_ref_ptr();
        if self.is_unique() {
            if !self.has_table() {
                self.allocate_table(vm);
            }

            let mut map = if self.transitions.is_enabled_unique_transition() {
                Self::new_unique(vm, unsafe { Handle::from_raw(self) })
            } else {
                unsafe { Handle::from_raw(self) }
            };
            if !map.deleted.empty() {
                entry.offset = map.deleted.pop();
            } else {
                entry.offset = self.get_slots_size() as _;
            }
            unwrap_unchecked(map.table.as_mut()).insert(name, entry);
            return map;
        }

        // existing transition check
        if let Some(map) = self.transitions.find(name, attributes) {
            *offset = map.added.1.offset;
            return map;
        }
        if self.transit_count > 32 {
            // stop transition
            let mut map = Self::new_unique(vm, unsafe { Handle::from_raw(self) });
            // go to above unique path
            return map.add_property_transition(vm, name, attributes, offset);
        }
        let mut map = Self::new(vm, unsafe { Handle::from_raw(self) });
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
        self.transitions.insert(vm, name, attributes, map);
        *offset = map.added.1.offset;

        map
    }

    pub fn get(&mut self, vm: impl AsRefPtr<JsVirtualMachine>, name: Symbol) -> MapEntry {
        if !self.has_table() {
            if self.previous.is_none() {
                return MapEntry::not_found();
            }
            if self.is_adding_map() {
                if self.added.0 == name {
                    return self.added.1;
                }
            }
            self.allocate_table(vm);
        }
        let it = self.table.as_ref().unwrap().get(&name);
        it.copied().unwrap_or(MapEntry::not_found())
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
}

impl Handle<Structure> {
    pub fn change_prototype_with_no_transition(&mut self, prototype: Handle<JsObject>) -> Self {
        self.prototype = Some(prototype);
        *self
    }
}
