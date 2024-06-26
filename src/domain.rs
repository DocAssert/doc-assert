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

use std::collections::HashMap;
use std::fmt::Display;
use std::str::FromStr;

use crate::json_diff::path::Path;

#[derive(Debug, Clone)]
pub(crate) struct TestCase {
    pub request: Request,
    pub response: Response,
}

// TODO consider using client's enums?
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

impl Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpMethod::Get => write!(f, "GET"),
            HttpMethod::Post => write!(f, "POST"),
            HttpMethod::Put => write!(f, "PUT"),
            HttpMethod::Delete => write!(f, "DELETE"),
        }
    }
}

impl FromStr for HttpMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "GET" => Ok(HttpMethod::Get),
            "POST" => Ok(HttpMethod::Post),
            "PUT" => Ok(HttpMethod::Put),
            "DELETE" => Ok(HttpMethod::Delete),
            _ => Err(format!("{} is not a valid http method", s)),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Request {
    // TODO add cert
    pub http_method: HttpMethod,
    pub headers: HashMap<String, String>,
    pub uri: String,
    pub body: Option<String>,
    pub line_number: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RetryPolicy {
    pub max_retries: u64,
    pub delay: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        RetryPolicy {
            max_retries: 1,
            delay: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Response {
    pub code: u16,
    pub headers: HashMap<String, String>,
    pub ignore_paths: Vec<String>,
    pub ignore_orders: Vec<String>,
    pub body: Option<String>,
    pub line_number: usize,
    pub variables: HashMap<String, Path>,
    pub retries: RetryPolicy,
}
