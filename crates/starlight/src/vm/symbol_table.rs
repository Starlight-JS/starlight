use super::class::{Class, JsClass};
/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use super::method_table::*;
use super::object::ObjectTag;
use super::Context;
use crate::gc::cell::{GcCell, GcPointer, Trace, Tracer};
use crate::gc::snapshot::deserializer::{Deserializable, Deserializer};
use crate::gc::snapshot::serializer::SnapshotSerializer;
use crate::prelude::*;
use crate::vm::object::JsObject;
use dashmap::DashMap;
use std::mem::ManuallyDrop;
use std::sync::atomic::Ordering;
use std::{mem::MaybeUninit, sync::atomic::AtomicU32};
pub struct SymbolTable {
    pub(crate) symbols: DashMap<&'static str, u32>,
    pub(crate) ids: DashMap<u32, &'static str>,
    key: AtomicU32,
}
impl Drop for SymbolTable {
    fn drop(&mut self) {
        for entry in self.ids.iter_mut() {
            let key = entry.value();
            unsafe {
                let _ = Box::from_raw((*key) as *const _ as *mut str);
            }
        }
        self.symbols.clear();
        self.ids.clear();
    }
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}
impl SymbolTable {
    pub fn new() -> Self {
        Self {
            symbols: DashMap::with_capacity(0),
            ids: DashMap::with_capacity(0),
            key: AtomicU32::new(128),
        }
    }

    pub fn description(&self, symbol: SymbolID) -> &'static str {
        *self.ids.get(&symbol.0).unwrap()
    }
    pub fn intern(&self, val: impl AsRef<str>) -> SymbolID {
        let string = val.as_ref();
        if let Some(key) = self.symbols.get(string) {
            return SymbolID(*key.value());
        }

        let string = Box::leak(string.to_string().into_boxed_str());
        let make_new_key = || self.key.fetch_add(1, Ordering::Relaxed);
        let key = *self
            .symbols
            .entry(string)
            .or_insert_with(make_new_key)
            .value();
        self.ids.insert(key, string);
        SymbolID(key)
    }
}

macro_rules! builtin_symbols {
    ($m: ident) => {
        $m! {
            /*PROTOTYPE prototype 0,
            TO_STRING toString 1,
            CONSTRUCTOR constructor 2,
            LENGTH length 3,
            BYTE_LENGTH byteLength 4,
            GET get 5,
            SET set 6,
            CALL call 7,
            APPLY apply 8*/

        }
    };
}

macro_rules! def_sid {
    ($($id: ident $val: ident $ix: expr),*) => {
        $(pub const $id: SymbolID = SymbolID($ix);)*
    };
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct SymbolID(pub(crate) u32);

impl SymbolID {
    builtin_symbols! {
        def_sid
    }

    pub const PUBLIC_START: SymbolID = Self(128);
}
/// Runtime symbol type.
///
///
/// This type is used as property names and inside JsSymbol.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub enum Symbol {
    /// Interned string.
    Key(SymbolID),
    /// Private symbol. You can't create it in JS world.
    Private(SymbolID),
    /// Represents index value, this variant is used when you can definetely put array
    /// index inside u32 so it does not take space in interner gc.
    Index(u32),
}

macro_rules! def_sym {
    ($($id: ident $val: ident $ix: expr),*) => {
        $(
            pub const $id: Symbol = Symbol::Key(SymbolID::$id);
        )*
    };
}

impl Symbol {
    builtin_symbols! {
        def_sym
    }
    pub fn private(self) -> Self {
        match self {
            Self::Key(x) => Self::Private(x),
            _ => unreachable!(),
        }
    }
    pub fn get_id(self) -> SymbolID {
        match self {
            Self::Key(x) => x,
            Self::Private(x) => x,
            _ => unreachable!(),
        }
    }
    pub fn is_index(self) -> bool {
        /*match self {
            Self::Index(_) => true,
            _ => false,
        }*/
        matches!(self, Self::Index(_))
    }
    pub fn get_index(self) -> u32 {
        match self {
            Self::Index(x) => x,
            _ => unreachable!(),
        }
    }
    pub fn is_key(self) -> bool {
        !self.is_index()
    }
}
impl GcCell for Symbol {
    fn deser_pair(&self) -> (usize, usize) {
        (Self::deserialize as _, Self::allocate as _)
    }
}
unsafe impl Trace for Symbol {}

pub const DUMMY_SYMBOL: Symbol = Symbol::Key(SymbolID(0));

#[no_mangle]
#[doc(hidden)]
pub static mut SYMBOL_TABLE: MaybeUninit<SymbolTable> = MaybeUninit::uninit();

macro_rules! globals {
    ($($id: ident $val: ident $ix: expr),*) => {
       $( pub static $id: &'static str = stringify!($val);)*
    };
}
builtin_symbols!(globals);
macro_rules! intern_builtins {
    ($($id: ident $val: ident $ix: expr),*) => {
        let mut _symtab = symbol_table();
        $(
            _symtab.ids.insert($ix,$id);
            _symtab.symbols.insert($id,$ix);
        )*
    };
}
pub(crate) fn initialize_symbol_table() {
    unsafe {
        SYMBOL_TABLE.as_mut_ptr().write(SymbolTable::new());
        LENGTH = "length".intern();
    }
    builtin_symbols!(intern_builtins);
}

pub fn length_id() -> Symbol {
    unsafe { LENGTH }
}
pub fn symbol_table() -> &'static SymbolTable {
    unsafe { &*SYMBOL_TABLE.as_ptr() }
}
pub trait Internable {
    fn intern(&self) -> Symbol;
}

impl Internable for str {
    fn intern(&self) -> Symbol {
        Symbol::Key(symbol_table().intern(self))
    }
}

impl Internable for String {
    fn intern(&self) -> Symbol {
        Symbol::Key(symbol_table().intern(self))
    }
}

impl Internable for u32 {
    fn intern(&self) -> Symbol {
        Symbol::Index(*self)
    }
}

impl Internable for usize {
    fn intern(&self) -> Symbol {
        if *self as u32 as usize == *self {
            return (*self as u32).intern();
        }
        self.to_string().intern()
    }
}

pub struct JsSymbol {
    pub(crate) sym: Symbol,
}

impl JsSymbol {
    pub fn new(mut ctx: GcPointer<Context>, sym: Symbol) -> GcPointer<Self> {
        ctx.heap().allocate(Self { sym })
    }

    pub fn symbol(&self) -> Symbol {
        self.sym
    }
}

unsafe impl Trace for JsSymbol {}
impl GcCell for JsSymbol {
    fn deser_pair(&self) -> (usize, usize) {
        (Self::deserialize as _, Self::allocate as _)
    }
}

impl std::fmt::Display for SymbolID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", symbol_table().description(*self))
    }
}

static mut LENGTH: Symbol = Symbol::Key(SymbolID(0));

pub struct JsSymbolObject {
    sym: GcPointer<JsSymbol>,
}

extern "C" fn fsz() -> usize {
    std::mem::size_of::<JsSymbolObject>()
}

extern "C" fn ser(_: &JsObject, _: &mut SnapshotSerializer) {
    todo!()
}

extern "C" fn deser(_: &mut JsObject, _: &mut Deserializer) {
    todo!()
}
#[allow(improper_ctypes_definitions)]
extern "C" fn trace(tracer: &mut dyn Tracer, obj: &mut JsObject) {
    obj.data::<JsSymbolObject>().sym.trace(tracer);
}

impl JsClass for JsSymbolObject {
    fn class() -> &'static Class {
        define_jsclass!(
            JsSymbolObject,
            Symbol,
            None,
            Some(trace),
            Some(deser),
            Some(ser),
            Some(fsz)
        )
    }
}
impl JsSymbolObject {
    pub fn symbol(&self) -> GcPointer<JsSymbol> {
        self.sym
    }

    pub fn new(ctx: GcPointer<Context>, sym: GcPointer<JsSymbol>) -> GcPointer<JsObject> {
        let map = ctx.global_data().symbol_structure.unwrap();
        let mut obj = JsObject::new(ctx, &map, Self::class(), ObjectTag::Ordinary);
        *obj.data::<Self>() = ManuallyDrop::new(Self { sym });
        obj
    }
}
