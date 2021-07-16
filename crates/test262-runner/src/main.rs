#![allow(
    clippy::redundant_allocation,
    clippy::needless_borrow,
    clippy::field_reassign_with_default
)]
pub mod exec;
pub mod js262;
pub mod read;
pub mod results;
use self::read::{read_harness, read_suite, read_test, MetaData, Negative, TestFlag};
use bitflags::bitflags;
use colored::Colorize;
use fxhash::{FxHashMap, FxHashSet};
use once_cell::sync::Lazy;
use results::{compare_results, write_json};
use serde::{Deserialize, Serialize};
use starlight::prelude::Options;
use starlight::{vm::Runtime, Platform};

use std::time::Instant;
use std::{
    fs,
    path::{Path, PathBuf},
};
use structopt::StructOpt;

/// Structure to allow defining ignored tests, features and files that should
/// be ignored even when reading.
#[derive(Debug)]
struct Ignored {
    tests: FxHashSet<Box<str>>,
    features: FxHashSet<Box<str>>,
    files: FxHashSet<Box<str>>,
    flags: TestFlags,
}

impl Ignored {
    /// Checks if the ignore list contains the given test name in the list of
    /// tests to ignore.
    pub(crate) fn contains_test(&self, test: &str) -> bool {
        for ign in self.tests.iter() {
            if test.contains(&**ign) {
                return true;
            }
        }
        false
    }

    /// Checks if the ignore list contains the given feature name in the list
    /// of features to ignore.
    pub(crate) fn contains_any_feature(&self, features: &[Box<str>]) -> bool {
        features
            .iter()
            .any(|feature| self.features.contains(feature))
    }

    /// Checks if the ignore list contains the given file name in the list to
    /// ignore from reading.
    pub(crate) fn contains_file(&self, file: &str) -> bool {
        self.files.contains(file)
    }

    pub(crate) fn contains_any_flag(&self, flags: TestFlags) -> bool {
        flags.intersects(self.flags)
    }
}

impl Default for Ignored {
    fn default() -> Self {
        Self {
            tests: Default::default(),
            features: Default::default(),
            files: Default::default(),
            flags: TestFlags::empty(),
        }
    }
}

/// List of ignored tests.
static IGNORED: Lazy<Ignored> = Lazy::new(|| {
    let path = Path::new("test_ignore.txt");
    if path.exists() {
        let filtered = fs::read_to_string(path).expect("could not read test filters");
        filtered
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with("//"))
            .fold(Ignored::default(), |mut ign, line| {
                // let mut line = line.to_owned();
                if line.starts_with("file:") {
                    let file = line
                        .strip_prefix("file:")
                        .expect("prefix disappeared")
                        .trim()
                        .to_owned()
                        .into_boxed_str();
                    let test = if file.ends_with(".js") {
                        file.strip_suffix(".js")
                            .expect("suffix disappeared")
                            .to_owned()
                            .into_boxed_str()
                    } else {
                        file.clone()
                    };
                    ign.files.insert(file);
                    ign.tests.insert(test);
                } else if line.starts_with("feature:") {
                    ign.features.insert(
                        line.strip_prefix("feature:")
                            .expect("prefix disappeared")
                            .trim()
                            .to_owned()
                            .into_boxed_str(),
                    );
                } else if line.starts_with("flag:") {
                    let flag = line
                        .strip_prefix("flag:")
                        .expect("prefix disappeared")
                        .trim()
                        .parse::<TestFlag>()
                        .expect("invalid flag found");
                    ign.flags.insert(flag.into())
                } else {
                    let mut test = line.trim();
                    if test.ends_with(".js") {
                        test = test.strip_suffix(".js").expect("suffix disappeared");
                    }
                    ign.tests.insert(test.to_owned().into_boxed_str());
                }
                ign
            })
    } else {
        Default::default()
    }
});

/// Starlight test262 tester
#[derive(StructOpt, Debug)]
#[structopt(name = "Starlight test262 tester")]
enum Cli {
    /// Run the test suite.
    Run {
        /// Whether to show verbose output.
        #[structopt(short, long, parse(from_occurrences))]
        verbose: u8,

        /// Path to the Test262 suite.
        #[structopt(long, parse(from_os_str), default_value = "./test262")]
        test262_path: PathBuf,

        /// Which specific test or test suite to run.
        #[structopt(short, long, parse(from_os_str), default_value = "test")]
        suite: PathBuf,

        /// Optional output folder for the full results information.
        #[structopt(short, long, parse(from_os_str))]
        output: Option<PathBuf>,
    },
    Compare {
        /// Base results of the suite.
        #[structopt(parse(from_os_str))]
        base: PathBuf,

        /// New results to compare.
        #[structopt(parse(from_os_str))]
        new: PathBuf,

        /// Whether to use markdown output
        #[structopt(short, long)]
        markdown: bool,

        #[structopt(short, long)]
        detail: bool,
    },
}

/// All the harness include files.
#[derive(Debug, Clone)]
struct Harness {
    assert: Box<str>,
    sta: Box<str>,
    includes: FxHashMap<Box<str>, Box<str>>,
}

/// Represents a test suite.
#[derive(Debug, Clone)]
struct TestSuite {
    name: Box<str>,
    suites: Vec<TestSuite>,
    tests: Vec<Test>,
}

/// Outcome of a test suite.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SuiteResult {
    #[serde(rename = "n")]
    name: Box<str>,
    #[serde(rename = "c")]
    total: usize,
    #[serde(rename = "o")]
    passed: usize,
    #[serde(rename = "i")]
    ignored: usize,
    #[serde(rename = "p")]
    panic: usize,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    #[serde(rename = "s")]
    suites: Vec<SuiteResult>,
    #[serde(rename = "t")]
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    tests: Vec<TestResult>,
}

/// Outcome of a test.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestResult {
    #[serde(rename = "n")]
    name: Box<str>,
    #[serde(rename = "s", default)]
    strict: bool,
    #[serde(skip)]
    result_text: Box<str>,
    #[serde(rename = "r")]
    result: TestOutcomeResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum TestOutcomeResult {
    #[serde(rename = "O")]
    Passed,
    #[serde(rename = "I")]
    Ignored,
    #[serde(rename = "F")]
    Failed,
    #[serde(rename = "P")]
    Panic,
}

/// Represents a test.
#[derive(Debug, Clone, Default)]
struct Test {
    name: Box<str>,
    description: Box<str>,
    esid: Option<Box<str>>,
    flags: TestFlags,
    information: Box<str>,
    features: Box<[Box<str>]>,
    expected_outcome: Outcome,
    includes: Box<[Box<str>]>,
    locale: Locale,
    content: Box<str>,
}

impl Test {
    /// Creates a new test.
    #[inline]
    fn new<N, C>(name: N, content: C, metadata: MetaData) -> Self
    where
        N: Into<Box<str>>,
        C: Into<Box<str>>,
    {
        Self {
            name: name.into(),
            description: metadata.description,
            esid: metadata.esid,
            flags: metadata.flags.into(),
            information: metadata.info,
            features: metadata.features,
            expected_outcome: Outcome::from(metadata.negative),
            includes: metadata.includes,
            locale: metadata.locale,
            content: content.into(),
        }
    }

    /// Sets the name of the test.
    fn set_name<N>(&mut self, name: N)
    where
        N: Into<Box<str>>,
    {
        self.name = name.into()
    }
}

/// An outcome for a test.
#[derive(Debug, Clone)]
enum Outcome {
    Positive,
    Negative { phase: Phase, error_type: Box<str> },
}

impl Default for Outcome {
    fn default() -> Self {
        Self::Positive
    }
}

impl From<Option<Negative>> for Outcome {
    fn from(neg: Option<Negative>) -> Self {
        neg.map(|neg| Self::Negative {
            phase: neg.phase,
            error_type: neg.error_type,
        })
        .unwrap_or_default()
    }
}

bitflags! {
    struct TestFlags: u16 {
        const STRICT = 0b000000001;
        const NO_STRICT = 0b000000010;
        const MODULE = 0b000000100;
        const RAW = 0b000001000;
        const ASYNC = 0b000010000;
        const GENERATED = 0b000100000;
        const CAN_BLOCK_IS_FALSE = 0b001000000;
        const CAN_BLOCK_IS_TRUE = 0b010000000;
        const NON_DETERMINISTIC = 0b100000000;
    }
}

impl Default for TestFlags {
    fn default() -> Self {
        Self::STRICT | Self::NO_STRICT
    }
}

impl From<TestFlag> for TestFlags {
    fn from(flag: TestFlag) -> Self {
        match flag {
            TestFlag::OnlyStrict => Self::STRICT,
            TestFlag::NoStrict => Self::NO_STRICT,
            TestFlag::Module => Self::MODULE,
            TestFlag::Raw => Self::RAW,
            TestFlag::Async => Self::ASYNC,
            TestFlag::Generated => Self::GENERATED,
            TestFlag::CanBlockIsFalse => Self::CAN_BLOCK_IS_FALSE,
            TestFlag::CanBlockIsTrue => Self::CAN_BLOCK_IS_TRUE,
            TestFlag::NonDeterministic => Self::NON_DETERMINISTIC,
        }
    }
}

impl<T> From<T> for TestFlags
where
    T: AsRef<[TestFlag]>,
{
    fn from(flags: T) -> Self {
        let flags = flags.as_ref();
        if flags.is_empty() {
            Self::default()
        } else {
            let mut result = Self::empty();
            for flag in flags {
                result |= Self::from(*flag);
            }

            if !result.intersects(Self::default()) {
                result |= Self::default()
            }

            result
        }
    }
}

/// Phase for an error.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Phase {
    Parse,
    Early,
    Resolution,
    Runtime,
}

/// Locale information structure.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(transparent)]
struct Locale {
    locale: Box<[Box<str>]>,
}
/// Program entry point.
fn main() {
    Platform::initialize();
    std::panic::set_hook(Box::new(|info| {
        println!("{}", info);
    }));
    match Cli::from_args() {
        Cli::Run {
            verbose,
            test262_path,
            suite,
            output,
        } => {
            run_test_suite(
                verbose,
                test262_path.as_path(),
                suite.as_path(),
                output.as_deref(),
            );
        }
        Cli::Compare {
            base,
            new,
            markdown,
            detail,
        } => compare_results(base.as_path(), new.as_path(), markdown, detail),
    }
}

fn show_result(results: &SuiteResult) {
    println!();
    println!("Results:");
    println!("Total tests: {}", results.total);
    println!("Passed tests: {}", results.passed.to_string().green());
    println!("Ignored tests: {}", results.ignored.to_string().yellow());
    println!(
        "Failed tests: {} (panics: {})",
        (results.total - results.passed - results.ignored - results.panic)
            .to_string()
            .red(),
        results.panic.to_string().red()
    );
    println!(
        "Conformance: {:.2}%",
        (results.passed as f64 / results.total as f64) * 100.0
    );
}

/// Runs the full test suite.
fn run_test_suite(verbose: u8, test262_path: &Path, suite: &Path, output: Option<&Path>) {
    if let Some(path) = output {
        if path.exists() {
            if !path.is_dir() {
                eprintln!("The output path must be a directory.");
                std::process::exit(1);
            }
        } else {
            fs::create_dir_all(path).expect("could not create the output directory");
        }
    }

    if verbose != 0 {
        println!("Loading the test suite...");
    }
    let harness = read_harness(test262_path).expect("could not read initialization bindings");

    let start = Instant::now();

    let results = if suite.to_string_lossy().ends_with(".js") {
        let options = Options::default();
        let mut rt = Runtime::new(options, None);
        let test = read_test(&test262_path.join(suite)).expect("could not get the test to run");

        if verbose != 0 {
            println!("Test loaded, starting...");
        }
        let suite = TestSuite {
            name: test.name.clone(),
            suites: Vec::new(),
            tests: vec![test],
        };
        suite.run_main(&harness, verbose, &mut rt)
    } else {
        let suite =
            read_suite(&test262_path.join(suite)).expect("could not get the list of tests to run");

        if verbose != 0 {
            println!("Test suite loaded, starting tests...");
        }
        let options = Options::default();
        let mut rt = Runtime::new(options, None);
        suite.run_main(&harness, verbose, &mut rt)
    };
    show_result(&results);
    write_json(results, output, verbose)
        .expect("could not write the results to the output JSON file");

    println!("Test262 Cost: {:?}", start.elapsed());
}
