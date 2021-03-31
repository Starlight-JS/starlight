use std::path::{Path, PathBuf};
use starlight::prelude::*;
use structopt::*;
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

    let gcstack = rt.shadowstack();

    let string = std::fs::read_to_string(&options.file);
    match string {
        Ok(source) => {
            root!(
                function = gcstack,
                match rt.compile(
                    options.file.as_os_str().to_str().unwrap(),
                    "<script>",
                    &source
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
            let global = rt.global_object();

            root!(
                args = gcstack,
                Arguments::new(JsValue::encode_object_value(global), &mut [])
            );
            let start = std::time::Instant::now();
            match function.as_function_mut().call(&mut rt, &mut args) {
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
                }
            }
        }
        Err(error) => {
            eprintln!("Error while reading JS source: {}", error);
            std::process::exit(1);
        }
    }

    drop(rt);
}
