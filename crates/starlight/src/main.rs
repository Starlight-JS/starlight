use starlight::{
    gc::handle::Handle,
    jsrt::jsrt_init,
    vm::{Options, VirtualMachineRef},
};
use starlight::{
    runtime::{arguments::Arguments, value::JsValue},
    vm::VirtualMachine,
};
const HELP: &str = "\
App
USAGE:
  starlight [OPTIONS] [INPUT]
FLAGS:
  -h, --help            Prints help information
OPTIONS:
    -d,--dump-bytecode  Dump bytecode to stderr
ARGS:
  <INPUT>
";

#[derive(Debug)]
struct AppArgs {
    dump_bytecode: bool,
    input: std::path::PathBuf,
}
fn main() {
    let args = match parse_args() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error: {}.", e);
            std::process::exit(1);
        }
    };
    let opts = Options {
        dump_bytecode: args.dump_bytecode,
    };
    let file = std::fs::read(args.input);
    let contents = match file {
        Ok(v) => String::from_utf8(v).unwrap(),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };
    let mut vm = VirtualMachine::new(opts);
    jsrt_init(&mut vm);
    let func = vm
        .compile(false, &contents, "<script>")
        .map(|x| x.root(&mut vm));
    match func {
        Ok(mut func) => {
            let args = Arguments::new(&mut vm, JsValue::undefined(), 0);
            let mut args = Handle::new(vm.space(), args);
            args.this = JsValue::new(vm.global_object());
            match func.as_function_mut().call(&mut vm, &mut args) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!(
                        "Unhandled: {}",
                        e.to_string(&mut vm)
                            .unwrap_or_else(|_| "cannot get error".to_string())
                    );

                    eprintln!(
                        "Stacktrace: \n{}",
                        vm.take_stacktrace()
                            .unwrap_or_else(|| "no stacktrace".to_string())
                    );
                }
            }
        }
        Err(e) => {
            eprintln!(
                "Unhandled: {}",
                e.to_string(&mut vm)
                    .unwrap_or_else(|_| "cannot get error".to_string())
            );
        }
    }
    VirtualMachineRef::dispose(vm);
}

fn parse_args() -> Result<AppArgs, pico_args::Error> {
    let mut pargs = pico_args::Arguments::from_env();

    if pargs.contains(["-h", "--help"]) {
        print!("{}", HELP);
        std::process::exit(0);
    }
    let args = AppArgs {
        dump_bytecode: pargs.contains(["-d", "--dump-bytecode"]),
        input: pargs.free_from_str()?,
    }; // It's up to the caller what to do with the remaining arguments.
    let remaining = pargs.finish();
    if !remaining.is_empty() {
        eprintln!("Warning: unused arguments left: {:?}.", remaining);
    }

    Ok(args)
}
