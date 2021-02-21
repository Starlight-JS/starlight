use crate::gc::cell::{Cell, Trace};
use crate::heap::Allocator;
use std::hash::{Hash, Hasher};
pub const DUMMY_SYMBOL: Symbol = Symbol::Key("");
/// Runtime symbol type.
///
///
/// This type is used as property names and inside JsSymbol.
#[derive(Clone, Copy, Eq)]
pub enum Symbol {
    /// Represents index value, this variant is used when you can definetely put array
    /// index inside u32 so it does not take space in interner heap.
    Indexed(u32),
    /// Interned string.
    Key(&'static str),
}
impl Symbol {
    pub fn as_string(&self) -> String {
        match self {
            Self::Indexed(x) => x.to_string(),
            Self::Key(x) => x.to_string(),
        }
    }
}
impl PartialEq for Symbol {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Indexed(x), Self::Indexed(y)) => x == y,
            (Self::Key(x), Self::Key(y)) => x.as_ptr() == y.as_ptr(),
            _ => false,
        }
    }
}

impl Hash for Symbol {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Symbol::Indexed(x) => {
                state.write_u8(0xff);
                state.write_u32(*x);
            }
            Symbol::Key(x) => {
                state.write_u8(0xfa);
                state.write_usize(x.as_ptr() as _);
            }
        }
    }
}

#[cfg(feature = "debug-snapshots")]
impl serde::Serialize for Symbol {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut x = serializer.serialize_struct("Symbol", 1)?;
        x.serialize_field("as_str", &self.as_string())?;
        x.end()
    }
}
impl Cell for Symbol {}
unsafe impl Trace for Symbol {}

#[derive(Eq, PartialEq, Hash)]
pub struct JsSymbol {
    sym: Symbol,
}

impl JsSymbol {
    pub fn new<A: Allocator<Self>>(allocator: &mut A, sym: Symbol) -> A::Result {
        allocator.allocate(Self { sym })
    }

    pub fn sym(&self) -> Symbol {
        self.sym
    }
}

unsafe impl Trace for JsSymbol {}
impl Cell for JsSymbol {}

#[cfg(feature = "debug-snapshots")]
impl serde::Serialize for JsSymbol {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut x = serializer.serialize_struct("JsSymbol", 1)?;
        x.serialize_field("sym", &self.sym)?;
        x.end()
    }
}

macro_rules! default_symbols {
    ($f: ident) => {
        $f! {
                length,
        eval,
        arguments,
        caller,
        callee,
        toString,
        toLocaleString,
        toJSON,
        valueOf,
        prototype,
        constructor,
        undefined,
        NaN,
        Infinity,
        null,
        name,
        message,
        stack,
        get,
        set,
        value,
        done,
        next,
        configurable,
        writable,
        enumerable,
        lastIndex,
        index,
        input,
        ignoreCase,
        multiline,
        global,
        source,
        compare,
        join,
        Intl,
        Collator,
        NumberFormat,
        DateTimeFormat,
        usage,
        localeMatcher,
        style,
        currency,
        currencyDisplay,
        minimumIntegerDigits,
        minimumFractionDigits,
        maximumFractionDigits,
        minimumSignificantDigits,
        maximumSignificantDigits,
        useGrouping,
        timeZone,
        hour12,
        formatMatcher,
        raw,
        byteLength,
        buffer,
        byteOffset,
        add,
        toPrimitive,
        __param_mapping
            }
    };
}

macro_rules! def_syms {
    ($($name:ident),*) => {
        impl Symbol {
            $(
                #[allow(non_snake_case)]
                pub fn $name() -> Self {
                    static SYM: &'static str = stringify!($name);
                    Self::Key(SYM)
                }
            )*
        }
    };
}

default_symbols!(def_syms);
