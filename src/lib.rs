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
    domain::{Request, Response},
    json_diff::path::{Key, Path},
};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Display;
use std::io::Write;
use std::vec;

mod domain;
mod executor;
mod json_diff;
mod parser;

/// Builder for the assertions.
///
/// The builder is used to configure the assertions.
///
/// # Examples
///
/// ```
/// # #![allow(unused_mut)]
/// use doc_assert::DocAssert;
/// use doc_assert::Variables;
///
/// async fn test() {
///     // Create Variables for values that will be shared between requests and responses
///     let mut variables = Variables::new();
///     variables.insert_string("token".to_string(), "abcd".to_string());
///     // Create a DocAssert builder with the base URL and the path to the documentation file
///     let mut doc_assert = DocAssert::new()
///         .with_url("http://localhost:8080")
///         .with_doc_path("path/to/README.md");
///     // Execute the assertions
///     let report = doc_assert.assert().await;
/// }
/// ```
pub struct DocAssert<'a> {
    url: Option<&'a str>,
    doc_paths: Vec<&'a str>,
    pub(crate) variables: Variables,
    reporter: Option<Box<dyn Reporter + 'a>>,
}

impl<'a> DocAssert<'a> {
    /// Constructs a new, empty `DocAssert` builder.
    ///
    /// The builder is used to configure the assertions.
    ///
    /// # Examples
    ///
    /// ```
    /// # #![allow(unused_mut)]
    /// use doc_assert::DocAssert;
    /// let mut doc_assert = DocAssert::new();
    /// ```
    pub fn new() -> Self {
        Self {
            url: None,
            doc_paths: vec![],
            variables: Variables::new(),
            reporter: None,
        }
    }

    /// Sets the base URL to test against.
    ///
    /// The URL will be used to make the requests.
    ///
    /// # Examples
    ///
    /// ```
    /// # #![allow(unused_mut)]
    /// use doc_assert::DocAssert;
    /// let mut doc_assert = DocAssert::new().with_url("http://localhost:8080");
    /// ```
    pub fn with_url(mut self, url: &'a str) -> Self {
        self.url = Some(url);
        self
    }

    /// Sets the path to the documentation file.
    ///
    /// The path will be used to parse the documentation.
    ///
    /// # Examples
    ///
    /// ```
    /// # #![allow(unused_mut)]
    /// use doc_assert::DocAssert;
    /// let mut doc_assert = DocAssert::new().with_doc_path("path/to/README.md");
    /// ```
    pub fn with_doc_path(mut self, doc_path: &'a str) -> Self {
        self.doc_paths.push(doc_path);
        self
    }

    /// Sets the variables to be used in the assertions.
    ///
    /// The variables will be used to replace the placeholders in the documentation.
    ///
    /// # Examples
    ///
    /// ```
    /// # #![allow(unused_mut)]
    /// use doc_assert::DocAssert;
    /// use doc_assert::Variables;
    ///
    /// let mut variables = Variables::new();
    /// variables.insert_string("token".to_string(), "abcd".to_string());
    /// let mut doc_assert = DocAssert::new().with_variables(variables);
    /// ```
    pub fn with_variables(mut self, variables: Variables) -> Self {
        self.variables = variables;
        self
    }

    /// Sets the reporter notified about every test case as soon as it is executed.
    ///
    /// Without a reporter the results are available only in the [`Report`] returned by
    /// [`DocAssert::assert`], once the whole suite has been executed. Use
    /// [`StdoutReporter`] to print the results while they are produced.
    ///
    /// # Examples
    ///
    /// ```
    /// # #![allow(unused_mut)]
    /// use doc_assert::DocAssert;
    /// use doc_assert::StdoutReporter;
    ///
    /// let mut doc_assert = DocAssert::new().with_reporter(StdoutReporter::new());
    /// ```
    pub fn with_reporter(mut self, reporter: impl Reporter + 'a) -> Self {
        self.reporter = Some(Box::new(reporter));
        self
    }

    /// Execute the assertions
    ///
    /// The assertions will be executed and a report will be returned
    ///
    /// # Examples
    ///
    /// ```
    /// # #![allow(unused_mut)]
    /// use doc_assert::DocAssert;
    /// async fn test() {
    ///     let mut doc_assert = DocAssert::new()
    ///         .with_url("http://localhost:8080")
    ///         .with_doc_path("path/to/README.md");
    ///     match doc_assert.assert().await {
    ///         Ok(report) => {
    ///             // handle success
    ///         }
    ///         Err(err) => {
    ///             // handle error
    ///         }
    ///     };
    /// }
    /// ```
    pub async fn assert(mut self) -> Result<Report, AssertionError> {
        let url = self.url.take().expect("URL is required");
        let mut reporter = self
            .reporter
            .take()
            .unwrap_or_else(|| Box::new(NoopReporter));

        // every documentation file is parsed upfront so that a parsing error is reported
        // before any request is made and the number of test cases is known in advance
        let mut test_cases = vec![];
        for doc_path in &self.doc_paths {
            let parsed = parser::parse(doc_path.to_string())
                .map_err(|e| AssertionError::ParsingError(e.clone()))?;
            for tc in parsed {
                let id = TestCaseId {
                    http_method: tc.request.http_method.to_string(),
                    uri: tc.request.uri.clone(),
                    doc_path: doc_path.to_string(),
                    line_number: tc.request.line_number,
                };
                test_cases.push((id, tc));
            }
        }

        let total_count = test_cases.len();
        let mut failed_count = 0;
        let mut summary = String::new();
        let mut failures = String::new();

        reporter.suite_started(total_count);

        for (id, tc) in test_cases {
            reporter.test_case_started(&id);
            let result = executor::execute(url, tc, &mut self.variables).await;
            match &result {
                Ok(_) => summary.push_str(format!("{} ✅\n", id).as_str()),
                Err(err) => {
                    summary.push_str(format!("{} ❌\n", id).as_str());
                    failures.push_str(format!("-------------\n{}: {}\n", id, err).as_str());
                    failed_count += 1;
                }
            }
            reporter.test_case_finished(&id, &result);
        }

        let report = Report {
            total_count,
            failed_count,
            summary,
            failures: (failed_count > 0).then_some(failures),
        };
        reporter.suite_finished(&report);

        if failed_count == 0 {
            Ok(report)
        } else {
            Err(AssertionError::TestSuiteError(report))
        }
    }
}

impl<'a> Default for DocAssert<'a> {
    fn default() -> Self {
        Self::new()
    }
}

/// Report of the assertions
///
/// The report contains the total number of tests, the number of failed tests,
/// a summary of passed and failed tests, and detailed information about
/// the failed assertions.
///
/// # Examples
///
/// ```
/// # #![allow(unused_mut)]
/// use doc_assert::DocAssert;
/// use doc_assert::Variables;
///
/// async fn test() {
///     let mut doc_assert = DocAssert::new()
///         .with_url("http://localhost:8080")
///         .with_doc_path("path/to/README.md");
///     match doc_assert.assert().await {
///         Ok(report) => {
///             println!("{}", report);
///         }
///         Err(err) => {
///             // handle error
///         }
///     };
/// }
pub struct Report {
    /// Total number of tests
    total_count: usize,
    /// Number of failed tests
    failed_count: usize,
    /// Summary of passed and failed tests
    summary: String,
    /// Detailed information about the failed assertions
    failures: Option<String>,
}

impl Display for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.failures {
            Some(failures) => write!(
                f,
                "{} tests\n{}\nfailures:\n{}\ntest result: FAILED. {} passed; {} failed",
                self.total_count,
                self.summary,
                failures,
                self.total_count - self.failed_count,
                self.failed_count
            ),
            None => write!(
                f,
                "{} tests\n{}\ntest result: PASSED. {} passed; 0 failed",
                self.total_count, self.summary, self.total_count
            ),
        }
    }
}

impl Report {
    /// Total number of executed test cases
    pub fn total_count(&self) -> usize {
        self.total_count
    }

    /// Number of failed test cases
    pub fn failed_count(&self) -> usize {
        self.failed_count
    }

    /// Number of passed test cases
    pub fn passed_count(&self) -> usize {
        self.total_count - self.failed_count
    }

    /// Summary of passed and failed test cases
    pub fn summary(&self) -> &str {
        &self.summary
    }

    /// Detailed information about the failed assertions, `None` if all the test cases passed
    pub fn failures(&self) -> Option<&str> {
        self.failures.as_deref()
    }
}

/// Identifies a single test case defined in the documentation.
///
/// It is displayed the same way it appears in the [`Report`] summary,
/// for example `GET /blog (README.md:12)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestCaseId {
    /// HTTP method of the request
    pub http_method: String,
    /// URI the request is sent to
    pub uri: String,
    /// Path to the documentation file the test case is defined in
    pub doc_path: String,
    /// Line number the request is defined at
    pub line_number: usize,
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

/// Receives the progress of a run while the test cases are being executed.
///
/// A reporter is registered with [`DocAssert::with_reporter`] and makes the results
/// available as soon as they are produced, instead of waiting for the whole suite to
/// finish. Every method has an empty default implementation, so only the events of
/// interest need to be implemented.
///
/// See [`StdoutReporter`] for a ready to use implementation printing the progress
/// to the standard output.
///
/// # Examples
///
/// ```
/// use doc_assert::{Reporter, TestCaseId};
///
/// #[derive(Default)]
/// struct FailedTestCases(Vec<String>);
///
/// impl Reporter for FailedTestCases {
///     fn test_case_finished(&mut self, id: &TestCaseId, result: &Result<(), String>) {
///         if result.is_err() {
///             self.0.push(id.to_string());
///         }
///     }
/// }
/// ```
pub trait Reporter: Send {
    /// Called once, before the first test case is executed.
    ///
    /// At this point every documentation file has been parsed successfully,
    /// so `total_count` is the final number of test cases to be executed.
    fn suite_started(&mut self, total_count: usize) {
        let _ = total_count;
    }

    /// Called just before the request of a test case is sent.
    fn test_case_started(&mut self, id: &TestCaseId) {
        let _ = id;
    }

    /// Called as soon as a test case has been executed.
    fn test_case_finished(&mut self, id: &TestCaseId, result: &Result<(), String>) {
        let _ = (id, result);
    }

    /// Called once, after the last test case has been executed.
    fn suite_finished(&mut self, report: &Report) {
        let _ = report;
    }
}

/// [`Reporter`] printing the progress of a run to the standard output.
///
/// Every test case is printed as soon as it has been executed, the details of the
/// failures and the final result follow once the whole suite is done:
///
/// ```text
/// 2 tests
/// GET /blog (README.md:12) ✅
/// POST /blog (README.md:30) ❌
///
/// failures:
/// -------------
/// POST /blog (README.md:30): expected response code 201, got 500
///
/// test result: FAILED. 1 passed; 1 failed
/// ```
///
/// # Examples
///
/// ```
/// # #![allow(unused_mut)]
/// use doc_assert::DocAssert;
/// use doc_assert::StdoutReporter;
///
/// let mut doc_assert = DocAssert::new().with_reporter(StdoutReporter::new());
/// ```
#[derive(Debug, Default)]
pub struct StdoutReporter;

impl StdoutReporter {
    /// Constructs a new `StdoutReporter`.
    pub fn new() -> Self {
        Self
    }
}

impl Reporter for StdoutReporter {
    fn suite_started(&mut self, total_count: usize) {
        println!("{} tests", total_count);
    }

    fn test_case_started(&mut self, id: &TestCaseId) {
        // the line is completed by `test_case_finished`, flushing it makes the test case
        // currently being executed visible while the request is in flight
        print!("{} ", id);
        let _ = std::io::stdout().flush();
    }

    fn test_case_finished(&mut self, _id: &TestCaseId, result: &Result<(), String>) {
        match result {
            Ok(_) => println!("✅"),
            Err(_) => println!("❌"),
        }
    }

    fn suite_finished(&mut self, report: &Report) {
        match report.failures() {
            Some(failures) => println!(
                "\nfailures:\n{}\ntest result: FAILED. {} passed; {} failed",
                failures,
                report.passed_count(),
                report.failed_count()
            ),
            None => println!(
                "\ntest result: PASSED. {} passed; 0 failed",
                report.total_count()
            ),
        }
    }
}

/// [`Reporter`] used when none was registered.
struct NoopReporter;

impl Reporter for NoopReporter {}

/// Error type for DocAssert run
pub enum AssertionError {
    /// Error parsing the documentation file
    ParsingError(String),
    /// Error executing tests
    TestSuiteError(Report),
}

/// Variables to be used in the request and response bodies.
///
/// The variables are used to replace placeholders in the request
/// and response bodies in case some values need to be shared between requests and responses.
///
/// # Examples
///
/// Variables can be passed one by one with specified type:
///
/// ```
/// # use doc_assert::Variables;
/// # use serde_json::Value;
/// let mut variables = Variables::new();
/// variables.insert_string("name".to_string(), "John".to_string());
/// variables.insert_int("age".to_string(), 30);
/// ```
///
/// A `Value` can be passed directly:
///
/// ```
/// # use doc_assert::Variables;
/// # use serde_json::Value;
/// let mut variables = Variables::new();
/// variables.insert_value("name".to_string(), Value::String("John".to_string()));
/// variables.insert_value("age".to_string(), Value::Number(serde_json::Number::from(30)));
/// ```
///
/// Alternatively, they can be passed as a JSON object:
///
/// ```
/// # use doc_assert::Variables;
/// # use serde_json::Value;
/// let json = r#"{"name": "John", "age": 30}"#;
/// let variables = Variables::from_json(&serde_json::from_str(json).unwrap()).unwrap();
/// ```
///
#[derive(Debug, Clone, Default)]
pub struct Variables {
    map: HashMap<String, Value>,
}

impl Variables {
    /// Constructs a new `Variables`.
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
    /// # use serde_json::Value;
    /// let json = r#"{"name": "John", "age": 30}"#;
    /// let variables = Variables::from_json(&serde_json::from_str(json).unwrap()).unwrap();
    /// ```
    pub fn from_json(json: &Value) -> Result<Self, String> {
        let mut map = HashMap::new();

        if let Value::Object(obj) = json {
            for (key, value) in obj {
                map.insert(key.clone(), value.clone());
            }
        } else {
            return Err("variables must be an object".to_string());
        }

        Ok(Self { map })
    }

    /// Inserts a `Value` into the `Variables`.
    ///
    /// This can be useful when more complex types are needed.
    /// Since `Variables` is a wrapper around `HashMap` if you insert duplicate
    /// keys the value will be overwritten.
    ///
    /// # Examples
    ///
    /// ```
    /// # use doc_assert::Variables;
    /// # use serde_json::Value;
    /// let mut variables = Variables::new();
    /// variables.insert_value("name".to_string(), Value::String("John".to_string()));
    /// ```
    pub fn insert_value(&mut self, name: String, value: Value) {
        self.map.insert(name, value);
    }

    /// Inserts a `String` into the `Variables`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use doc_assert::Variables;
    /// let mut variables = Variables::new();
    /// variables.insert_string("name".to_string(), "John".to_string());
    /// ```
    pub fn insert_string(&mut self, name: String, value: String) {
        self.map.insert(name, Value::String(value));
    }

    /// Inserts an `i64` into the `Variables`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use doc_assert::Variables;
    /// let mut variables = Variables::new();
    /// variables.insert_int("age".to_string(), 30);
    /// ```
    pub fn insert_int(&mut self, name: String, value: i64) {
        self.map
            .insert(name, Value::Number(serde_json::Number::from(value)));
    }

    /// Inserts an `f64` into the `Variables`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use doc_assert::Variables;
    /// let mut variables = Variables::new();
    /// variables.insert_float("age".to_string(), 30.0);
    /// ```
    pub fn insert_float(&mut self, name: String, value: f64) {
        self.map.insert(
            name,
            Value::Number(serde_json::Number::from_f64(value).unwrap()),
        );
    }

    /// Inserts a `bool` into the `Variables`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use doc_assert::Variables;
    /// let mut variables = Variables::new();
    /// variables.insert_bool("is_adult".to_string(), true);
    /// ```
    pub fn insert_bool(&mut self, name: String, value: bool) {
        self.map.insert(name, Value::Bool(value));
    }

    /// Inserts a `null` into the `Variables`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use doc_assert::Variables;
    /// let mut variables = Variables::new();
    /// variables.insert_null("name".to_string());
    /// ```
    pub fn insert_null(&mut self, name: String) {
        self.map.insert(name, Value::Null);
    }

    pub(crate) fn obtain_from_response(
        &mut self,
        response: &Value,
        variable_templates: &HashMap<String, Path>,
    ) -> Result<(), String> {
        for (name, path) in variable_templates {
            let value = extract_value(path, response).ok_or_else(|| {
                format!("variable template {} not found in the response body", name)
            })?;

            self.map.insert(name.clone(), value);
        }

        Ok(())
    }

    fn replace_placeholders(&self, input: &mut String, trim_quotes: bool) -> Result<(), String> {
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
            return Err(format!("unresolved variable placeholders in {}", input));
        }

        Ok(())
    }

    pub(crate) fn replace_request_placeholders(&self, input: &mut Request) -> Result<(), String> {
        self.replace_placeholders(&mut input.uri, true)?;

        if let Some(body) = &mut input.body {
            self.replace_placeholders(body, false)?;
        }

        for (_, value) in &mut input.headers.iter_mut() {
            self.replace_placeholders(value, true)?;
        }

        Ok(())
    }

    pub(crate) fn replace_response_placeholders(&self, input: &mut Response) -> Result<(), String> {
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
    use std::sync::{Arc, Mutex};

    use crate::{DocAssert, Report, Reporter, TestCaseId};

    /// [`Reporter`] recording the events in the order they were received.
    #[derive(Clone, Default)]
    struct RecordingReporter {
        events: Arc<Mutex<Vec<String>>>,
    }

    impl RecordingReporter {
        fn record(&self, event: String) {
            self.events.lock().unwrap().push(event);
        }

        fn events(&self) -> Vec<String> {
            self.events.lock().unwrap().clone()
        }
    }

    impl Reporter for RecordingReporter {
        fn suite_started(&mut self, total_count: usize) {
            self.record(format!("suite started: {}", total_count));
        }

        fn test_case_started(&mut self, id: &TestCaseId) {
            self.record(format!(
                "started: {} {}:{}",
                id.http_method, id.uri, id.line_number
            ));
        }

        fn test_case_finished(&mut self, id: &TestCaseId, result: &Result<(), String>) {
            let outcome = if result.is_ok() { "passed" } else { "failed" };
            self.record(format!(
                "finished: {} {}:{} {}",
                id.http_method, id.uri, id.line_number, outcome
            ));
        }

        fn suite_finished(&mut self, report: &Report) {
            self.record(format!(
                "suite finished: {} passed, {} failed",
                report.passed_count(),
                report.failed_count()
            ));
        }
    }

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

    async fn server_with_passing_and_failing_endpoint() -> mockito::ServerGuard {
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
    async fn test_every_test_case_is_reported_as_soon_as_it_is_executed() {
        let server = server_with_passing_and_failing_endpoint().await;
        let doc = TempDoc::new(DOC);
        let reporter = RecordingReporter::default();

        let result = DocAssert::new()
            .with_url(server.url().as_str())
            .with_doc_path(doc.path())
            .with_reporter(reporter.clone())
            .assert()
            .await;

        assert!(result.is_err());
        // each test case is reported before the next one is started, which is what makes
        // the results visible while the suite is still running
        assert_eq!(
            reporter.events(),
            vec![
                "suite started: 2",
                "started: GET /passing:1",
                "finished: GET /passing:1 passed",
                "started: GET /failing:7",
                "finished: GET /failing:7 failed",
                "suite finished: 1 passed, 1 failed",
            ]
        );
    }

    #[tokio::test]
    async fn test_report_is_still_returned_when_no_reporter_is_registered() {
        let server = server_with_passing_and_failing_endpoint().await;
        let doc = TempDoc::new(DOC);

        let result = DocAssert::new()
            .with_url(server.url().as_str())
            .with_doc_path(doc.path())
            .assert()
            .await;

        match result {
            Ok(_) => panic!("expected the suite to fail"),
            Err(crate::AssertionError::TestSuiteError(report)) => {
                assert_eq!(report.total_count(), 2);
                assert_eq!(report.passed_count(), 1);
                assert_eq!(report.failed_count(), 1);
                assert!(report.summary().contains("GET /passing"));
                assert!(report.failures().unwrap().contains("GET /failing"));
            }
            Err(crate::AssertionError::ParsingError(err)) => panic!("parsing error: {}", err),
        }
    }

    #[tokio::test]
    async fn test_parsing_error_is_reported_before_any_test_case_is_executed() {
        let server = server_with_passing_and_failing_endpoint().await;
        let doc = TempDoc::new(DOC);
        let reporter = RecordingReporter::default();

        let result = DocAssert::new()
            .with_url(server.url().as_str())
            .with_doc_path(doc.path())
            .with_doc_path("this/file/does/not/exist.md")
            .with_reporter(reporter.clone())
            .assert()
            .await;

        assert!(matches!(
            result,
            Err(crate::AssertionError::ParsingError(_))
        ));
        assert!(reporter.events().is_empty());
    }
}
