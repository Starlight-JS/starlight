use crate::{prelude::*, vm::Context};
use std::intrinsics::unlikely;

macro_rules! builtin_symbols {
    ($ctx: expr,$ctor: expr,$m: ident) => {
        $m! { $ctx,$ctor,
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
    ($ctx: expr,$ctor: expr,$($name : literal),*) => {
        $(
            let name = format!("Symbol.{}",$name);
            let sym = JsSymbol::new($ctx,name.intern().private());

            $ctor.define_own_property($ctx,$name.intern(),&*DataDescriptor::new(JsValue::new(sym),NONE),false)?;
        )*
    }
}

impl Context {
    pub(crate) fn init_symbol_in_realm(&mut self) {
        let mut init = || -> Result<(), JsValue> {
            let name = "constructor".intern();
            let constructor = self.global_data.symbol_prototype.unwrap().get_own_property(self, name).unwrap().value();
            self.global_object()
                .put(self, "Symbol".intern(), JsValue::new(constructor), false)?;
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

            let mut ctor = JsNativeFunction::new(self, "Symbol".intern(), symbol_ctor, 1);

            def_native_method!(self, ctor, for, symbol_for, 1)?;
            def_native_method!(self, ctor, keyFor, symbol_key_for, 1)?;
            builtin_symbols!(self, ctor, def_symbols);
            
            let name = "prototype".intern();
            ctor.define_own_property(self, name, &*DataDescriptor::new(JsValue::from(sym_proto), NONE) , false)?;
            
            let name = "constructor".intern();
            sym_proto.define_own_property(self, name, &*DataDescriptor::new(JsValue::from(ctor), W | C), false)?;

            Ok(())
        };

        match init() {
            Ok(_) => (),
            Err(_) => unreachable!(),
        }
    }
}

pub fn symbol_ctor(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    if unlikely(args.ctor_call) {
        return Err(JsValue::new(
            ctx.new_type_error("Symbol is not an constructor"),
        ));
    }

    let arg = args.at(0).to_string(ctx)?.intern();
    Ok(JsValue::new(JsSymbol::new(ctx, arg)))
}
pub fn symbol_for(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let arg = args.at(0).to_string(ctx)?.intern();

    if let Some(sym) = ctx.symbol_table.get(&arg) {
        Ok(JsValue::new(*sym))
    } else {
        let sym = JsSymbol::new(ctx, arg);
        ctx.symbol_table.insert(arg, sym);
        Ok(JsValue::new(sym))
    }
}

pub fn symbol_key_for(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let sym = args.at(0).to_symbol(ctx)?;
    let desc = ctx.description(sym);
    Ok(JsValue::new(JsString::new(ctx, desc)))
}

pub fn symbol_to_string(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let sym = args.this.to_symbol(ctx)?;
    let desc = ctx.description(sym);
    Ok(JsValue::new(JsString::new(ctx, format!("Symbol({})", desc))))
}

pub fn symbol_value_of(_ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    Ok(args.this)
}
