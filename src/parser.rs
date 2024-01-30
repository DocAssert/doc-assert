use std::collections::HashMap;
use std::fs;
use std::str::{FromStr, Lines};

use crate::domain::{HttpMethod, Request, Response, TestCase};

const DOC_ASSERT_REQUEST: &str = "```docassertrequest";
const DOC_ASSERT_RESPONSE: &str = "```docassertresponse";
const IGNORE_PREFIX: &str = "[ignore]";

fn parse(path: String) -> Result<Vec<TestCase>, String> {
    let (mut requests, mut responses) = (vec![], vec![]);
    let binding = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut lines = binding.lines();
    while let Some(line) = lines.next() {
        // todo refactor to push req/resp as func?
        if line.starts_with(DOC_ASSERT_REQUEST) {
            requests.push(get_request(get_code(&mut lines))?);
        }
        if line.starts_with(DOC_ASSERT_RESPONSE) {
            responses.push(get_response(get_code(&mut lines))?);
        }
        if line.starts_with(IGNORE_PREFIX) {
            if responses.is_empty() || responses.len() != requests.len() {
                return Err(format!("Misplaced ignore {}", line));
            }
            let l = responses.len();
            responses[l - 1].ignore_paths.push(get_ignore_path(line)?);
        }
    }
    if requests.len() != responses.len() {
        return Err(
            format!(
                "There is {} requests and {} responses but you need equal number of both",
                requests.len(),
                responses.len())
        );
    }
    let test_cases = requests.iter()
        .zip(responses.iter())
        .map(|(req, resp)| {
            TestCase { request: req.clone(), response: resp.clone() }
        })
        .collect::<Vec<TestCase>>();
    Ok(test_cases)
}

fn get_code(lines: &mut Lines) -> String {
    let mut buff = String::new();
    while let Some(line) = lines.next() {
        if line.starts_with("```") {
            break;
        }
        buff.push_str(format!("{}\n", line).as_str());
    }
    return buff;
}

fn get_ignore_path(s: &str) -> Result<String, String> {
    let no_whitespace = s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    let mut path = no_whitespace.split(":#").skip(1).collect::<String>();
    path.remove(0);
    path.pop();
    // TODO add validation of ignore path
    Ok(path)
}

fn get_request(code: String) -> Result<Request, String> {
    let mut lines = code.lines();

    // Parse HTTP method and URL
    let parts = lines.next().ok_or("Request line not found")
        .map(|line| line.split_whitespace().collect::<Vec<&str>>())?;
    if parts.len() != 2 {
        return Err(format!("Invalid request line {}", parts.join(" ")));
    }

    let (headers, body) = get_headers_and_body(lines)?;

    Ok(Request {
        http_method: HttpMethod::from_str(parts[0])?,
        url: parts[1].to_string(),
        headers,
        body,
    })
}

fn get_response(code: String) -> Result<Response, String> {
    let mut lines = code.lines();

    // Parse HTTP method and URL
    let parts = lines.next().ok_or("Response code line not found")
        .map(|line| line.split_whitespace().collect::<Vec<&str>>())?;
    if parts.len() != 2 {
        return Err(format!("Invalid response code line {}", parts.join(" ")));
    }
    let http_code = parts[1].parse::<i32>().map_err(|_| "Invalid HTTP code".to_string())?;

    let (headers, body) = get_headers_and_body(lines)?;

    Ok(Response {
        code: http_code,
        headers,
        ignore_paths: vec![],
        body,
    })
}

fn get_headers_and_body(mut lines: Lines) -> Result<(HashMap<String, String>, Option<String>), String> {
    let mut headers = HashMap::new();
    let mut body = String::new();
    for line in &mut lines {
        if body.is_empty() && line.contains(":") && !line.contains("{") {
            let header_parts = line.split(":").map(|s| s.trim()).collect::<Vec<&str>>();
            if header_parts.len() != 2 {
                return Err(format!("Invalid header line {}", line));
            }
            headers.insert(header_parts[0].to_string(), header_parts[1].to_string());
            continue;
        }
        body.push_str(line);
    }
    let body = if body.is_empty() {
        None
    } else {
        Some(body.chars().filter(|c| !c.is_whitespace()).collect::<String>())
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
        assert_eq!(test_cases[0].request.http_method, crate::domain::HttpMethod::POST);
        assert_eq!(test_cases[0].request.url, "/api/user");
        assert_eq!(test_cases[0].request.headers.get("Content-Type").unwrap(), "application/json");
        assert_eq!(test_cases[0].request.body.as_ref().unwrap(), "{\"name\":\"test\"}");
        // response
        assert_eq!(test_cases[0].response.code, 201);
        assert_eq!(test_cases[0].response.headers.get("Content-Type").unwrap(), "application/json");
        assert_eq!(test_cases[0].response.body.as_ref().unwrap(), "{\"id\":1,\"name\":\"test\"}");
        assert_eq!(test_cases[0].response.ignore_paths[0], "id".to_string());
    }
}