/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use crate::bytecode::opcodes::Opcode;
use std::time::{Duration, Instant};
#[derive(Debug, Clone)]
pub struct PerfCounter {
    pub count: u64,
    pub duration: Duration,
}

impl PerfCounter {
    pub fn new() -> Self {
        PerfCounter {
            count: 0,
            duration: Duration::from_secs(0),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Perf {
    counter: Vec<PerfCounter>,
    timer: Instant,
    prev_time: Duration,
    prev_inst: u8,
}

impl Perf {
    pub const GC: u8 = 252;
    pub const CODEGEN: u8 = 253;
    pub const EXTERN: u8 = 254;
    pub const INVALID: u8 = 255;
}

impl Perf {
    pub fn new() -> Self {
        Perf {
            counter: vec![PerfCounter::new(); 1 << 15],
            timer: Instant::now(),
            prev_time: Duration::from_secs(0),
            prev_inst: Perf::INVALID,
        }
    }

    /// Record duration for current instruction.
    pub fn get_perf(&mut self, next_inst: u8) {
        let prev = self.prev_inst;
        //assert!(next_inst != 0);
        //assert!(prev != 0);
        let elapsed = self.timer.elapsed();
        if prev != Perf::INVALID {
            self.counter[prev as usize].count += 1;
            self.counter[prev as usize].duration += elapsed - self.prev_time;
        }
        self.prev_time = elapsed;
        self.prev_inst = next_inst;
    }

    pub fn get_perf_no_count(&mut self, next_inst: u8) {
        self.get_perf(next_inst);
        if next_inst != Perf::INVALID {
            self.counter[next_inst as usize].count -= 1;
        }
    }

    pub fn set_prev_inst(&mut self, inst: u8) {
        self.prev_inst = inst;
    }

    pub fn get_prev_inst(&mut self) -> u8 {
        self.prev_inst
    }

    pub fn print_perf(&self) {
        eprintln!("+-------------------------------------------+");
        eprintln!("| Performance stats for opcodes:               |");
        eprintln!(
            "| {:<20} {:>5} {:>8} {:>8} |",
            "Opcode", "count", "%time", "ns/inst"
        );
        eprintln!("+-------------------------------------------+");
        let sum = self
            .counter
            .iter()
            .fold(Duration::from_secs(0), |acc, x| acc + x.duration);
        for (
            i,
            PerfCounter {
                count: c,
                duration: d,
            },
        ) in self.counter.iter().enumerate()
        {
            if *c == 0 || i == 0 {
                continue;
            }
            eprintln!(
                "  {:<14}{:>10} {:>8.2} {:>8}",
                if i as u8 == Perf::CODEGEN {
                    "CODEGEN".to_string()
                } else if i as u8 == Perf::EXTERN {
                    "EXTERN".to_string()
                } else if i as u8 == Perf::GC {
                    "GC".to_string()
                } else {
                    format!("{:?}", unsafe { std::mem::transmute::<_, Opcode>(i as u8) })
                },
                if *c > 10000_000 {
                    format!("{:>12}M", c / 1000_000)
                } else if *c > 10000 {
                    format!("{:>10}K", c / 1000)
                } else {
                    format!("{:>9}", *c)
                },
                (d.as_micros() as f64) * 100.0 / (sum.as_micros() as f64),
                d.as_nanos() / (*c as u128)
            );
        }
    }
}
