use std::fmt::Display;
use std::vec;

use variables::Variables;

mod domain;
mod executor;
mod json_diff;
mod parser;
pub mod variables;

/// Builder for the assertions
pub struct DocAssert<'a> {
    url: Option<&'a str>,
    doc_paths: Vec<&'a str>,
    pub variables: Variables,
}

impl<'a> DocAssert<'a> {
    pub fn new() -> Self {
        Self {
            url: None,
            doc_paths: vec![],
            variables: Variables::new(),
        }
    }

    /// Set the base URL to test against
    pub fn with_url(mut self, url: &'a str) -> Self {
        self.url = Some(url);
        self
    }

    /// Set the path to the documentation file
    pub fn with_doc_path(mut self, doc_path: &'a str) -> Self {
        self.doc_paths.push(doc_path);
        self
    }

    /// Set the variables to be used in the assertions
    /// The variables will be used to replace the placeholders in the documentation
    pub fn with_variables(mut self, variables: Variables) -> Self {
        self.variables = variables;
        self
    }

    /// Execute the assertions
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
