// Copyright 2024 The DocAssert Authors
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]

use crate::domain::TestCase;
use std::fmt::Display;
use std::io::Write;

mod domain;
mod executor;
mod json_diff;
mod parser;
mod report;
mod variables;

pub use report::{Failure, Mismatch, Report, TestCaseId, TestCaseResult};
pub use variables::Variables;

/// Builder for a documentation test run.
///
/// Once configured, the run is performed in one of three ways:
///
/// - [`DocAssert::assert`] prints every test case as it is executed and fails the test
///   unless all of them passed, which is what a test suite usually wants;
/// - [`DocAssert::run`] prints nothing and returns the [`Report`];
/// - [`DocAssert::start`] returns a [`Run`] to execute the test cases one by one.
#[derive(Debug)]
pub struct DocAssert {
    url: String,
    doc_paths: Vec<String>,
    variables: Variables,
}

impl DocAssert {
    /// Constructs a `DocAssert` builder testing against `url`.
    ///
    /// `url` is the base URL every request of the documentation is sent to. At least one
    /// documentation file is also required before the run can start, see
    /// [`DocAssert::with_doc_path`].
    ///
    /// # Examples
    ///
    /// ```
    /// use doc_assert::DocAssert;
    /// let doc_assert = DocAssert::new("http://localhost:8080");
    /// ```
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            doc_paths: vec![],
            variables: Variables::new(),
        }
    }

    /// Adds a documentation file to test.
    ///
    /// At least one is required. The test cases of every file are executed in the order
    /// the files were added.
    ///
    /// # Examples
    ///
    /// ```
    /// use doc_assert::DocAssert;
    /// let doc_assert = DocAssert::new("http://localhost:8080").with_doc_path("README.md");
    /// ```
    pub fn with_doc_path(mut self, doc_path: impl Into<String>) -> Self {
        self.doc_paths.push(doc_path.into());
        self
    }

    /// Sets the variables to be used in the assertions.
    ///
    /// The variables replace the placeholders in the documentation.
    ///
    /// # Examples
    ///
    /// ```
    /// use doc_assert::{DocAssert, Variables};
    ///
    /// let mut variables = Variables::new();
    /// variables.insert("token", "abcd");
    /// let doc_assert = DocAssert::new("http://localhost:8080").with_variables(variables);
    /// ```
    pub fn with_variables(mut self, variables: Variables) -> Self {
        self.variables = variables;
        self
    }

    /// Executes every test case, printing each one as it is executed, and panics unless
    /// all of them passed.
    ///
    /// The output is the one of the `doc-assert` binary, see [`Run::print_progress`]. It
    /// is printed the way `cargo test` expects test output to be, so it is shown when the
    /// test fails, or as it is produced with `cargo test -- --nocapture`.
    ///
    /// # Panics
    ///
    /// Panics with the [`Error`] if the run could not be performed, and when any test case
    /// failed.
    ///
    /// # Examples
    ///
    /// ```
    /// use doc_assert::DocAssert;
    ///
    /// async fn test() {
    ///     DocAssert::new("http://localhost:8080")
    ///         .with_doc_path("README.md")
    ///         .assert()
    ///         .await;
    /// }
    /// ```
    pub async fn assert(self) {
        let run = match self.start() {
            Ok(run) => run,
            Err(err) => panic!("{}", err),
        };
        let report = run.print_progress().await;
        assert!(
            report.passed(),
            "{} of {} documentation test cases failed",
            report.failed_count(),
            report.total_count()
        );
    }

    /// Executes every test case and returns the [`Report`], printing nothing.
    ///
    /// `Err` means the run could not be performed at all; test cases that failed are
    /// reported by the [`Report`] itself, see [`Report::passed`].
    ///
    /// # Examples
    ///
    /// ```
    /// use doc_assert::DocAssert;
    ///
    /// async fn test() {
    ///     let report = DocAssert::new("http://localhost:8080")
    ///         .with_doc_path("README.md")
    ///         .run()
    ///         .await
    ///         .unwrap();
    ///
    ///     println!("{} of {} passed", report.passed_count(), report.total_count());
    /// }
    /// ```
    pub async fn run(self) -> Result<Report, Error> {
        let mut run = self.start()?;
        while run.next().await.is_some() {}
        Ok(run.finish())
    }

    /// Parses the documentation and prepares the run without executing anything yet.
    ///
    /// # Examples
    ///
    /// ```
    /// use doc_assert::DocAssert;
    ///
    /// async fn test() {
    ///     let mut run = DocAssert::new("http://localhost:8080")
    ///         .with_doc_path("README.md")
    ///         .start()
    ///         .unwrap();
    ///
    ///     while let Some(result) = run.next().await {
    ///         if !result.passed() {
    ///             break; // fail fast
    ///         }
    ///     }
    ///
    ///     let report = run.finish();
    /// }
    /// ```
    pub fn start(self) -> Result<Run, Error> {
        if self.doc_paths.is_empty() {
            return Err(Error::NoDocuments);
        }

        // every documentation file is parsed upfront so that a parsing error is reported
        // before any request is made and the number of test cases is known in advance
        let mut pending = vec![];
        for doc_path in &self.doc_paths {
            let test_cases = parser::parse(doc_path.clone()).map_err(|reason| Error::Parse {
                doc_path: doc_path.clone(),
                reason,
            })?;
            for test_case in test_cases {
                let id = TestCaseId::new(
                    test_case.request.http_method.to_string(),
                    test_case.request.uri.clone(),
                    doc_path.clone(),
                    test_case.request.line_number,
                );
                pending.push((id, test_case));
            }
        }

        Ok(Run {
            url: self.url,
            variables: self.variables,
            total_count: pending.len(),
            pending: pending.into_iter(),
            results: vec![],
        })
    }
}

/// A run in progress, returned by [`DocAssert::start`].
///
/// The test cases are executed one by one, in the order they appear in the documentation,
/// because a test case may use variables extracted from the responses of the previous
/// ones.
///
/// Dropping a `Run` discards the results of the test cases it executed; call
/// [`Run::finish`] to get the [`Report`] of the test cases executed so far.
#[derive(Debug)]
pub struct Run {
    url: String,
    variables: Variables,
    pending: std::vec::IntoIter<(TestCaseId, TestCase)>,
    results: Vec<TestCaseResult>,
    total_count: usize,
}

impl Run {
    /// Number of test cases the documentation defines
    pub fn total_count(&self) -> usize {
        self.total_count
    }

    /// Executes the next test case, `None` once every one of them has been executed.
    ///
    /// # Cancellation
    ///
    /// This is not cancellation safe. The test case is taken off the queue before the
    /// request is sent, so dropping the returned future part way through — racing it
    /// against a timeout, or selecting on it — loses that test case: it is not reported,
    /// even though its request may already have reached the server, and the variables it
    /// would have extracted are missing for the test cases after it. Drive it to
    /// completion, and stop the run between calls.
    pub async fn next(&mut self) -> Option<TestCaseResult> {
        let (id, test_case) = self.pending.next()?;
        Some(self.execute(id, test_case).await)
    }

    /// Executes every remaining test case, printing each one to stdout as it is executed,
    /// and returns the [`Report`].
    ///
    /// The output follows `cargo test`: the number of test cases, then one line per test
    /// case, printed as soon as it starts and completed with ✅ or ❌ once it is done, and
    /// finally what a [`Report`] displays after those lines, see [`Report::summary`]:
    ///
    /// ```text
    /// 2 tests
    /// GET /blog (README.md:12) ✅
    /// POST /blog (README.md:30) ❌
    ///
    /// failures:
    /// -------------
    /// POST /blog (README.md:30): response at line 36: expected response code 201, got 500
    ///
    /// test result: FAILED. 1 passed; 1 failed
    /// ```
    pub async fn print_progress(mut self) -> Report {
        println!("{} tests", self.total_count);
        while let Some((id, test_case)) = self.pending.next() {
            // the test case is named before it is executed so that a slow one, or one
            // being retried, shows what is being waited for
            print!("{} ", id);
            let _ = std::io::stdout().flush();
            let result = self.execute(id, test_case).await;
            println!("{}", result.mark());
        }

        let report = self.finish();
        println!("{}", report.summary());
        report
    }

    /// Returns the [`Report`] of the test cases executed so far.
    pub fn finish(self) -> Report {
        Report::new(self.results, self.total_count)
    }

    async fn execute(&mut self, id: TestCaseId, test_case: TestCase) -> TestCaseResult {
        let outcome = executor::execute(&self.url, test_case, &mut self.variables).await;
        let result = TestCaseResult::new(id, outcome);
        self.results.push(result.clone());
        result
    }
}

/// The run could not be performed.
///
/// This is not how a failed test case is reported; a run that executed its test cases
/// always produces a [`Report`], whether they passed or not.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// No documentation file was given
    NoDocuments,
    /// A documentation file could not be read or parsed
    Parse {
        /// Path to the documentation file
        doc_path: String,
        /// Why it could not be read or parsed
        reason: String,
    },
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NoDocuments => write!(f, "no documentation file to test"),
            Error::Parse { doc_path, reason } => {
                write!(f, "cannot parse {}: {}", doc_path, reason)
            }
        }
    }
}

impl std::error::Error for Error {}
