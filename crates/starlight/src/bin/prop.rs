use starlight::{Platform, prelude::{Arguments, C, DataDescriptor, GcPointer, Internable, JsObject, JsValue, Options, W}, vm::context::Context};

fn nop(_ctx: GcPointer<Context>,_args: &Arguments) -> Result<JsValue, JsValue>{
    Ok(JsValue::encode_undefined_value())
}

fn main(){
    Platform::initialize();
    let mut runtime = Platform::new_runtime(Options::default(), None);
    let ctx = runtime.new_context();

    let mut a = JsObject::new_empty(ctx);
    
    let desc = DataDescriptor::new(JsValue::encode_int32(1),W | C);
    a.define_own_property(ctx, "a".intern(), &desc ,false ).unwrap();

    let desc = DataDescriptor::new(JsValue::encode_int32(10),W | C);
    a.define_own_property(ctx, "b".intern(), &desc, false).unwrap();

    a.delete_non_indexed(ctx, "a".intern(), false).unwrap();
    
    let desc = DataDescriptor::new(JsValue::encode_int32(1),W | C);
    a.define_own_property(ctx, "a".intern(), &desc , false).unwrap();

    println!("{}",a.get_own_property(ctx, "b".intern()).unwrap().value().to_int32(ctx).unwrap());
}