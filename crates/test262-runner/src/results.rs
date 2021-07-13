use crate::{TestOutcomeResult, TestResult};

use super::SuiteResult;
#[cfg(target_pointer_width = "64")]
use git2::Repository;
use hex::ToHex;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    env, fs,
    io::{self, BufReader, BufWriter},
    path::Path,
};

/// Structure to store full result information.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct ResultInfo {
    #[serde(rename = "c")]
    commit: Box<str>,
    #[serde(rename = "u")]
    test262_commit: Box<str>,
    #[serde(rename = "r")]
    results: SuiteResult,
}

/// Structure to store full result information.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct ReducedResultInfo {
    #[serde(rename = "c")]
    commit: Box<str>,
    #[serde(rename = "u")]
    test262_commit: Box<str>,
    #[serde(rename = "t")]
    total: usize,
    #[serde(rename = "o")]
    passed: usize,
    #[serde(rename = "i")]
    ignored: usize,
    #[serde(rename = "p")]
    panic: usize,
}

impl From<ResultInfo> for ReducedResultInfo {
    /// Creates a new reduced suite result from a full suite result.
    fn from(info: ResultInfo) -> Self {
        Self {
            commit: info.commit,
            test262_commit: info.test262_commit,
            total: info.results.total,
            passed: info.results.passed,
            ignored: info.results.ignored,
            panic: info.results.panic,
        }
    }
}

/// File name of the "latest results" JSON file.
const LATEST_FILE_NAME: &str = "latest.json";

/// File name of the "all results" JSON file.
const RESULTS_FILE_NAME: &str = "results.json";

/// Writes the results of running the test suite to the given JSON output file.
///
/// It will append the results to the ones already present, in an array.
pub(crate) fn write_json(
    results: SuiteResult,
    output: Option<&Path>,
    verbose: u8,
) -> io::Result<()> {
    if let Some(path) = output {
        fs::create_dir_all(&path)?;

        if verbose != 0 {
            println!("Writing the results to {}...", path.display());
        }

        // Write the latest results.

        let latest_path = path.join(LATEST_FILE_NAME);

        let new_results = ResultInfo {
            commit: env::var("GITHUB_SHA").unwrap_or_default().into_boxed_str(),
            test262_commit: get_test262_commit(),
            results,
        };

        let latest_output = BufWriter::new(fs::File::create(latest_path)?);
        serde_json::to_writer(latest_output, &new_results)?;

        // Write the full list of results, retrieving the existing ones first.

        let all_path = path.join(RESULTS_FILE_NAME);

        let mut all_results: Vec<ReducedResultInfo> = if all_path.exists() {
            serde_json::from_reader(BufReader::new(fs::File::open(&all_path)?))?
        } else {
            Vec::new()
        };

        all_results.push(new_results.into());

        let output = BufWriter::new(fs::File::create(&all_path)?);
        serde_json::to_writer(output, &all_results)?;

        if verbose != 0 {
            println!("Results written correctly");
        }
    }

    Ok(())
}

/// Gets the commit OID of the test262 submodule.
fn get_test262_commit() -> Box<str> {
    #[cfg(target_pointer_width = "64")]
    {
        let repo =
            Repository::open(".").expect("could not open git repository in current directory");

        let submodule = repo
            .submodules()
            .expect("could not get the list of submodules of the repo")
            .into_iter()
            .find(|sub| sub.path() == Path::new("test262"))
            .expect("test262 submodule not found");

        submodule
            .index_id()
            .expect("could not get the commit OID")
            .encode_hex::<String>()
            .into_boxed_str()
    }
    #[cfg(target_pointer_width = "32")]
    {
        "".to_string().into_boxed_str()
    }
}

pub(crate) fn get_all_tests_from_suite(suite: SuiteResult) -> Vec<TestResult> {
    let mut tests = suite.tests;
    let suites = suite.suites;
    for sub_suite in suites.into_iter() {
        let mut sub_tests = get_all_tests_from_suite(sub_suite);
        tests.append(&mut sub_tests);
    }
    return tests;
}

pub(crate) fn get_key_of_test(test: &TestResult) -> String {
    let key = (if test.strict {
        "[strict] "
    } else {
        "[non-strict] "
    })
    .to_string()
        + &test.name.to_string()[2..];
    return key;
}

/// Compares the results of two test suite runs.
pub(crate) fn compare_results(base: &Path, new: &Path, markdown: bool, detail: bool) {
    let base_results: ResultInfo = serde_json::from_reader(BufReader::new(
        fs::File::open(base).expect("could not open the base results file"),
    ))
    .expect("could not read the base results");

    let new_results: ResultInfo = serde_json::from_reader(BufReader::new(
        fs::File::open(new).expect("could not open the new results file"),
    ))
    .expect("could not read the new results");

    let base_total = base_results.results.total as isize;
    let new_total = new_results.results.total as isize;
    let total_diff = new_total - base_total;

    let base_passed = base_results.results.passed as isize;
    let new_passed = new_results.results.passed as isize;
    let passed_diff = new_passed - base_passed;

    let base_ignored = base_results.results.ignored as isize;
    let new_ignored = new_results.results.ignored as isize;
    let ignored_diff = new_ignored - base_ignored;

    let base_panics = base_results.results.panic as isize;
    let new_panics = new_results.results.panic as isize;
    let panic_diff = new_panics - base_panics;

    let base_failed = base_total - base_passed - base_ignored - base_panics;
    let new_failed = new_total - new_passed - new_ignored - new_panics;
    let failed_diff = new_failed - base_failed;

    let base_conformance = (base_passed as f64 / base_total as f64) * 100_f64;
    let new_conformance = (new_passed as f64 / new_total as f64) * 100_f64;
    let conformance_diff = new_conformance - base_conformance;

    if markdown {
        use num_format::{Locale, ToFormattedString};

        /// Generates a proper diff format, with some bold text if things change.
        fn diff_format(diff: isize) -> String {
            format!(
                "{}{}{}{}",
                if diff != 0 { "**" } else { "" },
                if diff > 0 { "+" } else { "" },
                diff.to_formatted_string(&Locale::en),
                if diff != 0 { "**" } else { "" }
            )
        }

        println!("### Test262 conformance changes:");
        println!("| Test result | Dev count | PR count | Difference |");
        println!("| :---------: | :----------: | :------: | :--------: |");
        println!(
            "| Total | {} | {} | {} |",
            base_total.to_formatted_string(&Locale::en),
            new_total.to_formatted_string(&Locale::en),
            diff_format(total_diff),
        );
        println!(
            "| Passed | {} | {} | {} |",
            base_passed.to_formatted_string(&Locale::en),
            new_passed.to_formatted_string(&Locale::en),
            diff_format(passed_diff),
        );
        println!(
            "| Ignored | {} | {} | {} |",
            base_ignored.to_formatted_string(&Locale::en),
            new_ignored.to_formatted_string(&Locale::en),
            diff_format(ignored_diff),
        );
        println!(
            "| Failed | {} | {} | {} |",
            base_failed.to_formatted_string(&Locale::en),
            new_failed.to_formatted_string(&Locale::en),
            diff_format(failed_diff),
        );
        println!(
            "| Panics | {} | {} | {} |",
            base_panics.to_formatted_string(&Locale::en),
            new_panics.to_formatted_string(&Locale::en),
            diff_format(panic_diff),
        );
        println!(
            "| Conformance | {:.2} | {:.2} | {} |",
            base_conformance,
            new_conformance,
            format!(
                "{}{}{:.2}%{}",
                if conformance_diff.abs() > f64::EPSILON {
                    "**"
                } else {
                    ""
                },
                if conformance_diff > 0_f64 { "+" } else { "" },
                conformance_diff,
                if conformance_diff.abs() > f64::EPSILON {
                    "**"
                } else {
                    ""
                },
            ),
        );
    } else {
        println!("Test262 conformance changes:");
        println!("| Test result | Dev count | PR count | Difference |");
        println!(
            "|    Passed   | {:^6} | {:^5} | {:^10} |",
            base_passed,
            new_passed,
            base_passed - new_passed
        );
        println!(
            "|   Ignored   | {:^6} | {:^5} | {:^10} |",
            base_ignored,
            new_ignored,
            base_ignored - new_ignored
        );
        println!(
            "|   Failed    | {:^6} | {:^5} | {:^10} |",
            base_failed,
            new_failed,
            base_failed - new_failed,
        );
        println!(
            "|   Panics    | {:^6} | {:^5} | {:^10} |",
            base_panics,
            new_panics,
            base_panics - new_panics
        );
    }
    if !detail {
        return;
    }
    let base_tests = get_all_tests_from_suite(base_results.results);
    let new_tests = get_all_tests_from_suite(new_results.results);

    let base_tests: Vec<TestResult> = base_tests
        .into_iter()
        .filter(|t| !matches!(t.result, TestOutcomeResult::Ignored))
        .collect();
    let new_tests: Vec<TestResult> = new_tests
        .into_iter()
        .filter(|t| !matches!(t.result, TestOutcomeResult::Ignored))
        .collect();

    let mut base_tests_map = HashMap::new();
    let mut new_test_map = HashMap::new();

    for test in base_tests {
        let key = get_key_of_test(&test);
        base_tests_map.insert(key, test);
    }
    for test in new_tests {
        let key = get_key_of_test(&test);
        new_test_map.insert(key, test);
    }

    let mut failed_tests: Vec<String> = Vec::new();
    for test in base_tests_map.values() {
        if !matches!(test.result, TestOutcomeResult::Passed) {
            if let Some(new_test) = new_test_map.get(&get_key_of_test(test)) {
                if matches!(new_test.result, crate::TestOutcomeResult::Passed) {
                    failed_tests.push(get_key_of_test(test));
                }
            }
        }
    }

    show_detail_faled_tests("Base Failed But New Passed", failed_tests);
    println!();

    let mut failed_tests: Vec<String> = Vec::new();
    for test in new_test_map.values() {
        if !matches!(test.result, TestOutcomeResult::Passed) {
            if let Some(base_test) = base_tests_map.get(&get_key_of_test(test)) {
                if matches!(base_test.result, crate::TestOutcomeResult::Passed) {
                    failed_tests.push(get_key_of_test(test));
                }
            }
        }
    }
    show_detail_faled_tests("New Failed But Base Passed", failed_tests);
}

pub fn show_detail_faled_tests(title: &str, failed_tests: Vec<String>) {
    if failed_tests.len() != 0 {
        println!(
            "<details><summary>{}</summary>\n\n```\n{}\n```\n</details>",
            title,
            failed_tests
                .iter()
                .enumerate()
                .map(|(i, s)| (i + 1).to_string() + ". " + &s)
                .collect::<Vec<String>>()
                .join("\n")
        );
    } else {
        println!("<details><summary>{}</summary></details>", title);
    }
}
