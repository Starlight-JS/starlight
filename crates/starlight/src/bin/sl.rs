/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use starlight::gc::default_heap;
use starlight::prelude::*;
use starlight::vm::context::Context;
use std::path::Path;
use structopt::*;

#[cfg(not(debug_assertions))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use const_random::const_random;
const BIN_ID: u64 = const_random!(u64);
const SNAPSHOT_FILENAME: &str = ".startup-snapshot";
fn main() {
    Platform::initialize();
    let options = Options::from_args();

    let mut deserialized = false;
    let mut rt = if Path::new(SNAPSHOT_FILENAME).exists() {
        let mut src = std::fs::read(SNAPSHOT_FILENAME);
        match src {
            Ok(ref mut src) => {
                let mut bytes: [u8; 8] = [0; 8];
                bytes.copy_from_slice(&src[0..8]);
                let heap = default_heap(&options);
                if u64::from_ne_bytes(bytes) != BIN_ID {
                    Runtime::with_heap(heap, options, None)
                } else {
                    let snapshot = &src[8..];
                    deserialized = true;
                    let heap = default_heap(&options);
                    Deserializer::deserialize(false, snapshot, options, heap, None, |_, _| {})
                }
            }
            Err(_) => {
                let heap = default_heap(&options);
                Runtime::with_heap(heap, options, None)
            }
        }
    } else {
        let heap = default_heap(&options);
        Runtime::with_heap(heap, options, None)
    };

    #[cfg(all(target_pointer_width = "64", feature = "ffi"))]
    if rt.options().enable_ffi {
        rt.add_ffi();
    }
    if !deserialized {
        let snapshot = Snapshot::take(false, &mut rt, |_, _| {});
        let mut buf = Vec::<u8>::with_capacity(8 + snapshot.buffer.len());
        buf.extend(&BIN_ID.to_ne_bytes());
        buf.extend(snapshot.buffer.iter());
        std::fs::write(SNAPSHOT_FILENAME, &buf).unwrap();
    }

    let gcstack = rt.shadowstack();

    let mut ctx = Context::new(&mut rt);

    let string = std::fs::read_to_string(&rt.options().file);
    match string {
        Ok(source) => {
            let name = rt.options().file.as_os_str().to_str().unwrap().to_string();
            letroot!(
                function = gcstack,
                match ctx.compile_module(&name, "<script>", &source,) {
                    Ok(function) => function.get_jsobject(),
                    Err(e) => {
                        let string = e.to_string(&mut ctx);
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
            letroot!(funcc = gcstack, *function);
            let global = ctx.global_object();
            letroot!(module_object = gcstack, JsObject::new_empty(&mut ctx));
            let exports = JsObject::new_empty(&mut ctx);
            module_object
                .put(&mut ctx, "@exports".intern(), JsValue::new(exports), false)
                .unwrap_or_else(|_| unreachable!());
            let mut args = [JsValue::new(*module_object)];
            letroot!(
                args = gcstack,
                Arguments::new(JsValue::encode_object_value(global), &mut args)
            );

            let start = std::time::Instant::now();
            match function
                .as_function_mut()
                .call(&mut ctx, &mut args, JsValue::new(*funcc))
            {
                Ok(_) => {
                    let elapsed = start.elapsed();
                    eprintln!("Executed in {}ms", elapsed.as_nanos() as f64 / 1000000f64);
                }
                Err(e) => {
                    let str = match e.to_string(&mut ctx) {
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

    drop(rt);
    std::process::exit(0);
}
