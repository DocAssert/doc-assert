use std::collections::HashMap;
use std::fs;
use std::iter::Enumerate;
use std::str::{FromStr, Lines};

use crate::domain::{HttpMethod, Request, Response, TestCase};
use crate::json_diff::path::{JSONPath, Path, JSON_PATH_REGEX};
use regex::Regex;

const DOC_ASSERT_REQUEST: &str = "```docassertrequest";
const DOC_ASSERT_RESPONSE: &str = "```docassertresponse";
const IGNORE_PREFIX: &str = "[ignore]";
const VARIABLE_PREFIX: &str = "[let ";

pub(crate) fn parse<'a>(path: String) -> Result<Vec<TestCase>, String> {
    let (mut requests, mut responses) = (vec![], vec![]);
    let binding = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut lines = binding.lines().enumerate();
    while let Some((mut line_no, line)) = lines.next() {
        line_no += 1;
        if line.starts_with(DOC_ASSERT_REQUEST) {
            let request = get_request(line_no, get_code(&mut lines)).map_err(|err| {
                format!(
                    "parsing error of a request code block starting at line {}: {}",
                    line_no,
                    err.to_string()
                )
            })?;
            requests.push(request);
        }

        if line.starts_with(DOC_ASSERT_RESPONSE) {
            let response = get_response(line_no, get_code(&mut lines)).map_err(|err| {
                format!(
                    "parsing error of a response code block starting at line {}: {}",
                    line_no,
                    err.to_string()
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

        if line.starts_with(VARIABLE_PREFIX) {
            if responses.is_empty() || responses.len() != requests.len() {
                return Err(format!("misplaced variable at line {}: {}", line_no, line));
            }
            let (name, path) = get_variable_template(line)?;

            let l = responses.len();
            responses[l - 1].variables.insert(name, path);
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

fn get_ignore_path(s: &str) -> Result<String, String> {
    let no_whitespace = s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    let mut path = no_whitespace.split(":#").skip(1).collect::<String>();
    path.remove(0);
    path.pop();

    if let Err(e) = path.jsonpath() {
        return Err(format!("invalid ignore path {}", e));
    }

    Ok(path)
}

fn get_variable_template(s: &str) -> Result<(String, Path), String> {
    let re =
        Regex::new(format!(r"^\[let\s(?<var>\w+)\]:\s#\s(?<value>{JSON_PATH_REGEX})").as_str())
            .unwrap();

    let caps = re
        .captures(s)
        .ok_or(format!("invalid variable template: {}", s))?;

    let name = caps
        .name("var")
        .ok_or(format!("invalid variable template: {}", s))?;

    let value = caps
        .name("value")
        .ok_or(format!("invalid variable template: {}", s))?;

    match value.as_str().jsonpath() {
        Ok(p) => return Ok((name.as_str().to_owned(), p)),
        Err(e) => return Err(format!("invalid variable template: {}: {}", s, e)),
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

fn get_response<'a>(code_block_line_no: usize, code: String) -> Result<Response, String> {
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
        body,
        line_number: code_block_line_no,
        variables: HashMap::new(),
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
        body.push_str(line);
    }
    let body = if body.is_empty() {
        None
    } else {
        Some(
            body.chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>(),
        )
    };
    Ok((headers, body))
}

#[cfg(test)]
mod tests {
    use crate::parser::parse;

    #[test]
    fn test_parse() {
        let result = parse("README.md".to_string());
        assert!(result.is_ok());
        let test_cases = result.unwrap();
        assert_eq!(test_cases.len(), 1);
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
            "{\"name\":\"test\"}"
        );
        // response
        assert_eq!(test_cases[0].response.code, 201);
        assert_eq!(
            test_cases[0].response.headers.get("Content-Type").unwrap(),
            "application/json"
        );
        assert_eq!(
            test_cases[0].response.body.as_ref().unwrap(),
            "{\"id\":1,\"name\":\"test\"}"
        );
        assert_eq!(test_cases[0].response.ignore_paths[0], "$.id".to_string());
    }
}
