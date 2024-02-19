use std::{collections::HashMap, vec};

use serde_json::Value;
use variables::Variables;

mod domain;
mod executor;
mod json_diff;
mod parser;
pub mod variables;

pub struct DocAssert<'a> {
    url: Option<&'a str>,
    doc_paths: Vec<&'a str>,
    pub variables: Variables,
}

/// Builder for the assertions
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
    pub async fn assert(mut self) -> Result<(), Vec<String>> {
        let url = self.url.take().expect("URL is required");
        let mut errors: Vec<String> = vec![];

        for doc_path in self.doc_paths {
            let test_cases = parser::parse(doc_path.to_string())
                .map_err(|e| vec![format!("{}: {}", doc_path, e)])?;
            for tc in test_cases {
                if let Err(e) = executor::execute(url, tc, &mut self.variables).await {
                    errors.push(format!("{}: {}", doc_path, e));
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl<'a> Default for DocAssert<'a> {
    fn default() -> Self {
        Self::new()
    }
}
