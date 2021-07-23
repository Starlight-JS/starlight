#![allow(incomplete_features)]
#![feature(specialization)]

use starlight::{
    define_jsclass,
    jsrt::VM_NATIVE_REFERENCES,
    prelude::*,
    prelude::{JsClass, Options},
    vm::{
        builder::{ClassBuilder, ClassConstructor},
        context::Context,
    },
    Platform,
};

pub struct Person {
    age: i32,
}

impl ClassConstructor for Person {
    fn constructor(
        _ctx: GcPointer<Context>,
        args: &starlight::prelude::Arguments<'_>,
    ) -> Result<Self, JsValue> {
        Ok(Person {
            age: args.at(0).get_int32(),
        })
    }
    fn init(builder: &mut ClassBuilder) -> Result<(), JsValue> {
        builder.method("sayHello", Person::say_hello, 0)?;
        Ok(())
    }
}

impl Person {
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
