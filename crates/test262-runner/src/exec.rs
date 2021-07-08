use super::{
    Harness, Outcome, Phase, SuiteResult, Test, TestFlags, TestOutcomeResult, TestResult,
    TestSuite, IGNORED,
};

use colored::Colorize;
use rayon::prelude::*;
use starlight::vm::{parse, Runtime};
use std::panic;
use std::panic::AssertUnwindSafe;

impl TestSuite {
    pub(crate) fn run_main(
        &self,
        harness: &Harness,
        verbose: u8,
        rt: &mut Box<Runtime>,
    ) -> SuiteResult {
        if verbose != 0 {
            println!("Suite {}:", self.name);
        }

        // TODO: in parallel
        let suites: Vec<_> = self
            .suites
            .iter()
            .map(|suite| suite.run(harness, verbose, rt))
            .collect();

        // TODO: in parallel
        let tests: Vec<_> = self
            .tests
            .iter()
            .map(|test| test.run(harness, verbose, rt))
            .flatten()
            .collect();

        if verbose != 0 {
            println!();
        }

        // Count passed tests
        let mut passed = 0;
        let mut ignored = 0;
        let mut panic = 0;
        for test in &tests {
            match test.result {
                TestOutcomeResult::Passed => passed += 1,
                TestOutcomeResult::Ignored => ignored += 1,
                TestOutcomeResult::Panic => panic += 1,
                TestOutcomeResult::Failed => {}
            }
        }

        // Count total tests
        let mut total = tests.len();
        for suite in &suites {
            total += suite.total;
            passed += suite.passed;
            ignored += suite.ignored;
            panic += suite.panic;
        }

        if verbose != 0 {
            println!(
            "Suite {} results: total: {}, passed: {}, ignored: {}, failed: {} (panics: {}{}), conformance: {:.2}%",
            self.name,
            total,
            passed.to_string().green(),
            ignored.to_string().yellow(),
            (total - passed - ignored).to_string().red(),
            if panic == 0 {"0".normal()} else {panic.to_string().red()},
            if panic != 0 {" ⚠"} else {""}.red(),
            (passed as f64 / total as f64) * 100.0
        );
        }

        SuiteResult {
            name: self.name.clone(),
            total,
            passed,
            ignored,
            panic,
            suites,
            tests,
        }
    }
    /// Runs the test suite.
    pub(crate) fn run(&self, harness: &Harness, verbose: u8, rt: &mut Box<Runtime>) -> SuiteResult {
        if verbose != 0 {
            println!("Suite {}:", self.name);
        }

        let suites: Vec<_> = self
            .suites
            .iter()
            .map(|suite| suite.run(harness, verbose, rt))
            .collect();

        let tests: Vec<_> = self
            .tests
            .iter()
            .map(|test| test.run(harness, verbose, rt))
            .flatten()
            .collect();

        if verbose != 0 {
            println!();
        }

        // Count passed tests
        let mut passed = 0;
        let mut ignored = 0;
        let mut panic = 0;
        for test in &tests {
            match test.result {
                TestOutcomeResult::Passed => passed += 1,
                TestOutcomeResult::Ignored => ignored += 1,
                TestOutcomeResult::Panic => panic += 1,
                TestOutcomeResult::Failed => {}
            }
        }

        // Count total tests
        let mut total = tests.len();
        for suite in &suites {
            total += suite.total;
            passed += suite.passed;
            ignored += suite.ignored;
            panic += suite.panic;
        }

        if verbose != 0 {
            println!(
            "Suite {} results: total: {}, passed: {}, ignored: {}, failed: {} (panics: {}{}), conformance: {:.2}%",
            self.name,
            total,
            passed.to_string().green(),
            ignored.to_string().yellow(),
            (total - passed - ignored).to_string().red(),
            if panic == 0 {"0".normal()} else {panic.to_string().red()},
            if panic != 0 {" ⚠"} else {""}.red(),
            (passed as f64 / total as f64) * 100.0
        );
        }

        SuiteResult {
            name: self.name.clone(),
            total,
            passed,
            ignored,
            panic,
            suites,
            tests,
        }
    }
}

impl Test {
    /// Runs the test.
    pub(crate) fn run(
        &self,
        harness: &Harness,
        verbose: u8,
        rt: &mut Box<Runtime>,
    ) -> Vec<TestResult> {
        let mut results = Vec::new();
        if self.flags.contains(TestFlags::STRICT) {
            results.push(self.run_once(harness, true, verbose, rt));
        }

        if self.flags.contains(TestFlags::NO_STRICT) || self.flags.contains(TestFlags::RAW) {
            results.push(self.run_once(harness, false, verbose, rt));
        }

        results
    }

    /// Runs the test once, in strict or non-strict mode
    fn run_once(
        &self,
        harness: &Harness,
        strict: bool,
        verbose: u8,
        rt: &mut Box<Runtime>,
    ) -> TestResult {
        if verbose >= 1 {
            eprintln!(
                "Starting `{}` {}",
                self.name,
                if strict { " (strict mode)" } else { "" }
            );
            eprintln!("Description: {}\nesid: {:?}\n", self.description, self.esid,);
        }

        let (result, result_text) = if !IGNORED.contains_any_flag(self.flags)
            && !IGNORED.contains_test(&self.name)
            && !IGNORED.contains_any_feature(&self.features)
            && (matches!(self.expected_outcome, Outcome::Positive)
                || matches!(
                    self.expected_outcome,
                    Outcome::Negative {
                        phase: Phase::Parse,
                        error_type: _,
                    }
                )
                || matches!(
                    self.expected_outcome,
                    Outcome::Negative {
                        phase: Phase::Early,
                        error_type: _,
                    }
                )
                || matches!(
                    self.expected_outcome,
                    Outcome::Negative {
                        phase: Phase::Runtime,
                        error_type: _,
                    }
                )) {
            let res = panic::catch_unwind(AssertUnwindSafe(|| match self.expected_outcome {
                Outcome::Positive => {
                    // TODO: implement async and add `harness/doneprintHandle.js` to the includes.

                    match self.set_up_env(&harness, strict, rt) {
                        Ok(_) => {
                            let content = if strict {
                                format!("\"use strict\";\n {}", self.content)
                            } else {
                                self.content.to_string()
                            };
                            let res = rt.eval_internal(None, false, &content, false);

                            let passed = res.is_ok();
                            let text = match res {
                                Ok(val) => val.to_string(rt).unwrap_or_else(|_| String::new()),
                                Err(e) => format!(
                                    "Uncaught {}",
                                    e.to_string(rt).unwrap_or_else(|_| String::new())
                                ),
                            };

                            (passed, text)
                        }
                        Err(e) => (false, e),
                    }
                }
                Outcome::Negative {
                    phase: Phase::Parse,
                    ref error_type,
                }
                | Outcome::Negative {
                    phase: Phase::Early,
                    ref error_type,
                } => {
                    assert_eq!(
                        error_type.as_ref(),
                        "SyntaxError",
                        "non-SyntaxError parsing/early error found in {}",
                        self.name
                    );

                    match parse(&self.content.as_ref(), strict) {
                        Ok(n) => (false, format!("{:?}", n)),
                        Err(e) => (true, format!("Uncaught {:?}", e)),
                    }
                }
                Outcome::Negative {
                    phase: Phase::Resolution,
                    error_type: _,
                } => todo!("check module resolution errors"),
                Outcome::Negative {
                    phase: Phase::Runtime,
                    ref error_type,
                } => {
                    if let Err(e) = parse(&self.content.as_ref(), strict) {
                        (false, format!("Uncaught {:?}", e))
                    } else {
                        match self.set_up_env(&harness, strict, rt) {
                            Ok(_) => {
                                match rt.eval_internal(None, false, &self.content.as_ref(), false) {
                                    Ok(res) => {
                                        (false, res.to_string(rt).unwrap_or_else(|_| String::new()))
                                    }
                                    Err(e) => {
                                        let passed = e
                                            .to_string(rt)
                                            .unwrap_or_else(|_| String::new())
                                            .contains(error_type.as_ref());

                                        (
                                            passed,
                                            format!(
                                                "Uncaught {}",
                                                e.to_string(rt).unwrap_or_else(|_| String::new())
                                            ),
                                        )
                                    }
                                }
                            }
                            Err(e) => (false, e),
                        }
                    }
                }
            }));

            let result = res
                .map(|(res, text)| {
                    if res {
                        (TestOutcomeResult::Passed, text)
                    } else {
                        (TestOutcomeResult::Failed, text)
                    }
                })
                .unwrap_or_else(|_| {
                    eprintln!("last panic was on test \"{}\"", self.name);
                    (TestOutcomeResult::Panic, String::new())
                });

            if verbose > 1 {
                println!(
                    "Result: {}",
                    if matches!(result, (TestOutcomeResult::Passed, _)) {
                        "Passed".green()
                    } else if matches!(result, (TestOutcomeResult::Failed, _)) {
                        "Failed".red()
                    } else {
                        "⚠ Panic ⚠".red()
                    }
                );
            } else {
                print!(
                    "{}",
                    if matches!(result, (TestOutcomeResult::Passed, _)) {
                        ".".green()
                    } else {
                        ".".red()
                    }
                );
            }

            result
        } else {
            if verbose >= 1 {
                println!("Result: {}", "Ignored".yellow());
            } else {
                print!("{}", ".".yellow());
            }
            (TestOutcomeResult::Ignored, String::new())
        };

        if verbose >= 1 {
            println!("Result text:");
            println!("{}", result_text);
            println!();
        }

        TestResult {
            name: self.name.clone(),
            strict,
            result,
            result_text: result_text.into_boxed_str(),
        }
    }

    /// Sets the environment up to run the test.
    fn set_up_env(
        &self,
        harness: &Harness,
        _strict: bool,
        context: &mut Box<Runtime>,
    ) -> Result<(), String> {
        // Create new Realm
        // TODO: in parallel.
        context.create_realm().unwrap_or_else(|_| unreachable!());

        /*let mut context = Runtime::new(
            RuntimeParams::default().with_dump_bytecode(false),
            GcParams::default()
                .with_parallel_marking(false)
                .with_conservative_marking(false),
            None,
        );*/
        context
            .eval_internal(None, false, &harness.assert.as_ref(), false)
            .map_err(|e| {
                format!(
                    "could not run assert.js:\n{}",
                    e.to_string(context).unwrap_or_else(|_| String::new())
                )
            })?;
        context
            .eval_internal(None, false, &harness.sta.as_ref(), false)
            .map_err(|e| {
                format!(
                    "could not run sta.js:\n{}",
                    e.to_string(context).unwrap_or_else(|_| String::new())
                )
            })?;

        for include in self.includes.iter() {
            context
                .eval_internal(
                    None,
                    false,
                    &harness
                        .includes
                        .get(include)
                        .ok_or_else(|| format!("could not find the {} include file.", include))?
                        .as_ref(),
                    false,
                )
                .map_err(|e| {
                    format!(
                        "could not run the {} include file:\nUncaught {}",
                        include,
                        e.to_string(context).unwrap_or_else(|_| String::new())
                    )
                })?;
        }

        Ok(())
    }
}
