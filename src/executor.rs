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
use std::str::FromStr;
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Body, Client, Method, Response};

use crate::domain::{HttpMethod, Request, TestCase};
use crate::json_diff::path::Path;
use crate::json_diff::{diff, CompareMode, Config};
use crate::Variables;

pub(crate) async fn execute(
    base_url: &str,
    test_case: TestCase,
    variables: &mut Variables,
) -> Result<(), String> {
    let mut test_request = test_case.request;
    variables.replace_request_placeholders(&mut test_request)?;

    let test_request_line_number = test_request.line_number;
    let http_method = &test_request.http_method;
    let uri = &test_request.uri;

    let mut test_response = test_case.response;
    variables.replace_response_placeholders(&mut test_response)?;
    let test_response_line_number: usize = test_response.line_number;

    for i in 0..test_response.retries.max_retries {
        let response = get_response(base_url, &test_request).await.map_err(|err| {
            format!(
                "error executing request {} {} defined at line {}: {}",
                http_method, uri, test_request_line_number, err
            )
        });

        match response {
            Err(e) => {
                if i == test_response.retries.max_retries - 1 {
                    return Err(e);
                }
                tokio::time::sleep(Duration::from_millis(test_response.retries.delay)).await;
                continue;
            }
            Ok(response) => {
                let assert_response = assert_response(response, &test_response, variables)
                    .await
                    .map_err(|err| {
                        format!(
                            "error asserting response from {} {} defined at line {}: {}",
                            http_method, uri, test_response_line_number, err
                        )
                    });
                match assert_response {
                    Ok(_) => return Ok(()),
                    Err(e) => {
                        if i == test_response.retries.max_retries - 1 {
                            return Err(e);
                        }
                        tokio::time::sleep(Duration::from_millis(test_response.retries.delay))
                            .await;
                        continue;
                    }
                }
            }
        }
    }

    Err("internal error executing request".to_string())
}

async fn assert_response(
    response: Response,
    test_response: &crate::domain::Response,
    variables: &mut Variables,
) -> Result<(), String> {
    if test_response.code != response.status().as_u16() {
        return Err(format!(
            "expected response code {}, got {}",
            test_response.code,
            response.status().as_u16()
        ));
    }
    for (key, val) in test_response.headers.iter() {
        match response.headers().get(key.as_str()) {
            Some(test_val) => {
                if test_val != val.as_str() {
                    return Err(format!(
                        "expected header {} to be {}, got {}",
                        key,
                        val,
                        test_val.to_str().unwrap()
                    ));
                }
            }
            None => return Err(format!("expected header {} not found", key)),
        }
    }
    if let Some(test_body) = test_response.body.as_ref() {
        let mut diff_config = Config::new(CompareMode::Strict);
        for path in test_response.ignore_paths.iter() {
            diff_config = diff_config.ignore_path(
                Path::from_jsonpath(path.as_str())
                    .map_err(|err| format!("invalid path {}: {}", path, err))?,
            );
        }
        for order in test_response.ignore_orders.iter() {
            diff_config = diff_config.ignore_order(
                Path::from_jsonpath(order.as_str())
                    .map_err(|err| format!("invalid path {}: {}", order, err))?,
            );
        }

        let response_body = response.text().await.map_err(|e| e.to_string())?;
        let actual = &serde_json::from_str::<serde_json::Value>(response_body.as_str())
            .map_err(|err| format!("error parsing JSON response from the server: {}", err))?;
        let expected = &serde_json::from_str::<serde_json::Value>(test_body.as_str())
            .map_err(|err| format!("error parsing JSON: {}", err))?;
        let diff_result = diff(expected, actual, diff_config);
        if !diff_result.is_empty() {
            return Err(format!(
                "expected response differs from actual {}",
                diff_result
                    .iter()
                    .map(|d| d.to_string())
                    .collect::<Vec<String>>()
                    .join("\n"),
            ));
        }

        if !test_response.variables.is_empty() {
            variables.obtain_from_response(actual, &test_response.variables)?;
        }
    }
    Ok(())
}

async fn get_response(base_url: &str, test_request: &Request) -> Result<Response, String> {
    let mut request_builder = Client::new()
        .request(
            map_method(&test_request.http_method),
            format!("{}{}", base_url, test_request.uri),
        )
        .headers(map_headers(&test_request.headers)?);
    if let Some(body) = &test_request.body {
        request_builder = request_builder.body(Body::from(body.clone()));
    }
    let response = request_builder.send().await.map_err(|e| e.to_string())?;
    Ok(response)
}

fn map_headers(headers: &HashMap<String, String>) -> Result<HeaderMap, String> {
    let mut header_map = HeaderMap::new();
    for (key, value) in headers {
        let header_name = HeaderName::from_str(key.clone().as_str()).map_err(|e| e.to_string())?;
        let header_value = HeaderValue::from_str(value.as_str()).map_err(|e| e.to_string())?;
        header_map.insert(header_name, header_value);
    }
    Ok(header_map)
}

fn map_method(http_method: &HttpMethod) -> Method {
    match http_method {
        HttpMethod::Get => Method::GET,
        HttpMethod::Post => Method::POST,
        HttpMethod::Put => Method::PUT,
        HttpMethod::Delete => Method::DELETE,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;

    use crate::domain::{HttpMethod, Request, Response, RetryPolicy, TestCase};
    use crate::executor::execute;
    use crate::json_diff::path::JSONPath;
    use crate::Variables;

    #[tokio::test]
    async fn test_execute() {
        let users_endpoint = "/users";
        let header_name = "Content-Type";
        let header_value = "application/json";
        let request_body = "{\"name\":\"John\"}";
        let request_body_template = "{\"name\":`name`}";
        let response_body = "{\"id\": 1, \"name\": \"John\"}";
        let response_status = 201;
        let mut server = mockito::Server::new();
        server
            .mock("POST", users_endpoint)
            .match_header(header_name, header_value)
            .match_body(mockito::Matcher::PartialJsonString(
                request_body.to_string(),
            ))
            .with_header(header_name, header_value)
            .with_status(response_status)
            .with_body(response_body)
            .create();

        let test_case = TestCase {
            request: Request {
                http_method: HttpMethod::Post,
                headers: vec![(header_name.to_string(), header_value.to_string())]
                    .into_iter()
                    .collect(),
                uri: users_endpoint.to_string(),
                body: Some(request_body_template.to_string()),
                line_number: 1,
            },
            response: Response {
                code: response_status as u16,
                headers: vec![(header_name.to_string(), header_value.to_string())]
                    .into_iter()
                    .collect(),
                ignore_paths: vec!["$.id".to_string()],
                ignore_orders: vec![],
                body: Some(response_body.to_string()),
                line_number: 2,
                variables: HashMap::new(),
                retries: RetryPolicy::default(),
            },
        };

        let mut variables = Variables::from_json(&json!({"name":"John"})).unwrap();

        let result = execute(server.url().as_str(), test_case, &mut variables).await;

        match result {
            Ok(_) => {}
            Err(ref err) => assert_eq!("", err),
        }
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_chained() {
        let users_endpoint = "/users";
        let header_name = "Content-Type";
        let header_value = "application/json";
        let request_body = "{\"name\":\"John\"}";
        let request_body_template = "{\"name\":`name`}";
        let response_body = "{\"id\": 1, \"name\": \"John\"}";
        let response_status = 201;
        let mut server = mockito::Server::new();
        server
            .mock("POST", users_endpoint)
            .match_header(header_name, header_value)
            .match_body(mockito::Matcher::PartialJsonString(
                request_body.to_string(),
            ))
            .with_header(header_name, header_value)
            .with_status(response_status)
            .with_body(response_body)
            .create();

        server
            .mock("GET", "/users/1")
            .match_header(header_name, header_value)
            .with_header(header_name, header_value)
            .with_status(200)
            .with_body(response_body)
            .create();

        let mut response_variables = HashMap::new();
        response_variables.insert("id".to_string(), "$.id".jsonpath().unwrap());

        let test_case = TestCase {
            request: Request {
                http_method: HttpMethod::Post,
                headers: vec![(header_name.to_string(), header_value.to_string())]
                    .into_iter()
                    .collect(),
                uri: users_endpoint.to_string(),
                body: Some(request_body_template.to_string()),
                line_number: 1,
            },
            response: Response {
                code: response_status as u16,
                headers: vec![(header_name.to_string(), header_value.to_string())]
                    .into_iter()
                    .collect(),
                ignore_paths: vec!["$.id".to_string()],
                ignore_orders: vec![],
                body: Some(response_body.to_string()),
                line_number: 2,
                variables: response_variables,
                retries: RetryPolicy::default(),
            },
        };

        let mut variables = Variables::from_json(&json!({"name":"John"})).unwrap();

        let result: Result<(), String> =
            execute(server.url().as_str(), test_case, &mut variables).await;

        match result {
            Ok(_) => {}
            Err(ref err) => assert_eq!("", err),
        }
        assert!(result.is_ok());

        let test_case = TestCase {
            request: Request {
                http_method: HttpMethod::Get,
                headers: vec![(header_name.to_string(), header_value.to_string())]
                    .into_iter()
                    .collect(),
                uri: format!("{}/`id`", users_endpoint),
                body: None,
                line_number: 3,
            },
            response: Response {
                code: 200,
                headers: vec![(header_name.to_string(), header_value.to_string())]
                    .into_iter()
                    .collect(),
                ignore_paths: vec![],
                ignore_orders: vec![],
                body: Some(response_body.to_string()),
                line_number: 4,
                variables: HashMap::new(),
                retries: RetryPolicy::default(),
            },
        };

        let result: Result<(), String> =
            execute(server.url().as_str(), test_case, &mut variables).await;

        assert_eq!(Ok(()), result);
    }
}
