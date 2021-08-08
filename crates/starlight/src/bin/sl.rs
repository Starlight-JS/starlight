/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use starlight::vm::context::Context;
use starlight::{letroot, prelude::*};
use structopt::*;

#[cfg(not(debug_assertions))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    Platform::initialize();
    let options = Options::from_args();

    let mut vm = VirtualMachine::new(options, None);

    #[cfg(all(target_pointer_width = "64", feature = "ffi"))]
    if vm.options().enable_ffi {
        vm.add_ffi();
    }

    let mut ctx = Context::new(&mut vm);

    let string = std::fs::read_to_string(&vm.options().file);
    match string {
        Ok(source) => {
            let name = vm.options().file.as_os_str().to_str().unwrap().to_string();
            letroot!(
                function = foo,
                match ctx.compile_module(&name, "<script>", &source) {
                    Ok(function) => function.get_jsobject(),
                    Err(e) => {
                        let string = e.to_string(ctx);
                        match string {
                            Ok(val) => {
                                eprintln!("Compilation failed: {}", val);
                                std::process::exit(1);
                            }
                            Err(_e) => {
                                eprintln!("Failed to get error as string");
                                std::process::exit(1);
                            }
                        }
                    }
                }
            );

            let global = ctx.global_object();
            let mut module_object = JsObject::new_empty(ctx);
            let exports = JsObject::new_empty(ctx);
            module_object
                .put(ctx, "@exports".intern(), JsValue::new(exports), false)
                .unwrap_or_else(|_| unreachable!());
            let mut args = [JsValue::new(module_object)];
            let mut args = Arguments::new(JsValue::encode_object_value(global), &mut args);

            let start = std::time::Instant::now();
            let f = function;
            match function
                .as_function_mut()
                .call(ctx, &mut args, JsValue::new(f))
            {
                Ok(_) => {
                    let elapsed = start.elapsed();
                    eprintln!("Executed in {}ms", elapsed.as_nanos() as f64 / 1000000f64);
                }
                Err(e) => {
                    let str = match e.to_string(ctx) {
                        Ok(s) => s,
                        Err(_) => "<unknown error>".to_owned(),
                    };
                    eprintln!("Uncaught exception: {}", str);
                    eprintln!("Stacktrace: \n{}", ctx.take_stacktrace());
                }
            }
        }
        Err(error) => {
            eprintln!("Error while reading JS source: {}", error);
            std::process::exit(1);
        }
    }
    unsafe {
        vm.dispose();
    }

    std::process::exit(0);
}
