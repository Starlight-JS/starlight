use std::{num::ParseIntError, path::PathBuf};

use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct Options {
    #[structopt(
        long = "sizeClassProgression",
        default_value = "1.4",
        help = "Set size class progression for heap"
    )]
    pub size_class_progression: f64,
    #[structopt(long = "dumpSizeClasses", help = "Dump heap size classes at startup")]
    pub dump_size_classes: bool,
    #[structopt(
        long="heapSize",
        help="Set heap size (default 2GB)",
        default_value="2GB",
        parse(try_from_str=parse_size_from_str))]
    pub heap_size: usize,
    #[structopt(
        long = "gc-threads",
        default_value = "4",
        help = "Set number of GC marker threads"
    )]
    pub gc_threads: u32,
    #[structopt(long = "parallelMarking", help = "Enable parallel marking GC")]
    pub parallel_marking: bool,
    #[structopt(parse(from_os_str), help = "Input JS file")]
    pub file: PathBuf,
    #[structopt(short = "d", long = "dumpBytecode", help = "Dump bytecode")]
    pub dump_bytecode: bool,
    #[structopt(long = "disableIC", help = "Disable inline caching")]
    pub disable_ic: bool,

    #[structopt(long = "enable-ffi", help = "Enable FFI and CFunction objects for use")]
    pub enable_ffi: bool,
    #[structopt(
        long = "dumpStats",
        help = "Dump various statistics at the end of execution"
    )]
    pub dump_stats: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            parallel_marking: false,
            dump_bytecode: false,
            disable_ic: false,
            dump_size_classes: false,
            dump_stats: false,
            enable_ffi: false,
            size_class_progression: 1.4,
            heap_size: 2 * 1024 * 1024 * 1024,
            file: PathBuf::new(),
            gc_threads: 4,
        }
    }
}

fn parse_size_from_str(s: &str) -> Result<usize, ParseIntError> {
    let s = s.to_lowercase();
    let (number, unit) = s.split_at(s.find(|c: char| !c.is_digit(10)).unwrap_or(s.len()));
    let multiplier = match unit {
        "kb" | "k" => 1024,
        "mb" | "m" => 1024 * 1024,
        "gb" | "g" => 1024 * 1024 * 1024,
        _ => 1,
    };

    number
        .parse::<usize>()
        .map_err(|x| x.into())
        .map(|x| x * multiplier)
}
