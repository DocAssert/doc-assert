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
use std::fs;
use std::iter::Enumerate;
use std::str::{FromStr, Lines};

use regex::Regex;

use crate::domain::{HttpMethod, Request, Response, RetryPolicy, TestCase};
use crate::json_diff::path::{JSONPath, Path, JSON_PATH_REGEX};

const DOC_ASSERT_REQUEST: &str = "```docassertrequest";
const DOC_ASSERT_RESPONSE: &str = "```docassertresponse";
const IGNORE_PREFIX: &str = "[ignore]";
const IGNORE_ORDER_PREFIX: &str = "[ignore-order]";
const VARIABLE_PREFIX: &str = "[let ";
const RETRY_PREFIX: &str = "[retry]";

pub(crate) fn parse(path: String) -> Result<Vec<TestCase>, String> {
    let (mut requests, mut responses) = (vec![], vec![]);
    let binding = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut lines = binding.lines().enumerate();
    while let Some((mut line_no, line)) = lines.next() {
        line_no += 1;
        if line.starts_with(DOC_ASSERT_REQUEST) {
            let request = get_request(line_no, get_code(&mut lines)).map_err(|err| {
                format!(
                    "parsing error of a request code block starting at line {}: {}",
                    line_no, err
                )
            })?;
            requests.push(request);
        }

        if line.starts_with(DOC_ASSERT_RESPONSE) {
            let response = get_response(line_no, get_code(&mut lines)).map_err(|err| {
                format!(
                    "parsing error of a response code block starting at line {}: {}",
                    line_no, err
                )
            })?;
            responses.push(response);
        }

        if line.starts_with(IGNORE_PREFIX) {
            if responses.is_empty() || responses.len() != requests.len() {
                return Err(format!("misplaced ignore at line {}: {}", line_no, line));
            }
            let l = responses.len();
            responses[l - 1].ignore_paths.push(get_ignore_path(line)?);
        }

        if line.starts_with(IGNORE_ORDER_PREFIX) {
            if responses.is_empty() || responses.len() != requests.len() {
                return Err(format!(
                    "misplaced ignore-order at line {}: {}",
                    line_no, line
                ));
            }
            let l = responses.len();
            responses[l - 1].ignore_orders.push(get_ignore_path(line)?);
        }

        if line.starts_with(VARIABLE_PREFIX) {
            if responses.is_empty() || responses.len() != requests.len() {
                return Err(format!("misplaced variable at line {}: {}", line_no, line));
            }
            let (name, path) = get_variable_template(line)?;

            let l = responses.len();
            responses[l - 1].variables.insert(name, path);
        }

        if line.starts_with(RETRY_PREFIX) {
            if responses.is_empty() || responses.len() != requests.len() {
                return Err(format!("misplaced retry at line {}: {}", line_no, line));
            }
            let retry_policy = get_retry_policy(line)?;

            let l = responses.len();
            responses[l - 1].retries = retry_policy;
        }
    }
    if requests.len() != responses.len() {
        return Err(format!(
            "there is {} requests and {} responses but you need equal number of both",
            requests.len(),
            responses.len()
        ));
    }

    let test_cases = requests
        .iter()
        .zip(responses.iter())
        .map(|(req, resp)| TestCase {
            request: req.clone(),
            response: resp.clone(),
        })
        .collect::<Vec<TestCase>>();

    Ok(test_cases)
}

fn get_code(lines: &mut Enumerate<Lines>) -> String {
    let mut buff = String::new();
    while let Some(line) = lines.next() {
        if line.1.starts_with("```") {
            break;
        }
        buff.push_str(format!("{}\n", line.1).as_str());
    }
    buff
}

fn get_ignore_path(line: &str) -> Result<String, String> {
    let no_whitespace = line
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();
    let mut path = no_whitespace.split(":#").skip(1).collect::<String>();
    path.remove(0);
    path.pop();

    if let Err(e) = path.jsonpath() {
        return Err(format!("invalid ignore path {}", e));
    }

    Ok(path)
}

fn get_retry_policy(line: &str) -> Result<RetryPolicy, String> {
    let re = Regex::new(r"^\[retry\]:\s#\s\((?<max_retries>\d+),\s*(?<delay>\d+)\)").unwrap();

    let caps = re
        .captures(line)
        .ok_or(format!("invalid retry properties: {}", line))?;

    let max_retries = caps
        .name("max_retries")
        .ok_or(format!("invalid retry properties: {}", line))?
        .as_str()
        .parse::<u64>()
        .map_err(|e| format!("invalid max_retries: {}", e))?;

    let delay = caps
        .name("delay")
        .ok_or(format!("invalid retry properties: {}", line))?
        .as_str()
        .parse::<u64>()
        .map_err(|e| format!("invalid delay: {}", e))?;

    Ok(RetryPolicy { max_retries, delay })
}

fn get_variable_template(line: &str) -> Result<(String, Path), String> {
    let re =
        Regex::new(format!(r"^\[let\s(?<var>\w+)\]:\s#\s\((?<value>{JSON_PATH_REGEX})\)").as_str())
            .unwrap();

    let caps = re
        .captures(line)
        .ok_or(format!("invalid variable template: {}", line))?;

    let name = caps
        .name("var")
        .ok_or(format!("invalid variable template: {}", line))?;

    let value = caps
        .name("value")
        .ok_or(format!("invalid variable template: {}", line))?;

    match value.as_str().jsonpath() {
        Ok(p) => Ok((name.as_str().to_owned(), p)),
        Err(e) => Err(format!("invalid variable template: {}: {}", line, e)),
    }
}

fn get_request(code_block_line_no: usize, code: String) -> Result<Request, String> {
    let mut lines = code.lines();

    // Parse HTTP method and URL
    let parts = lines
        .next()
        .ok_or("expected HTTP method and URI".to_string())
        .map(|line| line.split_whitespace().collect::<Vec<&str>>())?;
    if parts.len() != 2 {
        return Err(format!(
            "invalid HTTP method or URI: \"{}\"",
            parts.join(" ")
        ));
    }

    let (headers, body) = get_headers_and_body(lines)?;

    Ok(Request {
        http_method: HttpMethod::from_str(parts[0])?,
        uri: parts[1].to_string(),
        headers,
        body,
        line_number: code_block_line_no,
    })
}

fn get_response(code_block_line_no: usize, code: String) -> Result<Response, String> {
    let mut lines = code.lines();

    // Parse HTTP method and URL
    let parts = lines
        .next()
        .ok_or("response code line not found".to_string())
        .map(|line| line.split_whitespace().collect::<Vec<&str>>())?;
    if parts.len() != 2 {
        return Err(format!("invalid response code line {}", parts.join(" ")));
    }
    let http_code = parts[1]
        .parse::<u16>()
        .map_err(|err| format!("invalid HTTP code: {}", err))?;
    if !(100..=599).contains(&http_code) {
        return Err(format!("HTTP code {} outside of valid range", http_code));
    }

    let (headers, body) = get_headers_and_body(lines)?;

    Ok(Response {
        code: http_code,
        headers,
        ignore_paths: vec![],
        ignore_orders: vec![],
        body,
        line_number: code_block_line_no,
        variables: HashMap::new(),
        retries: RetryPolicy::default(),
    })
}

fn get_headers_and_body(
    mut lines: Lines,
) -> Result<(HashMap<String, String>, Option<String>), String> {
    let mut headers = HashMap::new();
    let mut body = String::new();
    for line in &mut lines {
        if body.is_empty() && line.contains(':') && !line.contains('{') {
            let header_parts = line.split(':').map(|s| s.trim()).collect::<Vec<&str>>();
            if header_parts.len() != 2 {
                return Err(format!("invalid header line {}", line));
            }
            headers.insert(header_parts[0].to_string(), header_parts[1].to_string());
            continue;
        }
        body.push_str(line.trim());
    }
    let body = if body.is_empty() { None } else { Some(body) };
    Ok((headers, body))
}

#[cfg(test)]
mod tests {
    use crate::{domain::RetryPolicy, parser::parse};

    #[test]
    fn test_parse() {
        let result = parse("tests/data/README.md".to_string());
        assert!(result.is_ok());
        let test_cases = result.unwrap();
        assert_eq!(test_cases.len(), 2);
        // request
        assert_eq!(
            test_cases[0].request.http_method,
            crate::domain::HttpMethod::Post
        );
        assert_eq!(test_cases[0].request.uri, "/api/user");
        assert_eq!(
            test_cases[0].request.headers.get("Content-Type").unwrap(),
            "application/json"
        );
        assert_eq!(
            test_cases[0].request.body.as_ref().unwrap(),
            "{\"name\": \"test\"}"
        );
        // response
        assert_eq!(test_cases[0].response.code, 201);
        assert_eq!(
            test_cases[0].response.headers.get("Content-Type").unwrap(),
            "application/json"
        );
        assert_eq!(
            test_cases[0].response.body.as_ref().unwrap(),
            "{\"id\": 1,\"name\": \"test\"}"
        );
        assert_eq!(test_cases[0].response.ignore_paths[0], "$.id".to_string());

        assert_eq!(
            test_cases[0]
                .response
                .variables
                .get("name")
                .unwrap()
                .to_string(),
            ".name".to_string()
        );

        assert_eq!(
            &test_cases[0].response.retries,
            &RetryPolicy {
                max_retries: 3,
                delay: 4500
            }
        )
    }
}
