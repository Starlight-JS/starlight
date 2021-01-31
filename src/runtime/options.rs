use std::num::ParseIntError;

fn parse_size_from_osstr(s: &str) -> Result<usize, ParseIntError> {
    let s = s.to_lowercase();
    let (number, unit) = s.split_at(s.find(|c: char| !c.is_digit(10)).unwrap_or(s.len()));
    let multiplier = match unit {
        "kb" => 1024,
        "mb" => 1024 * 1024,
        "gb" => 1024 * 1024 * 1024,
        _ => 1,
    };

    number
        .parse::<usize>()
        .map_err(|x| x.into())
        .map(|x| x * multiplier)
}
use structopt::StructOpt;

#[derive(Debug, StructOpt, Clone)]
#[structopt(name = "js", about = "JS engine in Rust programming language")]
pub struct Options {
    #[structopt(
        long = "heap-size",
        help = "Set maximum heap size for GC",
        default_value = "512KB",
        parse(try_from_str=parse_size_from_osstr)
    )]
    pub heap_size: usize,
    #[structopt(
        long = "gc-threshold",
        help = "Set threshold for GC",
        default_value="100KB",
        parse(try_from_str=parse_size_from_osstr)
    )]
    pub threshold: usize,
    #[structopt(long = "gc-verbose", help = "Enable verbose GC logging")]
    pub verbose_gc: bool,
}
