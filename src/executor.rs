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
use serde_json::Value;

use crate::domain::{HttpMethod, Request, TestCase};
use crate::json_diff::{diff, CompareMode, Config};
use crate::report::{Failure, Mismatch};
use crate::Variables;

pub(crate) async fn execute(
    base_url: &str,
    test_case: TestCase,
    variables: &mut Variables,
) -> Result<(), Failure> {
    let mut request = test_case.request;
    variables.replace_request_placeholders(&mut request)?;

    let mut expected = test_case.response;
    variables.replace_response_placeholders(&mut expected)?;

    // what the documentation describes is checked before anything is sent, as sending the
    // request again would not make it any more valid
    let headers =
        map_headers(&request.headers).map_err(|reason| Failure::InvalidDocumentation {
            line_number: request.line_number,
            reason,
        })?;
    let expected_body = expected
        .body
        .as_deref()
        .map(serde_json::from_str::<Value>)
        .transpose()
        .map_err(|err| Failure::InvalidDocumentation {
            line_number: expected.line_number,
            reason: format!("expected body is not valid JSON: {}", err),
        })?;
    let diff_config = diff_config(&expected);

    let client = Client::new();
    let mut attempt = 0;
    loop {
        attempt += 1;

        let failure = match send(&client, base_url, &request, headers.clone()).await {
            Err(reason) => Failure::RequestFailed { reason },
            Ok(response) => {
                match assert_response(response, &expected, expected_body.as_ref(), &diff_config)
                    .await
                {
                    Ok(extracted) => {
                        variables.extend(extracted);
                        return Ok(());
                    }
                    Err(cause) => Failure::ResponseMismatch {
                        line_number: expected.line_number,
                        cause,
                    },
                }
            }
        };

        if attempt >= expected.retries.max_attempts {
            return Err(failure);
        }
        tokio::time::sleep(Duration::from_millis(expected.retries.delay)).await;
    }
}

fn diff_config(expected: &crate::domain::Response) -> Config {
    let mut config = Config::new(CompareMode::Strict);
    for path in &expected.ignore_paths {
        config = config.ignore_path(path.clone());
    }
    for path in &expected.ignore_orders {
        config = config.ignore_order(path.clone());
    }
    config
}

/// Checks a response against the expected one, returning the variables extracted from it.
async fn assert_response(
    response: Response,
    expected: &crate::domain::Response,
    expected_body: Option<&Value>,
    diff_config: &Config,
) -> Result<HashMap<String, Value>, Mismatch> {
    if expected.code != response.status().as_u16() {
        return Err(Mismatch::StatusCode {
            expected: expected.code,
            actual: response.status().as_u16(),
        });
    }
    for (key, val) in expected.headers.iter() {
        match response.headers().get(key.as_str()) {
            Some(actual) => {
                if actual != val.as_str() {
                    return Err(Mismatch::Header {
                        name: key.clone(),
                        expected: val.clone(),
                        actual: String::from_utf8_lossy(actual.as_bytes()).into_owned(),
                    });
                }
            }
            None => return Err(Mismatch::MissingHeader { name: key.clone() }),
        }
    }

    let Some(expected_body) = expected_body else {
        return Ok(HashMap::new());
    };

    let response_body = response
        .text()
        .await
        .map_err(|err| Mismatch::UnreadableResponseBody {
            reason: err.to_string(),
        })?;
    let actual = serde_json::from_str::<Value>(response_body.as_str()).map_err(|err| {
        Mismatch::MalformedResponseBody {
            reason: err.to_string(),
        }
    })?;
    let diff_result = diff(expected_body, &actual, diff_config.clone());
    if !diff_result.is_empty() {
        return Err(Mismatch::Body {
            differences: diff_result.iter().map(|d| d.to_string()).collect(),
        });
    }

    Variables::extract_from_response(&actual, &expected.variables)
}

async fn send(
    client: &Client,
    base_url: &str,
    request: &Request,
    headers: HeaderMap,
) -> Result<Response, String> {
    let mut request_builder = client
        .request(
            map_method(&request.http_method),
            format!("{}{}", base_url, request.uri),
        )
        .headers(headers);
    if let Some(body) = &request.body {
        request_builder = request_builder.body(Body::from(body.clone()));
    }
    request_builder.send().await.map_err(|e| e.to_string())
}

fn map_headers(headers: &HashMap<String, String>) -> Result<HeaderMap, String> {
    let mut header_map = HeaderMap::new();
    for (key, value) in headers {
        let header_name = HeaderName::from_str(key.as_str())
            .map_err(|e| format!("invalid header name {}: {}", key, e))?;
        let header_value = HeaderValue::from_str(value.as_str())
            .map_err(|e| format!("invalid value of header {}: {}", key, e))?;
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

    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use crate::domain::{HttpMethod, Request, Response, RetryPolicy, TestCase};
    use crate::executor::execute;
    use crate::json_diff::path::JSONPath;
    use crate::report::{Failure, Mismatch};
    use crate::Variables;

    /// `GET /resource` expecting a 200 with `body`, sent as many times as `retries` allows.
    fn get_resource(body: Option<&str>, retries: RetryPolicy) -> TestCase {
        TestCase {
            request: Request {
                http_method: HttpMethod::Get,
                headers: HashMap::new(),
                uri: "/resource".to_string(),
                body: None,
                line_number: 1,
            },
            response: Response {
                code: 200,
                headers: HashMap::new(),
                ignore_paths: vec![],
                ignore_orders: vec![],
                body: body.map(str::to_string),
                line_number: 4,
                variables: HashMap::new(),
                retries,
            },
        }
    }

    fn retries(max_attempts: u64, delay: u64) -> RetryPolicy {
        RetryPolicy {
            max_attempts,
            delay,
        }
    }

    /// Mocks `GET /resource` answering with the bodies one after the other, the last one
    /// repeated once they are exhausted.
    async fn serve_in_turn(
        server: &mut mockito::ServerGuard,
        bodies: &'static [&'static str],
    ) -> mockito::Mock {
        let served = Arc::new(AtomicUsize::new(0));
        server
            .mock("GET", "/resource")
            .with_status(200)
            .with_body_from_request(move |_| {
                let i = served.fetch_add(1, Ordering::SeqCst);
                bodies[i.min(bodies.len() - 1)].into()
            })
            .create_async()
            .await
    }

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
                ignore_paths: vec!["$.id".jsonpath().unwrap()],
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
                ignore_paths: vec!["$.id".jsonpath().unwrap()],
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

    #[tokio::test]
    async fn test_a_test_case_passes_as_soon_as_an_attempt_does() {
        let mut server = mockito::Server::new_async().await;
        let mock = serve_in_turn(
            &mut server,
            &[
                r#"{"ready":false}"#,
                r#"{"ready":false}"#,
                r#"{"ready":true}"#,
            ],
        )
        .await
        .expect(3);
        let test_case = get_resource(Some(r#"{"ready":true}"#), retries(5, 50));

        let started = Instant::now();
        let result = execute(server.url().as_str(), test_case, &mut Variables::new()).await;

        assert_eq!(Ok(()), result);
        mock.assert_async().await;
        assert!(started.elapsed() >= Duration::from_millis(100));
    }

    #[tokio::test]
    async fn test_the_failure_of_the_last_attempt_is_reported() {
        let mut server = mockito::Server::new_async().await;
        let mock = serve_in_turn(&mut server, &[r#"{"v":1}"#, r#"{"v":2}"#, r#"{"v":3}"#])
            .await
            .expect(3);
        let test_case = get_resource(Some(r#"{"v":0}"#), retries(3, 0));

        let result = execute(server.url().as_str(), test_case, &mut Variables::new()).await;

        // the policy counts attempts, so it is sent 3 times rather than retried 3 times
        mock.assert_async().await;
        match result {
            Err(Failure::ResponseMismatch {
                line_number: 4,
                cause: Mismatch::Body { differences },
            }) => {
                let differences = differences.join("\n");
                assert!(differences.contains('3'), "{}", differences);
                assert!(!differences.contains('2'), "{}", differences);
            }
            other => panic!("unexpected result: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_there_is_no_delay_after_the_last_attempt() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/resource")
            .with_status(500)
            .create_async()
            .await;
        let test_case = get_resource(None, retries(2, 500));

        let started = Instant::now();
        let result = execute(server.url().as_str(), test_case, &mut Variables::new()).await;
        let elapsed = started.elapsed();

        assert!(result.is_err());
        assert!(elapsed >= Duration::from_millis(500), "{:?}", elapsed);
        assert!(elapsed < Duration::from_millis(1000), "{:?}", elapsed);
    }

    #[tokio::test]
    async fn test_an_unreachable_server_fails_the_request() {
        let test_case = get_resource(None, retries(2, 50));

        let started = Instant::now();
        let result = execute("http://127.0.0.1:1", test_case, &mut Variables::new()).await;

        assert!(
            matches!(result, Err(Failure::RequestFailed { .. })),
            "{:?}",
            result
        );
        assert!(started.elapsed() >= Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_an_invalid_expected_body_fails_without_sending_the_request() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/resource")
            .expect(0)
            .create_async()
            .await;
        let test_case = get_resource(Some("{not json"), retries(3, 1000));

        let result = execute(server.url().as_str(), test_case, &mut Variables::new()).await;

        assert!(
            matches!(
                result,
                Err(Failure::InvalidDocumentation { line_number: 4, .. })
            ),
            "{:?}",
            result
        );
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_an_invalid_request_header_fails_without_sending_the_request() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/resource")
            .expect(0)
            .create_async()
            .await;
        let mut test_case = get_resource(None, retries(3, 1000));
        test_case
            .request
            .headers
            .insert("Bad Name".to_string(), "value".to_string());

        let result = execute(server.url().as_str(), test_case, &mut Variables::new()).await;

        assert!(
            matches!(
                result,
                Err(Failure::InvalidDocumentation { line_number: 1, .. })
            ),
            "{:?}",
            result
        );
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_an_unresolved_variable_fails_without_sending_the_request() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", mockito::Matcher::Any)
            .expect(0)
            .create_async()
            .await;
        let mut test_case = get_resource(None, RetryPolicy::default());
        test_case.request.uri = "/users/`id`".to_string();

        let result = execute(server.url().as_str(), test_case, &mut Variables::new()).await;

        assert_eq!(
            Err(Failure::UnresolvedVariables {
                names: vec!["id".to_string()]
            }),
            result
        );
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_a_non_ascii_header_value_is_reported_rather_than_panicking() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/resource")
            .with_status(200)
            .with_header("X-Name", "café")
            .create_async()
            .await;
        let mut test_case = get_resource(None, RetryPolicy::default());
        test_case
            .response
            .headers
            .insert("X-Name".to_string(), "cafe".to_string());

        let result = execute(server.url().as_str(), test_case, &mut Variables::new()).await;

        assert_eq!(
            Err(Failure::ResponseMismatch {
                line_number: 4,
                cause: Mismatch::Header {
                    name: "X-Name".to_string(),
                    expected: "cafe".to_string(),
                    actual: "café".to_string(),
                }
            }),
            result
        );
    }

    #[tokio::test]
    async fn test_variables_are_only_kept_from_a_passing_attempt() {
        let mut server = mockito::Server::new_async().await;
        serve_in_turn(&mut server, &[r#"{"id":1}"#]).await;
        let mut test_case = get_resource(Some(r#"{"id":1}"#), RetryPolicy::default());
        test_case.response.variables = [
            ("id".to_string(), "$.id".jsonpath().unwrap()),
            ("missing".to_string(), "$.missing".jsonpath().unwrap()),
        ]
        .into_iter()
        .collect();
        let mut variables = Variables::new();

        let result = execute(server.url().as_str(), test_case, &mut variables).await;

        assert_eq!(
            Err(Failure::ResponseMismatch {
                line_number: 4,
                cause: Mismatch::VariableNotFound {
                    name: "missing".to_string()
                }
            }),
            result
        );
        // `id` was found, but not kept since the test case did not pass
        let mut request = get_resource(None, RetryPolicy::default()).request;
        request.uri = "/users/`id`".to_string();
        assert!(variables
            .replace_request_placeholders(&mut request)
            .is_err());
    }
}
