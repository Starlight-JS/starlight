use starlight::{
    gc::{migc::MiGC, Heap},
    prelude::{Deserializer, Internable, Snapshot},
    vm::{GcParams, Runtime, RuntimeParams},
};
use std::{io::*, panic::AssertUnwindSafe};
use test262_harness::*;

fn main() {
    starlight::Platform::initialize();
    let harness = Harness::new("test262").unwrap();
    let mut rt = Runtime::new(RuntimeParams::default(), GcParams::default(), None);
    let snapshot = Snapshot::take(false, &mut rt, |_, _| {});
    let mut failed = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open("test262-failed")
        .unwrap();
    let mut succeed = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open("test262-succeed")
        .unwrap();
    let mut skipped = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open("test262-skipped")
        .unwrap();
    let mut nfail = 0;
    let mut nsucc = 0;
    let mut nskip = 0;
    let mut write_fail = |msg: &str| {
        failed.write(msg.as_bytes()).unwrap();
        failed.write(b"\n").unwrap();
        nfail += 1;
    };

    let mut write_succ = |msg: &str| {
        nsucc += 1;
        succeed.write(msg.as_bytes()).unwrap();
        succeed.write(b"\n").unwrap();
    };
    let mut write_skip = |msg: &str| {
        skipped.write(msg.as_bytes()).unwrap();
        skipped.write(b"\n").unwrap();
        nskip += 1;
    };

    for test in harness {
        if let Ok(test) = test {
            if test.desc.flags.contains(&Flag::Async) || test.desc.flags.contains(&Flag::Module) {
                write_skip(&format!("Skip '{}'\n", test.path.display()));
                continue;
            }
            println!("Running '{}'", test.path.display());
            let source =
                std::fs::read_to_string(&test.path).expect(&format!("{}", test.path.display()));
            let mut format = String::new();
            let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let mut rt = Deserializer::deserialize(
                    false,
                    &snapshot.buffer,
                    RuntimeParams::default(),
                    Heap::new(MiGC::new(GcParams::default().with_parallel_marking(false))),
                    None,
                    |_, _| {},
                );

                for inc in test.desc.includes.iter() {
                    format.push_str(inc);
                }
                format.push_str(&source);

                match rt.eval(
                    test.path.to_str(),
                    test.desc.flags.contains(&Flag::OnlyStrict),
                    &format,
                ) {
                    Ok(_) => {
                        if test.desc.negative.is_some() {
                            write_fail(&format!("Test '{}' should fail\n", test.path.display()));
                            return;
                        } else {
                            write_succ(&format!("Test '{}' passed\n", test.path.display()));
                        }
                    }
                    Err(e) => {
                        if let Some(ref neg) = test.desc.negative {
                            if let Some(ref kind) = neg.kind {
                                if e.is_jsobject() {
                                    let mut val = e.get_jsobject();
                                    let str = match val.get(&mut rt, "name".intern()) {
                                        Ok(x) => x.to_string(&mut rt).unwrap_or_else(|_| panic!()),
                                        Err(_) => {
                                            panic!()
                                        }
                                    };
                                    if kind == &str {
                                        write_succ(&format!(
                                            "Test '{}' passed\n",
                                            test.path.display()
                                        ));
                                        return;
                                    }
                                }
                            }
                            write_succ(&format!("Test '{}' passed\n", test.path.display()));
                            return;
                        } else {
                            write_fail(&format!("Test '{}' failed\n", test.path.display()));
                            return;
                        }
                    }
                }
            }));

            match res {
                Ok(_) => {}
                Err(_) => {
                    write_fail(&format!("Test '{}' panicked\n", test.path.display()));
                }
            }
        } else {
        }
    }

    println!("Passed: {}", nsucc);
    println!("Skipped: {}", nskip);
    println!("Failed: {}", nfail);
}
