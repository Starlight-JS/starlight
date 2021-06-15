/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use starlight::gc::formatted_size;
use starlight::prelude::*;
use std::path::{Path, PathBuf};
use structopt::*;

#[cfg(not(debug_assertions))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Debug, StructOpt)]
struct Options {
    #[structopt(
        long = "gc-threads",
        default_value = "4",
        help = "Set number of GC marker threads"
    )]
    gc_threads: u32,
    #[structopt(long = "parallel-gc", help = "Enable parallel marking GC")]
    parallel_marking: bool,
    #[structopt(parse(from_os_str), help = "Input JS file")]
    file: PathBuf,
    #[structopt(short = "d", long = "dump-bytecode", help = "Dump bytecode")]
    dump_bytecode: bool,
    #[structopt(long = "disable-ic", help = "Disable inline caching")]
    disable_ic: bool,
    #[structopt(
        long = "enable-malloc-gc",
        help = "Enable MallocGC, use this GC only for debugging purposes!"
    )]
    use_malloc_gc: bool,
    #[structopt(long = "enable-ffi", help = "Enable FFI and CFunction objects for use")]
    enable_ffi: bool,
    #[structopt(
        long = "dump-stats",
        help = "Dump various statistics at the end of execution"
    )]
    dump_stats: bool,
    #[structopt(
        long = "conservative-marking",
        help = "Enable conservative pointer marking (works only for MiGC)"
    )]
    cons_gc: bool,
    #[structopt(
        long = "enable-region-gc",
        help = "Enable region based garbage collector"
    )]
    region_gc: bool,
}

use const_random::const_random;
const BIN_ID: u64 = const_random!(u64);
const SNAPSHOT_FILENAME: &'static str = ".startup-snapshot";
fn main() {
    Platform::initialize();
    let options = Options::from_args();

    let gc = if options.parallel_marking {
        GcParams::default()
            .with_parallel_marking(true)
            .with_marker_threads(options.gc_threads)
    } else {
        GcParams::default().with_parallel_marking(false)
    };
    let gc = gc.with_conservative_marking(options.cons_gc);
    let heap = if options.use_malloc_gc {
        Heap::new(starlight::gc::malloc_gc::MallocGC::new(gc))
    } else {
        Heap::new(starlight::gc::migc::MiGC::new(gc))
    };
    let mut deserialized = false;
    let mut rt = if Path::new(SNAPSHOT_FILENAME).exists() {
        let mut src = std::fs::read(SNAPSHOT_FILENAME);
        match src {
            Ok(ref mut src) => {
                let mut bytes: [u8; 8] = [0; 8];
                bytes.copy_from_slice(&src[0..8]);
                if u64::from_ne_bytes(bytes) != BIN_ID {
                    Runtime::with_heap(
                        heap,
                        RuntimeParams::default()
                            .with_dump_bytecode(options.dump_bytecode)
                            .with_inline_caching(!options.disable_ic),
                        None,
                    )
                } else {
                    let snapshot = &src[8..];
                    deserialized = true;

                    Deserializer::deserialize(
                        false,
                        snapshot,
                        RuntimeParams::default()
                            .with_dump_bytecode(options.dump_bytecode)
                            .with_inline_caching(!options.disable_ic),
                        heap,
                        None,
                        |_, _| {},
                    )
                }
            }
            Err(_) => Runtime::with_heap(
                heap,
                RuntimeParams::default()
                    .with_dump_bytecode(options.dump_bytecode)
                    .with_inline_caching(!options.disable_ic),
                None,
            ),
        }
    } else {
        Runtime::with_heap(
            heap,
            RuntimeParams::default()
                .with_dump_bytecode(options.dump_bytecode)
                .with_inline_caching(!options.disable_ic),
            None,
        )
    };

    #[cfg(all(target_pointer_width = "64", feature = "ffi"))]
    if options.enable_ffi {
        rt.add_ffi();
    }
    if !deserialized {
        let snapshot = Snapshot::take(false, &mut rt, |_, _| {});
        let mut buf = Vec::<u8>::with_capacity(8 + snapshot.buffer.len());
        buf.extend(&BIN_ID.to_ne_bytes());
        buf.extend(snapshot.buffer.iter());
        std::fs::write(SNAPSHOT_FILENAME, &buf).unwrap();
    }
    let at_init = rt.heap().gc.stats();
    let gcstack = rt.shadowstack();

    let string = std::fs::read_to_string(&options.file);
    match string {
        Ok(source) => {
            letroot!(
                function = gcstack,
                match rt.compile_module(
                    options.file.as_os_str().to_str().unwrap(),
                    "<script>",
                    &source,
                ) {
                    Ok(function) => function.get_jsobject(),
                    Err(e) => {
                        let string = e.to_string(&mut rt);
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
            letroot!(funcc = gcstack, *&*function);
            let global = rt.global_object();
            letroot!(module_object = gcstack, JsObject::new_empty(&mut rt));
            let exports = JsObject::new_empty(&mut rt);
            module_object
                .put(&mut rt, "@exports".intern(), JsValue::new(exports), false)
                .unwrap_or_else(|_| unreachable!());
            let mut args = [JsValue::new(*&*module_object)];
            letroot!(
                args = gcstack,
                Arguments::new(JsValue::encode_object_value(global), &mut args)
            );

            let start = std::time::Instant::now();
            match function
                .as_function_mut()
                .call(&mut rt, &mut args, JsValue::new(*funcc))
            {
                Ok(_) => {
                    let elapsed = start.elapsed();
                    eprintln!("Executed in {}ms", elapsed.as_nanos() as f64 / 1000000f64);
                }
                Err(e) => {
                    let str = match e.to_string(&mut rt) {
                        Ok(s) => s,
                        Err(_) => "<unknown error>".to_owned(),
                    };
                    eprintln!("Uncaught exception: {}", str);
                    eprintln!("Stacktrace: \n{}", rt.take_stacktrace());
                }
            }
        }
        Err(error) => {
            eprintln!("Error while reading JS source: {}", error);
            std::process::exit(1);
        }
    }
    let after_exec = rt.heap().gc.stats();
    if options.dump_stats {
        eprintln!(
            "Memory used at start: {}",
            formatted_size(at_init.allocated)
        );
        eprintln!(
            "Memroy used at end: {}",
            formatted_size(after_exec.allocated)
        );
    }
    drop(rt);
    std::process::exit(0);
}
