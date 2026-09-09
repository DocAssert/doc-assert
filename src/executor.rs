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
use crate::report::{Failure, Mismatch};
use crate::Variables;

pub(crate) async fn execute(
    base_url: &str,
    test_case: TestCase,
    variables: &mut Variables,
) -> Result<(), Failure> {
    let mut test_request = test_case.request;
    variables.replace_request_placeholders(&mut test_request)?;

    let mut test_response = test_case.response;
    variables.replace_response_placeholders(&mut test_response)?;

    let http_method = test_request.http_method.to_string();
    let uri = test_request.uri.clone();
    let request_line_number = test_request.line_number;
    let response_line_number = test_response.line_number;

    for i in 0..test_response.retries.max_retries {
        let last_attempt = i == test_response.retries.max_retries - 1;

        let failure = match get_response(base_url, &test_request).await {
            Err(reason) => Failure::RequestFailed {
                http_method: http_method.clone(),
                uri: uri.clone(),
                line_number: request_line_number,
                reason,
            },
            Ok(response) => match assert_response(response, &test_response, variables).await {
                Ok(_) => return Ok(()),
                Err(cause) => Failure::ResponseMismatch {
                    http_method: http_method.clone(),
                    uri: uri.clone(),
                    line_number: response_line_number,
                    cause,
                },
            },
        };

        if last_attempt {
            return Err(failure);
        }
        tokio::time::sleep(Duration::from_millis(test_response.retries.delay)).await;
    }

    Err(Failure::NotExecuted)
}

async fn assert_response(
    response: Response,
    test_response: &crate::domain::Response,
    variables: &mut Variables,
) -> Result<(), Mismatch> {
    if test_response.code != response.status().as_u16() {
        return Err(Mismatch::StatusCode {
            expected: test_response.code,
            actual: response.status().as_u16(),
        });
    }
    for (key, val) in test_response.headers.iter() {
        match response.headers().get(key.as_str()) {
            Some(test_val) => {
                if test_val != val.as_str() {
                    return Err(Mismatch::Header {
                        name: key.clone(),
                        expected: val.clone(),
                        actual: test_val.to_str().unwrap().to_string(),
                    });
                }
            }
            None => return Err(Mismatch::MissingHeader { name: key.clone() }),
        }
    }
    if let Some(test_body) = test_response.body.as_ref() {
        let mut diff_config = Config::new(CompareMode::Strict);
        for path in test_response.ignore_paths.iter() {
            diff_config =
                diff_config.ignore_path(Path::from_jsonpath(path.as_str()).map_err(|err| {
                    Mismatch::InvalidIgnorePath {
                        path: path.clone(),
                        reason: err.to_string(),
                    }
                })?);
        }
        for order in test_response.ignore_orders.iter() {
            diff_config =
                diff_config.ignore_order(Path::from_jsonpath(order.as_str()).map_err(|err| {
                    Mismatch::InvalidIgnorePath {
                        path: order.clone(),
                        reason: err.to_string(),
                    }
                })?);
        }

        let response_body =
            response
                .text()
                .await
                .map_err(|err| Mismatch::UnreadableResponseBody {
                    reason: err.to_string(),
                })?;
        let actual =
            &serde_json::from_str::<serde_json::Value>(response_body.as_str()).map_err(|err| {
                Mismatch::MalformedResponseBody {
                    reason: err.to_string(),
                }
            })?;
        let expected =
            &serde_json::from_str::<serde_json::Value>(test_body.as_str()).map_err(|err| {
                Mismatch::MalformedExpectedBody {
                    reason: err.to_string(),
                }
            })?;
        let diff_result = diff(expected, actual, diff_config);
        if !diff_result.is_empty() {
            return Err(Mismatch::Body {
                differences: diff_result.iter().map(|d| d.to_string()).collect(),
            });
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
        let mut server = mockito::Server::new_async().await;
        server
            .mock("POST", users_endpoint)
            .match_header(header_name, header_value)
            .match_body(mockito::Matcher::PartialJsonString(
                request_body.to_string(),
            ))
            .with_header(header_name, header_value)
            .with_status(response_status)
            .with_body(response_body)
            .create_async()
            .await;

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

        assert_eq!(Ok(()), result);
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
        let mut server = mockito::Server::new_async().await;
        server
            .mock("POST", users_endpoint)
            .match_header(header_name, header_value)
            .match_body(mockito::Matcher::PartialJsonString(
                request_body.to_string(),
            ))
            .with_header(header_name, header_value)
            .with_status(response_status)
            .with_body(response_body)
            .create_async()
            .await;

        server
            .mock("GET", "/users/1")
            .match_header(header_name, header_value)
            .with_header(header_name, header_value)
            .with_status(200)
            .with_body(response_body)
            .create_async()
            .await;

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

        let result = execute(server.url().as_str(), test_case, &mut variables).await;

        assert_eq!(Ok(()), result);

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

        let result = execute(server.url().as_str(), test_case, &mut variables).await;

        assert_eq!(Ok(()), result);
    }
}
