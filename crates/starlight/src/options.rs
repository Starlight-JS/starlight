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
    #[structopt(long = "codegen-plugins", help = "Enable codegen plugins")]
    pub codegen_plugins: bool,
    #[structopt(long = "verboseGC", help = "Verbose GC cycle")]
    pub verbose_gc: bool,
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
            verbose_gc: false,
            codegen_plugins: false,
        }
    }
}

// for configure
impl Options {
    pub fn with_codegen_plugins(mut self, enable: bool) -> Self {
        self.codegen_plugins = enable;
        self
    }

    pub fn with_dump_size_classes(mut self, enable: bool) -> Self {
        self.dump_size_classes = enable;
        self
    }

    pub fn with_parallel_marking(mut self, enable: bool) -> Self {
        self.parallel_marking = enable;
        self
    }

    pub fn with_size_class_progression(mut self, size: f64) -> Self {
        self.size_class_progression = size;
        self
    }

    pub fn with_heap_size(mut self, size: usize) -> Self {
        self.heap_size = size;
        self
    }

    pub fn with_gc_threads(mut self, threads: u32) -> Self {
        self.gc_threads = threads;
        self
    }

    pub fn with_dump_bytecode(mut self, enable: bool) -> Self {
        self.dump_bytecode = enable;
        self
    }

    pub fn with_disable_ic(mut self, disable: bool) -> Self {
        self.disable_ic = disable;
        self
    }

    pub fn with_enable_ffi(mut self, enable: bool) -> Self {
        self.enable_ffi = enable;
        self
    }

    pub fn with_dump_stats(mut self, enable: bool) -> Self {
        self.dump_stats = enable;
        self
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
        .map_err(|x| x)
        .map(|x| x * multiplier)
}
