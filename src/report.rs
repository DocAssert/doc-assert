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

//! What a run produced: the outcome of every test case and the way it is rendered.

use std::fmt::Display;

/// Identifies a single test case defined in the documentation.
///
/// Displayed the way it appears in a [`Report`], for example `GET /blog (README.md:12)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestCaseId {
    http_method: String,
    uri: String,
    doc_path: String,
    line_number: usize,
}

impl TestCaseId {
    pub(crate) fn new(
        http_method: String,
        uri: String,
        doc_path: String,
        line_number: usize,
    ) -> Self {
        Self {
            http_method,
            uri,
            doc_path,
            line_number,
        }
    }

    /// HTTP method of the request
    pub fn http_method(&self) -> &str {
        &self.http_method
    }

    /// URI the request is sent to
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// Path to the documentation file the test case is defined in
    pub fn doc_path(&self) -> &str {
        &self.doc_path
    }

    /// Line number the request is defined at
    pub fn line_number(&self) -> usize {
        self.line_number
    }
}

impl Display for TestCaseId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} ({}:{})",
            self.http_method, self.uri, self.doc_path, self.line_number
        )
    }
}

/// Reason a test case did not pass.
///
/// A `Failure` only carries what the [`TestCaseId`] of the enclosing [`TestCaseResult`]
/// does not already say, so rendering the two together does not repeat the request.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Failure {
    /// A placeholder in the request or the expected response could not be resolved,
    /// usually because no variable of that name was defined or extracted earlier
    UnresolvedVariables {
        /// The URI, body or header value the placeholders were left in
        input: String,
    },
    /// The request could not be sent
    RequestFailed {
        /// What went wrong while sending it
        reason: String,
    },
    /// The response did not match the one described in the documentation
    ResponseMismatch {
        /// Line number the expected response is defined at, which is not the line the
        /// request is defined at, see [`TestCaseId::line_number`]
        line_number: usize,
        /// How the response differed
        cause: Mismatch,
    },
}

impl Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failure::UnresolvedVariables { input } => {
                write!(f, "unresolved variable placeholders in {}", input)
            }
            Failure::RequestFailed { reason } => write!(f, "request failed: {}", reason),
            Failure::ResponseMismatch { line_number, cause } => {
                write!(f, "response at line {}: {}", line_number, cause)
            }
        }
    }
}

impl std::error::Error for Failure {}

/// How a response differed from the one described in the documentation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Mismatch {
    /// The status code was not the expected one
    StatusCode {
        /// Status code the documentation describes
        expected: u16,
        /// Status code the server responded with
        actual: u16,
    },
    /// A header held a different value than the expected one
    Header {
        /// Name of the header
        name: String,
        /// Value the documentation describes
        expected: String,
        /// Value the server responded with
        actual: String,
    },
    /// A header described in the documentation was not present in the response
    MissingHeader {
        /// Name of the header
        name: String,
    },
    /// The body differed from the expected one
    Body {
        /// One entry per difference found, in the order they were found
        differences: Vec<String>,
    },
    /// A variable the documentation extracts from the response was not found in it
    VariableNotFound {
        /// Name of the variable
        name: String,
    },
    /// An `[ignore]` or `[ignore-order]` JSONPath could not be parsed
    InvalidIgnorePath {
        /// The JSONPath as written in the documentation
        path: String,
        /// Why it could not be parsed
        reason: String,
    },
    /// The response body could not be read
    UnreadableResponseBody {
        /// Why it could not be read
        reason: String,
    },
    /// The response body was not valid JSON
    MalformedResponseBody {
        /// Why it could not be parsed
        reason: String,
    },
    /// The body described in the documentation was not valid JSON
    MalformedExpectedBody {
        /// Why it could not be parsed
        reason: String,
    },
}

impl Display for Mismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mismatch::StatusCode { expected, actual } => {
                write!(f, "expected response code {}, got {}", expected, actual)
            }
            Mismatch::Header {
                name,
                expected,
                actual,
            } => write!(
                f,
                "expected header {} to be {}, got {}",
                name, expected, actual
            ),
            Mismatch::MissingHeader { name } => write!(f, "expected header {} not found", name),
            Mismatch::Body { differences } => write!(
                f,
                "expected response differs from actual {}",
                differences.join("\n")
            ),
            Mismatch::VariableNotFound { name } => write!(
                f,
                "variable template {} not found in the response body",
                name
            ),
            Mismatch::InvalidIgnorePath { path, reason } => {
                write!(f, "invalid path {}: {}", path, reason)
            }
            Mismatch::UnreadableResponseBody { reason } => write!(f, "{}", reason),
            Mismatch::MalformedResponseBody { reason } => {
                write!(f, "error parsing JSON response from the server: {}", reason)
            }
            Mismatch::MalformedExpectedBody { reason } => {
                write!(f, "error parsing JSON: {}", reason)
            }
        }
    }
}

/// Outcome of a single executed test case.
///
/// Displayed as the line it takes in a [`Report`], for example `GET /blog (README.md:12) ✅`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestCaseResult {
    id: TestCaseId,
    failure: Option<Failure>,
}

impl TestCaseResult {
    pub(crate) fn new(id: TestCaseId, failure: Option<Failure>) -> Self {
        Self { id, failure }
    }

    /// The test case this is the outcome of
    pub fn id(&self) -> &TestCaseId {
        &self.id
    }

    /// Whether the test case passed
    pub fn passed(&self) -> bool {
        self.failure.is_none()
    }

    /// Why the test case failed, `None` if it passed
    pub fn failure(&self) -> Option<&Failure> {
        self.failure.as_ref()
    }
}

impl Display for TestCaseResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mark = if self.passed() { "✅" } else { "❌" };
        write!(f, "{} {}", self.id, mark)
    }
}

/// Outcome of a whole run.
///
/// A report is produced whenever the run completed, whether the test cases passed or not,
/// so it always describes what happened. Use [`Report::passed`] for the verdict.
///
/// Displaying a report renders the whole thing: the number of test cases, one line per
/// test case, the details of the failures and the final result. When the test cases are
/// printed while they are executed, print [`Report::summary`] instead so that the lines
/// are not repeated.
///
/// # Examples
///
/// ```
/// # use doc_assert::DocAssert;
/// async fn test() {
///     let report = DocAssert::new()
///         .with_url("http://localhost:8080")
///         .with_doc_path("README.md")
///         .run()
///         .await
///         .unwrap();
///     println!("{}", report);
///     assert!(report.passed());
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    results: Vec<TestCaseResult>,
}

impl Report {
    pub(crate) fn new(results: Vec<TestCaseResult>) -> Self {
        Self { results }
    }

    /// Outcome of every test case, in the order they were executed
    pub fn results(&self) -> &[TestCaseResult] {
        &self.results
    }

    /// Outcome of every test case that failed
    pub fn failures(&self) -> impl Iterator<Item = &TestCaseResult> {
        self.results.iter().filter(|r| !r.passed())
    }

    /// Number of test cases that were executed.
    ///
    /// A run that was stopped early only reports the test cases it got to, so this is
    /// not necessarily the number of test cases the documentation defines, which is
    /// what [`Run::total_count`](crate::Run::total_count) gives.
    pub fn executed_count(&self) -> usize {
        self.results.len()
    }

    /// Number of test cases that passed
    pub fn passed_count(&self) -> usize {
        self.results.iter().filter(|r| r.passed()).count()
    }

    /// Number of test cases that failed
    pub fn failed_count(&self) -> usize {
        self.executed_count() - self.passed_count()
    }

    /// Whether every executed test case passed
    pub fn passed(&self) -> bool {
        self.failed_count() == 0
    }

    /// The details of the failures followed by the final result.
    ///
    /// This is everything a [`Report`] displays except the number of test cases and the
    /// line of every test case, so it is what is left to print once the test cases have
    /// been printed as they were executed.
    pub fn summary(&self) -> Summary<'_> {
        Summary(self)
    }

    /// Panics with the whole report if any test case failed.
    ///
    /// Meant for use inside a test, where a failed test case should fail the test.
    ///
    /// # Panics
    ///
    /// Panics if [`Report::passed`] is `false`.
    pub fn assert_passed(&self) {
        assert!(self.passed(), "{}", self);
    }
}

impl Display for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{} tests", self.executed_count())?;
        for result in &self.results {
            writeln!(f, "{}", result)?;
        }
        write!(f, "{}", self.summary())
    }
}

/// The details of the failures followed by the final result, returned by [`Report::summary`].
#[derive(Debug)]
pub struct Summary<'a>(&'a Report);

impl Display for Summary<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let report = self.0;
        if !report.passed() {
            writeln!(f, "\nfailures:")?;
            for result in report.failures() {
                // `failure` is always set on a result that did not pass
                if let Some(failure) = result.failure() {
                    writeln!(f, "-------------\n{}: {}", result.id(), failure)?;
                }
            }
        }
        write!(
            f,
            "\ntest result: {}. {} passed; {} failed",
            if report.passed() { "PASSED" } else { "FAILED" },
            report.passed_count(),
            report.failed_count()
        )
    }
}
