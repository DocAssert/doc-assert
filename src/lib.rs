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

use std::fmt::Display;
use std::vec;

use variables::Variables;

mod domain;
mod executor;
mod json_diff;
mod parser;
pub mod variables;

/// Builder for the assertions.
///
/// The builder is used to configure the assertions.
///
/// # Examples
///
/// ```
/// # #![allow(unused_mut)]
/// use doc_assert::DocAssert;
/// use doc_assert::variables::Variables;
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
    /// use doc_assert::variables::Variables;
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
/// use doc_assert::variables::Variables;
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
    total_count: u8,
    /// Number of failed tests
    failed_count: u8,
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
