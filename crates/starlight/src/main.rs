use std::path::PathBuf;

use starlight::{
    vm::{arguments::Arguments, value::JsValue, GcParams, Runtime, RuntimeParams},
    Platform,
};
use structopt::*;

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
}

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
    let mut rt = Runtime::new(
        RuntimeParams::default()
            .with_dump_bytecode(options.dump_bytecode)
            .with_inline_caching(!options.disable_ic),
        gc,
        None,
    );

    let string = std::fs::read_to_string(&options.file);
    match string {
        Ok(source) => {
            let mut function = match rt.compile(options.file.as_os_str().to_str().unwrap(), &source)
            {
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
            };
            let global = rt.global_object();
            let mut args = Arguments::new(&mut rt, JsValue::encode_object_value(global), 0);
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
}
