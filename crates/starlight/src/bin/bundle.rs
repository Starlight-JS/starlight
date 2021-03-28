use starlight::{
    gc::snapshot::Snapshot,
    vm::{GcParams, Runtime, RuntimeParams},
    Platform,
};
use std::path::PathBuf;
use structopt::*;

#[derive(Debug, StructOpt)]
pub struct Options {
    #[structopt(parse(from_os_str), help = "JS file to compile")]
    input: PathBuf,
    #[structopt(parse(from_os_str), help = "Output file for bundle")]
    output: PathBuf,
    #[structopt(long = "use-musl", help = "Use musl-clang for linking")]
    use_musl: bool,
    #[structopt(long = "output-c", help = "Output bundle as raw C file")]
    output_c: bool,
}

fn main() {
    let opts = Options::from_args();
    Platform::initialize();
    let string = std::fs::read_to_string(&opts.input).unwrap_or_else(|error| {
        eprintln!("Failed to read JS file: {}", error);
        std::process::exit(1);
    });

    let mut rt = Runtime::new(RuntimeParams::default(), GcParams::default(), None);
    rt.gc().defer();
    let func = rt
        .compile(
            opts.input.as_os_str().to_str().unwrap(),
            "<script>",
            &string,
        )
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
    let snapshot = Snapshot::take(false, &mut rt, |ser, _rt| {
        ser.write_gcpointer(func.get_object())
    });
    rt.gc().undefer();
    let mut c_src = String::with_capacity(snapshot.buffer.len() + 128);
    c_src.push_str(&format!(
        r#"
#include <stddef.h>
#include <stdint.h>

static const uint8_t snapshot[{}] = {{

"#,
        snapshot.buffer.len()
    ));
    for (ix, byte) in snapshot.buffer.iter().enumerate() {
        c_src.push_str(&byte.to_string());
        if ix != snapshot.buffer.len() - 1 {
            c_src.push(',');
        }
    }
    c_src.push_str(&format!(
        r#"
    }};
 

    void __execute_bundle(const uint8_t*,size_t);
    void platform_initialize();
    int main() {{
        platform_initialize();
        __execute_bundle(snapshot,{});
    }}
    "#,
        snapshot.buffer.len()
    ));
    if opts.output_c {
        std::fs::write(format!("{}.c", opts.output.display()), c_src).unwrap();
    } else {
        std::fs::write(format!("{}.temp.c", opts.input.display()), c_src).unwrap();
        let cmd = if opts.use_musl { "musl-gcc" } else { "cc" };
        assert!(std::process::Command::new(cmd)
            .arg(format!("{}.temp.c", opts.input.display()))
            .arg("-static")
            .arg("-rdynamic")
            .arg("-lstarlight")
            .arg("-lpthread")
            .arg("-ldl")
            .arg("-lm")
            .arg("-std=c99")
            .arg("-pedantic")
            .arg("-Wall")
            .arg("-Wextra")
            .arg("-L/usr/lib")
            .arg("-L/usr/local/lib")
            .arg(format!("-o{}", opts.output.display()))
            .spawn()
            .unwrap()
            .wait()
            .unwrap()
            .success());

        std::fs::remove_file(format!("{}.temp.c", opts.input.display())).unwrap();
    }
}
