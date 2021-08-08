use crate::{
    prelude::*,
    vm::{builder::Builtin, context::Context, object::TypedJsObject},
    JsTryFrom,
};
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

impl Builtin for JsSymbolObject {
    fn native_references() -> Vec<usize> {
        vec![
            symbol_ctor as _,
            symbol_for as _,
            symbol_key_for as _,
            symbol_to_string as _,
            symbol_value_of as _,
        ]
    }
    fn init(mut ctx: GcPointer<Context>) -> Result<(), JsValue> {
        let obj_proto = ctx.global_data.object_prototype.unwrap();
        ctx.global_data.symbol_structure = Some(Structure::new_indexed(ctx, None, false));
        let structure = Structure::new_indexed(ctx, Some(obj_proto), false);
        let mut prototype = JsObject::new(ctx, &structure, JsObject::class(), ObjectTag::Ordinary);
        ctx.global_data
            .symbol_structure
            .unwrap()
            .change_prototype_with_no_transition(prototype);
        ctx.global_data.symbol_prototype = Some(prototype);
        def_native_method!(ctx, prototype, toString, symbol_to_string, 0)?;
        def_native_method!(ctx, prototype, valueOf, symbol_value_of, 0)?;

        let mut constructor = JsNativeFunction::new(ctx, "Symbol".intern(), symbol_ctor, 1);

        def_native_method!(ctx, constructor, for, symbol_for, 1)?;
        def_native_method!(ctx, constructor, keyFor, symbol_key_for, 1)?;
        builtin_symbols!(ctx, constructor, def_symbols);
        def_native_property!(ctx, constructor, prototype, prototype, NONE)?;
        def_native_property!(ctx, prototype, constructor, constructor, W | C)?;

        ctx.global_object()
            .put(ctx, "Symbol".intern(), JsValue::new(constructor), false)?;

        Ok(())
    }
}

pub fn symbol_ctor(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if unlikely(args.ctor_call) {
        return Err(JsValue::new(
            ctx.new_type_error("Symbol is not an constructor"),
        ));
    }

    let arg = args.at(0).to_string(ctx)?.intern();
    Ok(JsValue::new(JsSymbol::new(ctx, arg)))
}
pub fn symbol_for(mut ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let arg = args.at(0).to_string(ctx)?.intern();

    if let Some(sym) = ctx.symbol_table.get(&arg) {
        Ok(JsValue::new(*sym))
    } else {
        let sym = JsSymbol::new(ctx, arg);
        ctx.symbol_table.insert(arg, sym);
        Ok(JsValue::new(sym))
    }
}

pub fn symbol_key_for(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let sym = TypedJsObject::<JsSymbolObject>::try_from(ctx, args.at(0))?
        .symbol()
        .symbol();
    let desc = ctx.description(sym);
    Ok(JsValue::new(JsString::new(ctx, desc)))
}

pub fn symbol_to_string(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let sym = TypedJsObject::<JsSymbolObject>::try_from(ctx, args.this)?
        .symbol()
        .symbol();
    let desc = ctx.description(sym);
    Ok(JsValue::new(JsString::new(
        ctx,
        format!("Symbol({})", desc),
    )))
}

pub fn symbol_value_of(_ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    Ok(JsValue::new(
        TypedJsObject::<JsSymbolObject>::try_from(_ctx, args.this)?.symbol(),
    ))
}
