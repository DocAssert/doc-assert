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
use std::fmt::Display;
use std::vec;
use std::{collections::HashMap, iter::zip};
use tokio::{sync::mpsc, sync::mpsc::Receiver};

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
        let mut total_count = 0;
        let mut failed_count = 0;

        let mut summary = String::new();
        let mut failures = String::new();

        for doc_path in self.doc_paths {
            let test_cases = parser::parse(doc_path.to_string())
                .map_err(|e| AssertionError::ParsingError(e.clone()))?;
            for tc in test_cases {
                total_count += 1;
                let id = format!(
                    "{} {} ({}:{})",
                    tc.request.http_method, tc.request.uri, doc_path, tc.request.line_number
                );
                match executor::execute(url, tc, &mut self.variables).await {
                    Ok(_) => summary.push_str(format!("{} ✅\n", id).as_str()),
                    Err(err) => {
                        summary.push_str(format!("{} ❌\n", id).as_str());
                        failures.push_str(format!("-------------\n{}: {}\n", id, err).as_str());
                        failed_count += 1;
                    }
                }
            }
        }

        if failed_count == 0 {
            Ok(Report {
                total_count,
                failed_count,
                summary,
                failures: None,
            })
        } else {
            Err(AssertionError::TestSuiteError(Report {
                total_count,
                failed_count,
                summary,
                failures: Some(failures),
            }))
        }
    }

    pub async fn assert_stream(mut self) -> Result<AsyncReport, AssertionError> {
        let url = self.url.take().expect("URL is required").to_string();

        let test_cases_sets = self
            .doc_paths
            .iter()
            .map(|doc_path| {
                parser::parse(doc_path.to_string())
                    .map_err(|e| AssertionError::ParsingError(e.clone()))
            })
            .collect::<Result<Vec<Vec<domain::TestCase>>, AssertionError>>()?;

        let test_count = test_cases_sets.iter().fold(0, |acc, x| acc + x.len());

        let doc_paths = self
            .doc_paths
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<String>>();

        let mut variables = self.variables;

        let (summary_tx, summary_rx) = mpsc::channel::<String>(test_count);
        let (failures_tx, failures_rx) = mpsc::channel::<String>(test_count);

        tokio::spawn(async move {
            for (test_cases, doc_path) in zip(test_cases_sets, doc_paths) {
                for tc in test_cases {
                    let id = format!(
                        "{} {} ({}:{})",
                        tc.request.http_method, tc.request.uri, doc_path, tc.request.line_number
                    );

                    match executor::execute(&url, tc, &mut variables).await {
                        Ok(_) => summary_tx.send(format!("{} ✅", id)).await.unwrap(),
                        Err(err) => {
                            summary_tx.send(format!("{} ❌", id)).await.unwrap();
                            failures_tx
                                .send(format!("-------------\n{}: {}", id, err))
                                .await
                                .unwrap();
                        }
                    }
                }
            }
        });

        Ok(AsyncReport {
            total_count: test_count,
            summary: summary_rx,
            failures: failures_rx,
            passed: None,
        })
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

pub struct AsyncReport {
    /// Total number of tests
    pub total_count: usize,
    /// Stream of summary messages
    pub summary: Receiver<String>,
    /// Stream of detailed information about the failed assertions
    pub failures: Receiver<String>,
    /// Determines if the test suite passed, only if all tests have been executed
    pub passed: Option<bool>,
}

impl AsyncReport {
    pub async fn process_and_log(&mut self) {
        println!("{} tests", self.total_count);

        while let Some(message) = self.summary.recv().await {
            println!("{}", message);
        }

        let mut failed = 0;
        while let Some(message) = self.failures.recv().await {
            if failed == 0 {
                println!("\nfailures:");
            }

            println!("{}", message);
            failed += 1;
        }

        if failed > 0 {
            println!(
                "\ntest result: FAILED. {} passed; {} failed",
                self.total_count - failed,
                failed
            );
        } else {
            println!(
                "\ntest result: PASSED. {} passed; 0 failed",
                self.total_count
            );
        }

        self.passed = Some(failed == 0);
    }
}

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
#[derive(Debug, Default)]
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
