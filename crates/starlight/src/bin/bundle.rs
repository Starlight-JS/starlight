use std::path::PathBuf;

use starlight::{
    heap::snapshot::Snapshot,
    vm::{GcParams, Runtime, RuntimeParams},
    Platform,
};
use structopt::*;

#[derive(Debug, StructOpt)]
pub struct Options {
    #[structopt(parse(from_os_str), help = "JS file to compile")]
    input: PathBuf,
    #[structopt(parse(from_os_str), help = "Output file for bundle")]
    output: PathBuf,
}

fn main() {
    let opts = Options::from_args();
    Platform::initialize();
    let string = std::fs::read_to_string(&opts.input).unwrap_or_else(|error| {
        eprintln!("Failed to read JS file: {}", error);
        std::process::exit(1);
    });

    let mut rt = Runtime::new(RuntimeParams::default(), GcParams::default(), None);

    let func = rt
        .compile(opts.input.as_os_str().to_str().unwrap(), &string)
        .unwrap_or_else(|error| match error.to_string(&mut rt) {
            Ok(s) => {
                eprintln!("Failed to compile JS file: {}", s);
                std::process::exit(1);
            }
            Err(_) => {
                eprintln!("Failed to convert error to string");
                std::process::exit(1);
            }
        });
    let snapshot = Snapshot::take(false, &mut rt, |ser, rt| {
        ser.write_gcpointer(func.get_object())
    });
    let mut c_src = String::with_capacity(snapshot.buffer.len() + 128);
    c_src.push_str(
        r#"
#include <stddef.h>
#include <stdint.h>

static const uint8_t snapshot[] = {

"#,
    );
    for (ix, byte) in snapshot.buffer.iter().enumerate() {
        c_src.push_str(&byte.to_string());
        if ix != snapshot.buffer.len() - 1 {
            c_src.push(',');
        }
    }
    c_src.push_str(&format!(
        r#"
    }};
    #define SNAPSHOT_SIZE {}

    void __execute_bundle(uint8_t*,size_t);
    void platform_initialize();
    int main() {{
        platform_initialize();
        __execute_bundle(&snapshot,SNAPSHOT_SIZE);
    }}
    "#,
        snapshot.buffer.len()
    ));

    std::fs::write(opts.output, c_src).unwrap();
}
