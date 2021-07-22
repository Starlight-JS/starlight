use std::mem::ManuallyDrop;

use starlight::{
    def_native_method, def_native_property, define_jsclass,
    jsrt::VM_NATIVE_REFERENCES,
    prelude::*,
    prelude::{JsClass, Options},
    vm::context::Context,
    Platform,
};

pub struct Person {
    age: i32,
}

pub fn person_constructor(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let mut res = 0;
    if args.size() != 0 {
        res = args.at(0).to_int32(ctx)?;
    }
    Ok(JsValue::new(Person::new(ctx, res)))
}

impl Person {
    pub fn new(ctx: GcPointer<Context>, age: i32) -> GcPointer<JsObject> {
        let structure = ctx.global_data().get_structure("Person".intern()).unwrap();
        let obj = JsObject::new(ctx, &structure, Self::class(), ObjectTag::Ordinary);
        *obj.data::<Self>() = ManuallyDrop::new(Self { age });
        obj
    }

    pub fn say_hello(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
        let mut obj = args.this.to_object(ctx).unwrap();
        let person = obj.as_data::<Person>();
        println!("Hello {}", person.age);
        Ok(JsValue::UNDEFINED)
    }
}

impl JsClass for Person {
    fn class() -> &'static starlight::prelude::Class {
        define_jsclass!(Person, Person)
    }

    fn init(mut ctx: GcPointer<Context>) -> Result<(), JsValue> {
        let obj_proto = ctx.global_data().get_object_prototype();
        let structure = Structure::new_unique_indexed(ctx, Some(obj_proto), false);
        let mut proto = JsObject::new(ctx, &structure, Self::class(), ObjectTag::Ordinary);

        let structure = Structure::new_indexed(ctx, Some(proto), false);
        let mut constructor = JsNativeFunction::new(ctx, "Person".intern(), person_constructor, 1);

        def_native_property!(ctx, constructor, prototype, proto)?;
        def_native_property!(ctx, proto, constructor, constructor)?;
        def_native_method!(ctx, proto, sayHello, Person::say_hello, 0)?;

        ctx.register_structure("Person".intern(), structure);

        let mut global_object = ctx.global_object();
        def_native_property!(ctx, global_object, Person, constructor)?;

        Ok(())
    }
}

fn main() {
    Platform::initialize();
    let mut runtime = Platform::new_runtime(Options::default(), None);
    let mut ctx = runtime.new_context();

    ctx.register_class::<Person>().unwrap();

    match ctx.eval("let person = new Person(10);person.sayHello()") {
        Err(e) => {
            println!("{}", e.to_string(ctx).unwrap());
        }
        _ => {}
    }

    unsafe {
        VM_NATIVE_REFERENCES.push(Person::say_hello as _);
        VM_NATIVE_REFERENCES.push(person_constructor as _);
        VM_NATIVE_REFERENCES.push(Person::class() as *const _ as _);
    }

    let buf = Snapshot::take_context(false, &mut runtime, ctx, |_, _| {}).buffer;
    let mut ctx = Deserializer::deserialize_context(&mut runtime, false, &buf);
    match ctx.eval("let person = new Person(10);person.sayHello()") {
        Err(e) => {
            println!("{}", e.to_string(ctx).unwrap());
        }
        _ => {}
    }
}
