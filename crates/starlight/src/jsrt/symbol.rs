use crate::{prelude::*};
use std::intrinsics::unlikely;

macro_rules! builtin_symbols {
    ($rt: expr,$ctor: expr,$m: ident) => {
        $m! { $rt,$ctor,
            "asyncIterator",
            "hasInstance",
            "isConcatSpreadable",
            "iterator",
            "match",
            "matchAll",
            "replace",
            "search",
            "species",
            "split",
            "toPrimitive",
            "toStringTag",
            "unscopables"
        }
    };
}

macro_rules! def_symbols {
    ($rt: expr,$ctor: expr,$($name : literal),*) => {
        $(
            let name = format!("Symbol.{}",$name);
            let sym = JsSymbol::new($rt,name.intern().private());

            $ctor.define_own_property($rt,$name.intern(),&*DataDescriptor::new(JsValue::new(sym),NONE),false)?;
        )*
    }
}

impl Runtime {
    pub(crate) fn init_symbol_in_realm(&mut self) {
        let mut init = || -> Result<(), JsValue> {
            let mut ctor = JsNativeFunction::new(self, "Symbol".intern(), symbol_ctor, 1);

            def_native_method!(self, ctor, for, symbol_for, 1)?;
            def_native_method!(self, ctor, keyFor, symbol_key_for, 1)?;
            builtin_symbols!(self, ctor, def_symbols);
            self.realm()
                .global_object()
                .put(self, "Symbol".intern(), JsValue::new(ctor), false)?;
            Ok(())
        };
        match init() {
            Ok(_) => (),
            Err(_) => unreachable!(),
        }
    }

    pub(crate) fn init_symbol_in_global_data(&mut self, proto: GcPointer<JsObject>) {
        let mut init = || -> Result<(), JsValue> {
            let structure = Structure::new_indexed(self, Some(proto), false);
            let mut sym_proto =
                JsObject::new(self, &structure, JsObject::get_class(), ObjectTag::Ordinary);
            self.global_data.symbol_prototype = Some(sym_proto);
            def_native_method!(self, sym_proto, toString, symbol_to_string, 0)?;
            def_native_method!(self, sym_proto, valueOf, symbol_value_of, 0)?;
            Ok(())
        };

        match init() {
            Ok(_) => (),
            Err(_) => unreachable!(),
        }
    }
}

pub fn symbol_ctor(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    if unlikely(args.ctor_call) {
        return Err(JsValue::new(
            rt.new_type_error("Symbol is not an constructor"),
        ));
    }

    let arg = args.at(0).to_string(rt)?.intern();
    Ok(JsValue::new(JsSymbol::new(rt, arg)))
}
pub fn symbol_for(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let arg = args.at(0).to_string(rt)?.intern();

    if let Some(sym) = rt.symbol_table.get(&arg) {
        Ok(JsValue::new(*sym))
    } else {
        let sym = JsSymbol::new(rt, arg);
        rt.symbol_table.insert(arg, sym);
        Ok(JsValue::new(sym))
    }
}

pub fn symbol_key_for(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let sym = args.at(0).to_symbol(rt)?;
    let desc = rt.description(sym);
    Ok(JsValue::new(JsString::new(rt, desc)))
}

pub fn symbol_to_string(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let sym = args.this.to_symbol(rt)?;
    let desc = rt.description(sym);
    Ok(JsValue::new(JsString::new(rt, format!("Symbol({})", desc))))
}

pub fn symbol_value_of(_rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    Ok(args.this)
}
