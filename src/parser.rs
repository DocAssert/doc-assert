use std::collections::HashMap;
use std::fs;
use std::str::{FromStr, Lines};

use markdown::mdast::{Code, Node};
use markdown::to_mdast;

use crate::domain::{HttpMethod, Request, Response, TestCase};

const DOC_ASSERT_REQUEST: &str = "docassertrequest";
const DOC_ASSERT_RESPONSE: &str = "docassertresponse";

fn parse(path: String) -> Result<Vec<TestCase>, String> {
    fs::read_to_string(path)
        .map_err(|e| e.to_string())
        .and_then(|content| to_mdast(content.as_str(), &markdown::ParseOptions::default()))
        .map(|ast| get_code_blocks(ast))
        .and_then(|codes| {
            let (reqs, resps) = codes.iter()
                .fold((vec![], vec![]), |(mut reqs, mut resps), code| {
                    if code.lang == Some(DOC_ASSERT_REQUEST.to_string()) {
                        reqs.push(get_request(code));
                    } else if code.lang == Some(DOC_ASSERT_RESPONSE.to_string()) {
                        resps.push(get_response(code));
                    }
                    (reqs, resps)
                });
            reqs.into_iter().collect::<Result<Vec<Request>, String>>()
                .and_then(|reqs| {
                    resps.into_iter()
                        .collect::<Result<Vec<Response>, String>>()
                        .map(|resps| (reqs, resps))
                })
        })
        .map(|(reqs, resps)| {
            reqs.iter().zip(resps.iter()).map(|(req, resp)| {
                TestCase {
                    request: req.clone(),
                    response: resp.clone(),
                }
            }).collect::<Vec<TestCase>>()
        })
}

fn get_request(code: &Code) -> Result<Request, String> {
    let mut lines = code.value.lines();

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

fn get_response(code: &Code) -> Result<Response, String> {
    let mut lines = code.value.lines();

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
        ignore: vec![],
        body,
    })
}

fn get_code_blocks(node: Node) -> Vec<Code> {
    let mut code_blocks = vec![];
    match node {
        Node::Code(code) => {
            if let Some(lang) = &code.lang {
                if lang == DOC_ASSERT_REQUEST || lang == DOC_ASSERT_RESPONSE {
                    code_blocks.push(code);
                }
            }
        }
        _ => {
            node.children().iter().flat_map(|v| v.iter()).for_each(|child| {
                code_blocks.append(&mut get_code_blocks(child.clone()));
            });
        }
    }
    code_blocks
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
    }
}