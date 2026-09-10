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
#![allow(clippy::while_let_on_iterator)]

use crate::{
    domain::{Request, Response, TestCase},
    json_diff::path::{Key, Path},
};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Display;

mod domain;
mod executor;
mod json_diff;
mod parser;
mod report;

pub use report::{Failure, Mismatch, Report, Summary, TestCaseId, TestCaseResult};

/// Builder for a documentation test run.
///
/// # Examples
///
/// ```
/// use doc_assert::DocAssert;
///
/// async fn test() {
///     DocAssert::new()
///         .with_url("http://localhost:8080")
///         .with_doc_path("README.md")
///         .assert()
///         .await;
/// }
/// ```
#[derive(Debug, Default)]
pub struct DocAssert {
    url: Option<String>,
    doc_paths: Vec<String>,
    variables: Variables,
}

impl DocAssert {
    /// Constructs a new, empty `DocAssert` builder.
    ///
    /// A URL and at least one documentation file are required before the run can start.
    ///
    /// # Examples
    ///
    /// ```
    /// use doc_assert::DocAssert;
    /// let doc_assert = DocAssert::new();
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the base URL to test against.
    ///
    /// Required. The URL every request of the documentation is sent to.
    ///
    /// # Examples
    ///
    /// ```
    /// use doc_assert::DocAssert;
    /// let doc_assert = DocAssert::new().with_url("http://localhost:8080");
    /// ```
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
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
    /// let doc_assert = DocAssert::new().with_doc_path("README.md");
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
    /// let doc_assert = DocAssert::new().with_variables(variables);
    /// ```
    pub fn with_variables(mut self, variables: Variables) -> Self {
        self.variables = variables;
        self
    }

    /// Executes every test case and panics unless all of them passed.
    ///
    /// This is how `DocAssert` is meant to be used inside a test: anything that goes
    /// wrong fails the test, whether it is a documentation file that could not be parsed
    /// or a test case that did not pass. Use [`DocAssert::run`] to get the [`Report`] and
    /// handle it yourself, or [`DocAssert::start`] to handle every test case as soon as
    /// it has been executed.
    ///
    /// # Panics
    ///
    /// Panics if the run could not be performed, or if any test case failed. The whole
    /// [`Report`] is the panic message.
    ///
    /// # Examples
    ///
    /// ```
    /// use doc_assert::DocAssert;
    ///
    /// async fn test() {
    ///     DocAssert::new()
    ///         .with_url("http://localhost:8080")
    ///         .with_doc_path("README.md")
    ///         .assert()
    ///         .await;
    /// }
    /// ```
    pub async fn assert(self) -> Report {
        let report = match self.run().await {
            Ok(report) => report,
            Err(err) => panic!("{}", err),
        };
        report.assert_passed();
        report
    }

    /// Executes every test case and returns the [`Report`].
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
    ///     let report = DocAssert::new()
    ///         .with_url("http://localhost:8080")
    ///         .with_doc_path("README.md")
    ///         .run()
    ///         .await
    ///         .unwrap();
    ///
    ///     println!("{} of {} passed", report.passed_count(), report.executed_count());
    /// }
    /// ```
    pub async fn run(self) -> Result<Report, Error> {
        Ok(self.start()?.run_to_end().await)
    }

    /// Parses the documentation and prepares the run without executing anything yet.
    ///
    /// Use this to drive the run yourself and handle every test case as soon as it has
    /// been executed.
    ///
    /// # Examples
    ///
    /// ```
    /// use doc_assert::DocAssert;
    ///
    /// async fn test() {
    ///     let mut run = DocAssert::new()
    ///         .with_url("http://localhost:8080")
    ///         .with_doc_path("README.md")
    ///         .start()
    ///         .unwrap();
    ///
    ///     println!("{} tests", run.total_count());
    ///     while let Some(result) = run.next().await {
    ///         println!("{}", result);
    ///     }
    ///
    ///     let report = run.finish();
    ///     println!("{}", report.summary());
    /// }
    /// ```
    pub fn start(self) -> Result<Run, Error> {
        let url = self.url.ok_or(Error::NoUrl)?;
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
            url,
            variables: self.variables,
            total_count: pending.len(),
            pending: pending.into_iter(),
            results: vec![],
        })
    }
}

/// A run in progress, returned by [`DocAssert::start`].
///
/// The test cases are executed one by one, as [`Run::next`] is called, so their results
/// are available while the run is still going on. They are executed in the order they
/// appear in the documentation because a test case may use variables extracted from the
/// responses of the previous ones.
///
/// Dropping a `Run` cancels it; the test cases that were already executed are lost with
/// it, so call [`Run::finish`] to get the [`Report`] of a partial run.
#[derive(Debug)]
pub struct Run {
    url: String,
    variables: Variables,
    pending: std::vec::IntoIter<(TestCaseId, TestCase)>,
    results: Vec<TestCaseResult>,
    total_count: usize,
}

impl Run {
    /// Total number of test cases the documentation defines.
    ///
    /// This is how many test cases the run would execute if it were driven to the end;
    /// a run that is stopped early reports fewer, see [`Report::executed_count`].
    pub fn total_count(&self) -> usize {
        self.total_count
    }

    /// Number of test cases executed so far
    pub fn completed_count(&self) -> usize {
        self.results.len()
    }

    /// Executes the next test case, `None` once every one of them has been executed.
    ///
    /// # Cancellation
    ///
    /// This is not cancellation safe. The test case is taken off the queue before the
    /// request is sent, so dropping the returned future part way through — racing it
    /// against a timeout, or selecting on it — loses that test case: it is neither
    /// retried nor reported. Drive it to completion, and stop the run between calls.
    pub async fn next(&mut self) -> Option<TestCaseResult> {
        let (id, test_case) = self.pending.next()?;
        let failure = executor::execute(&self.url, test_case, &mut self.variables)
            .await
            .err();
        let result = TestCaseResult::new(id, failure);
        self.results.push(result.clone());
        Some(result)
    }

    /// Executes every remaining test case and returns the [`Report`].
    pub async fn run_to_end(mut self) -> Report {
        while self.next().await.is_some() {}
        self.finish()
    }

    /// Returns the [`Report`] of the test cases executed so far.
    pub fn finish(self) -> Report {
        Report::new(self.results)
    }
}

/// The run could not be performed.
///
/// This is not how a failed test case is reported; a run that executed its test cases
/// always produces a [`Report`], whether they passed or not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// No URL to test against was given
    NoUrl,
    /// No documentation file was given
    NoDocuments,
    /// A documentation file could not be read or parsed
    Parse {
        /// Path to the documentation file
        doc_path: String,
        /// Why it could not be read or parsed
        reason: String,
    },
    /// The variables were not given as a JSON object
    VariablesNotAnObject,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NoUrl => write!(f, "no URL to test against"),
            Error::NoDocuments => write!(f, "no documentation file to test"),
            Error::Parse { doc_path, reason } => {
                write!(f, "error parsing {}: {}", doc_path, reason)
            }
            Error::VariablesNotAnObject => write!(f, "variables must be a JSON object"),
        }
    }
}

impl std::error::Error for Error {}

/// Variables to be used in the request and response bodies.
///
/// The variables replace placeholders in the request and response bodies, in case some
/// values need to be shared between requests.
///
/// # Examples
///
/// Variables can be inserted one by one:
///
/// ```
/// # use doc_assert::Variables;
/// let mut variables = Variables::new();
/// variables.insert("name", "John");
/// variables.insert("age", 30);
/// ```
///
/// Alternatively, they can be passed as a JSON object:
///
/// ```
/// # use doc_assert::Variables;
/// let json = r#"{"name": "John", "age": 30}"#;
/// let variables = Variables::from_json(&serde_json::from_str(json).unwrap()).unwrap();
/// ```
#[derive(Debug, Clone, Default)]
pub struct Variables {
    map: HashMap<String, Value>,
}

impl Variables {
    /// Constructs a new, empty `Variables`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use doc_assert::Variables;
    /// let variables = Variables::new();
    /// ```
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Constructs a new `Variables` from a JSON object.
    ///
    /// # Examples
    ///
    /// ```
    /// # use doc_assert::Variables;
    /// let json = r#"{"name": "John", "age": 30}"#;
    /// let variables = Variables::from_json(&serde_json::from_str(json).unwrap()).unwrap();
    /// ```
    pub fn from_json(json: &Value) -> Result<Self, Error> {
        match json {
            Value::Object(obj) => Ok(Self {
                map: obj.clone().into_iter().collect(),
            }),
            _ => Err(Error::VariablesNotAnObject),
        }
    }

    /// Inserts a variable, overwriting any variable of the same name.
    ///
    /// Anything a `serde_json::Value` can be built from is accepted, which covers the
    /// strings, numbers and booleans a documentation usually needs. Pass a `Value`
    /// itself for the types it does not cover.
    ///
    /// # Examples
    ///
    /// ```
    /// # use doc_assert::Variables;
    /// # use serde_json::Value;
    /// let mut variables = Variables::new();
    /// variables.insert("name", "John");
    /// variables.insert("age", 30);
    /// variables.insert("is_adult", true);
    /// variables.insert("nickname", Value::Null);
    /// ```
    pub fn insert(&mut self, name: impl Into<String>, value: impl Into<Value>) {
        self.map.insert(name.into(), value.into());
    }

    pub(crate) fn obtain_from_response(
        &mut self,
        response: &Value,
        variable_templates: &HashMap<String, Path>,
    ) -> Result<(), Mismatch> {
        for (name, path) in variable_templates {
            let value = extract_value(path, response)
                .ok_or_else(|| Mismatch::VariableNotFound { name: name.clone() })?;

            self.map.insert(name.clone(), value);
        }

        Ok(())
    }

    fn replace_placeholders(&self, input: &mut String, trim_quotes: bool) -> Result<(), Failure> {
        for (name, value) in &self.map {
            let placeholder = format!("`{}`", name);
            let value_str = value.to_string();

            let value = if trim_quotes {
                value_str.trim_matches('"')
            } else {
                value_str.as_str()
            };

            *input = input.replace(&placeholder, value);
        }

        if input.contains('`') {
            return Err(Failure::UnresolvedVariables {
                input: input.clone(),
            });
        }

        Ok(())
    }

    pub(crate) fn replace_request_placeholders(&self, input: &mut Request) -> Result<(), Failure> {
        self.replace_placeholders(&mut input.uri, true)?;

        if let Some(body) = &mut input.body {
            self.replace_placeholders(body, false)?;
        }

        for (_, value) in &mut input.headers.iter_mut() {
            self.replace_placeholders(value, true)?;
        }

        Ok(())
    }

    pub(crate) fn replace_response_placeholders(
        &self,
        input: &mut Response,
    ) -> Result<(), Failure> {
        if let Some(body) = &mut input.body {
            self.replace_placeholders(body, false)?;
        }

        for (_, value) in &mut input.headers.iter_mut() {
            self.replace_placeholders(value, true)?;
        }

        Ok(())
    }
}

fn extract_value(path: &Path, value: &Value) -> Option<Value> {
    match path {
        Path::Root => None,
        Path::Keys(keys) => {
            let mut current = value;
            for key in keys {
                match key {
                    Key::Field(field) => current = current.get(field)?,
                    Key::Idx(index) => current = current.get(index)?,
                    _ => return None,
                }
            }
            Some(current.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{DocAssert, Error, Failure, Mismatch};

    /// Documentation file removing itself once it goes out of scope.
    struct TempDoc {
        path: std::path::PathBuf,
    }

    impl TempDoc {
        fn new(content: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "doc_assert_{}_{}.md",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::write(&path, content).unwrap();
            Self { path }
        }

        fn path(&self) -> &str {
            self.path.to_str().unwrap()
        }
    }

    impl Drop for TempDoc {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    /// One test case hitting `/passing`, then one hitting `/failing`.
    const DOC: &str = "```docassertrequest\n\
                       GET /passing\n\
                       ```\n\
                       ```docassertresponse\n\
                       HTTP 200\n\
                       ```\n\
                       ```docassertrequest\n\
                       GET /failing\n\
                       ```\n\
                       ```docassertresponse\n\
                       HTTP 200\n\
                       ```\n";

    async fn server() -> mockito::ServerGuard {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/passing")
            .with_status(200)
            .create_async()
            .await;
        server
            .mock("GET", "/failing")
            .with_status(500)
            .create_async()
            .await;
        server
    }

    #[tokio::test]
    async fn test_every_test_case_is_available_as_soon_as_it_is_executed() {
        let server = server().await;
        let doc = TempDoc::new(DOC);

        let mut run = DocAssert::new()
            .with_url(server.url())
            .with_doc_path(doc.path())
            .start()
            .unwrap();

        // the count is known before anything has been executed
        assert_eq!(2, run.total_count());
        assert_eq!(0, run.completed_count());

        let first = run.next().await.unwrap();
        assert!(first.passed());
        assert_eq!("GET", first.id().http_method());
        assert_eq!("/passing", first.id().uri());
        // the second test case has not been executed at the point the first is returned
        assert_eq!(1, run.completed_count());

        let second = run.next().await.unwrap();
        assert!(!second.passed());
        assert_eq!("/failing", second.id().uri());

        assert!(run.next().await.is_none());

        let report = run.finish();
        assert_eq!(2, report.executed_count());
        assert_eq!(1, report.passed_count());
        assert_eq!(1, report.failed_count());
        assert!(!report.passed());
        assert_eq!(1, report.failures().count());
    }

    #[tokio::test]
    async fn test_failure_carries_the_reason_it_failed() {
        let server = server().await;
        let doc = TempDoc::new(DOC);

        let report = DocAssert::new()
            .with_url(server.url())
            .with_doc_path(doc.path())
            .run()
            .await
            .unwrap();

        let failure = report.failures().next().unwrap().failure().unwrap();
        match failure {
            Failure::ResponseMismatch { cause, .. } => assert_eq!(
                &Mismatch::StatusCode {
                    expected: 200,
                    actual: 500
                },
                cause
            ),
            other => panic!("unexpected failure: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_run_can_be_stopped_early_and_still_report() {
        let server = server().await;
        let doc = TempDoc::new(DOC);

        let mut run = DocAssert::new()
            .with_url(server.url())
            .with_doc_path(doc.path())
            .start()
            .unwrap();

        run.next().await.unwrap();
        let report = run.finish();

        // only the test cases that were executed are reported
        assert_eq!(1, report.executed_count());
        assert!(report.passed());
    }

    #[test]
    fn test_a_run_without_documentation_is_an_error() {
        assert_eq!(
            Some(Error::NoDocuments),
            DocAssert::new()
                .with_url("http://localhost:8080")
                .start()
                .err()
        );
    }

    #[test]
    fn test_a_run_without_a_url_is_an_error() {
        assert_eq!(
            Some(Error::NoUrl),
            DocAssert::new().with_doc_path("README.md").start().err()
        );
    }

    #[test]
    fn test_parsing_error_names_the_file_it_comes_from() {
        let err = DocAssert::new()
            .with_url("http://localhost:8080")
            .with_doc_path("this/file/does/not/exist.md")
            .start()
            .unwrap_err();

        match err {
            Error::Parse { doc_path, .. } => assert_eq!("this/file/does/not/exist.md", doc_path),
            other => panic!("unexpected error: {:?}", other),
        }
    }

    #[test]
    fn test_report_renders_the_same_thing_as_the_streamed_output() {
        let report = crate::Report::new(vec![
            crate::TestCaseResult::new(
                crate::TestCaseId::new(
                    "GET".to_string(),
                    "/blog".to_string(),
                    "README.md".to_string(),
                    12,
                ),
                None,
            ),
            crate::TestCaseResult::new(
                crate::TestCaseId::new(
                    "POST".to_string(),
                    "/blog".to_string(),
                    "README.md".to_string(),
                    30,
                ),
                Some(Failure::ResponseMismatch {
                    line_number: 36,
                    cause: Mismatch::StatusCode {
                        expected: 201,
                        actual: 500,
                    },
                }),
            ),
        ]);

        // what the binary prints line by line has to add up to what `Report` displays
        let streamed = format!(
            "{} tests\n{}\n{}\n",
            report.executed_count(),
            report
                .results()
                .iter()
                .map(|r| r.to_string())
                .collect::<Vec<String>>()
                .join("\n"),
            report.summary()
        );
        assert_eq!(format!("{}\n", report), streamed);
    }
}
