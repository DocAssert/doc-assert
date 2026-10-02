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

    /// URI the request is sent to, as written in the documentation
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// Path to the documentation file the test case is defined in
    pub fn doc_path(&self) -> &str {
        &self.doc_path
    }

    /// Line number the request code block starts at
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
    /// Placeholders in the request or the expected response could not be resolved,
    /// usually because no variable of that name was defined or extracted earlier.
    /// The request was not sent.
    UnresolvedVariables {
        /// Names of the placeholders left, in the order they appear
        names: Vec<String>,
    },
    /// The documentation describes something that cannot be checked, such as an
    /// expected body that is not valid JSON. The request was not sent.
    InvalidDocumentation {
        /// Line number of the code block at fault
        line_number: usize,
        /// What is wrong with it
        reason: String,
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
            Failure::UnresolvedVariables { names } => {
                let names = names
                    .iter()
                    .map(|name| format!("`{}`", name))
                    .collect::<Vec<String>>();
                write!(f, "unresolved variables {}", names.join(", "))
            }
            Failure::InvalidDocumentation {
                line_number,
                reason,
            } => write!(
                f,
                "invalid documentation at line {}: {}",
                line_number, reason
            ),
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
            Mismatch::Body { differences } => {
                write!(f, "body differs from the expected one:")?;
                for difference in differences {
                    write!(f, "\n{}", difference)?;
                }
                Ok(())
            }
            Mismatch::VariableNotFound { name } => write!(
                f,
                "variable template {} not found in the response body",
                name
            ),
            Mismatch::UnreadableResponseBody { reason } => {
                write!(f, "error reading the response body: {}", reason)
            }
            Mismatch::MalformedResponseBody { reason } => {
                write!(f, "error parsing JSON response from the server: {}", reason)
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
    outcome: Result<(), Failure>,
}

impl TestCaseResult {
    pub(crate) fn new(id: TestCaseId, outcome: Result<(), Failure>) -> Self {
        Self { id, outcome }
    }

    /// The test case this is the outcome of
    pub fn id(&self) -> &TestCaseId {
        &self.id
    }

    /// Whether the test case passed
    pub fn passed(&self) -> bool {
        self.outcome.is_ok()
    }

    /// Why the test case failed, `None` if it passed
    pub fn failure(&self) -> Option<&Failure> {
        self.outcome.as_ref().err()
    }

    /// What follows the [`TestCaseId`] on the line of the test case
    pub(crate) fn mark(&self) -> &'static str {
        if self.passed() {
            "✅"
        } else {
            "❌"
        }
    }
}

impl Display for TestCaseResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.id, self.mark())
    }
}

/// Outcome of a run.
///
/// Displaying a report renders the whole thing, the same way `doc-assert` prints it: the
/// number of test cases, one line per executed test case, the details of the failures
/// and the final result.
///
/// A report produced by [`Run::finish`](crate::Run::finish) before the run was over only
/// holds the test cases that were executed. The ones left are counted as not run, and
/// such a report never [`passed`](Report::passed).
///
/// # Examples
///
/// ```
/// # use doc_assert::DocAssert;
/// async fn test() {
///     let report = DocAssert::new("http://localhost:8080")
///         .with_doc_path("README.md")
///         .run()
///         .await
///         .unwrap();
///     println!("{}", report);
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    results: Vec<TestCaseResult>,
    total_count: usize,
}

impl Report {
    pub(crate) fn new(results: Vec<TestCaseResult>, total_count: usize) -> Self {
        Self {
            results,
            total_count,
        }
    }

    /// Outcome of every executed test case, in the order they were executed
    pub fn results(&self) -> &[TestCaseResult] {
        &self.results
    }

    /// Every test case that failed, along with the reason it failed
    pub fn failures(&self) -> impl Iterator<Item = (&TestCaseId, &Failure)> {
        self.results
            .iter()
            .filter_map(|r| r.failure().map(|failure| (r.id(), failure)))
    }

    /// Number of test cases the documentation defines
    pub fn total_count(&self) -> usize {
        self.total_count
    }

    /// Number of test cases that were executed
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

    /// Number of test cases that were not executed because the run was stopped early
    pub fn not_run_count(&self) -> usize {
        self.total_count - self.executed_count()
    }

    /// Whether every test case the documentation defines was executed and passed
    pub fn passed(&self) -> bool {
        self.failed_count() == 0 && self.not_run_count() == 0
    }

    /// The details of the failures followed by the final result.
    ///
    /// This is what a [`Report`] displays after the line of every test case, so it is
    /// what is left to print once those lines have been printed as the test cases were
    /// executed.
    pub fn summary(&self) -> impl Display + '_ {
        Summary(self)
    }
}

impl Display for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{} tests", self.total_count)?;
        for result in &self.results {
            writeln!(f, "{}", result)?;
        }
        write!(f, "{}", self.summary())
    }
}

struct Summary<'a>(&'a Report);

impl Display for Summary<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let report = self.0;
        if report.failed_count() > 0 {
            writeln!(f, "\nfailures:")?;
            for (id, failure) in report.failures() {
                writeln!(f, "-------------\n{}: {}", id, failure)?;
            }
        }
        write!(
            f,
            "\ntest result: {}. {} passed; {} failed",
            if report.passed() { "PASSED" } else { "FAILED" },
            report.passed_count(),
            report.failed_count()
        )?;
        if report.not_run_count() > 0 {
            write!(f, "; {} not run", report.not_run_count())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Failure, Mismatch, Report, TestCaseId, TestCaseResult};

    fn id(http_method: &str, line_number: usize) -> TestCaseId {
        TestCaseId::new(
            http_method.to_string(),
            "/blog".to_string(),
            "README.md".to_string(),
            line_number,
        )
    }

    fn status_code_failure() -> Failure {
        Failure::ResponseMismatch {
            line_number: 36,
            cause: Mismatch::StatusCode {
                expected: 201,
                actual: 500,
            },
        }
    }

    #[test]
    fn test_report_of_a_passing_run() {
        let report = Report::new(vec![TestCaseResult::new(id("GET", 12), Ok(()))], 1);

        assert!(report.passed());
        assert_eq!(
            "1 tests\n\
             GET /blog (README.md:12) ✅\n\
             \n\
             test result: PASSED. 1 passed; 0 failed",
            report.to_string()
        );
    }

    #[test]
    fn test_report_of_a_failing_run() {
        let report = Report::new(
            vec![
                TestCaseResult::new(id("GET", 12), Ok(())),
                TestCaseResult::new(id("POST", 30), Err(status_code_failure())),
                TestCaseResult::new(
                    id("DELETE", 40),
                    Err(Failure::RequestFailed {
                        reason: "connection refused".to_string(),
                    }),
                ),
            ],
            3,
        );

        assert!(!report.passed());
        assert_eq!(2, report.failures().count());
        assert_eq!(
            "3 tests\n\
             GET /blog (README.md:12) ✅\n\
             POST /blog (README.md:30) ❌\n\
             DELETE /blog (README.md:40) ❌\n\
             \n\
             failures:\n\
             -------------\n\
             POST /blog (README.md:30): response at line 36: expected response code 201, got 500\n\
             -------------\n\
             DELETE /blog (README.md:40): request failed: connection refused\n\
             \n\
             test result: FAILED. 1 passed; 2 failed",
            report.to_string()
        );
    }

    #[test]
    fn test_report_of_a_run_stopped_early_does_not_pass() {
        let report = Report::new(vec![TestCaseResult::new(id("GET", 12), Ok(()))], 3);

        assert_eq!(2, report.not_run_count());
        assert!(!report.passed());
        assert_eq!(
            "3 tests\n\
             GET /blog (README.md:12) ✅\n\
             \n\
             test result: FAILED. 1 passed; 0 failed; 2 not run",
            report.to_string()
        );
    }

    #[test]
    fn test_report_without_test_cases_passes() {
        let report = Report::new(vec![], 0);

        assert!(report.passed());
        assert_eq!(
            "0 tests\n\ntest result: PASSED. 0 passed; 0 failed",
            report.to_string()
        );
    }

    #[test]
    fn test_failures_are_rendered() {
        let cases = [
            (
                Failure::UnresolvedVariables {
                    names: vec!["id".to_string(), "token".to_string()],
                },
                "unresolved variables `id`, `token`",
            ),
            (
                Failure::InvalidDocumentation {
                    line_number: 7,
                    reason: "expected body is not valid JSON".to_string(),
                },
                "invalid documentation at line 7: expected body is not valid JSON",
            ),
            (
                Failure::RequestFailed {
                    reason: "connection refused".to_string(),
                },
                "request failed: connection refused",
            ),
            (
                status_code_failure(),
                "response at line 36: expected response code 201, got 500",
            ),
        ];

        for (failure, expected) in cases {
            assert_eq!(expected, failure.to_string());
        }
    }

    #[test]
    fn test_mismatches_are_rendered() {
        let cases = [
            (
                Mismatch::StatusCode {
                    expected: 200,
                    actual: 404,
                },
                "expected response code 200, got 404",
            ),
            (
                Mismatch::Header {
                    name: "Content-Type".to_string(),
                    expected: "application/json".to_string(),
                    actual: "text/plain".to_string(),
                },
                "expected header Content-Type to be application/json, got text/plain",
            ),
            (
                Mismatch::MissingHeader {
                    name: "Content-Type".to_string(),
                },
                "expected header Content-Type not found",
            ),
            (
                Mismatch::Body {
                    differences: vec!["first".to_string(), "second".to_string()],
                },
                "body differs from the expected one:\nfirst\nsecond",
            ),
            (
                Mismatch::VariableNotFound {
                    name: "id".to_string(),
                },
                "variable template id not found in the response body",
            ),
            (
                Mismatch::UnreadableResponseBody {
                    reason: "connection reset".to_string(),
                },
                "error reading the response body: connection reset",
            ),
            (
                Mismatch::MalformedResponseBody {
                    reason: "expected value".to_string(),
                },
                "error parsing JSON response from the server: expected value",
            ),
        ];

        for (mismatch, expected) in cases {
            assert_eq!(expected, mismatch.to_string());
        }
    }
}
