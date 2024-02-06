use std::collections::HashMap;
use std::str::FromStr;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Body, Client, Method};

use crate::domain::{HttpMethod, TestCase};
use crate::json_diff::path::Path;
use crate::json_diff::{diff, CompareMode, Config};

pub(crate) async fn execute(base_url: String, test_cases: TestCase) -> Result<(), String> {
    let test_request = test_cases.request;
    let mut request_builder = Client::new()
        .request(
            map_method(test_request.http_method),
            format!("{}{}", base_url, test_request.uri),
        )
        .headers(map_headers(&test_request.headers)?);
    if let Some(body) = test_request.body {
        request_builder = request_builder.body(Body::from(body));
    }
    let response = request_builder.send().await.map_err(|e| e.to_string())?;
    let test_response = test_cases.response;
    if test_response.code != response.status().as_u16() {
        return Err(format!(
            "Expected response code {}, got {}",
            test_response.code,
            response.status().as_u16()
        ));
    }
    for (key, val) in test_response.headers {
        match response.headers().get(key.as_str()) {
            Some(test_val) => {
                if test_val != val.as_str() {
                    return Err(format!(
                        "Expected header {} to be {}, got {}",
                        key,
                        val,
                        test_val.to_str().unwrap()
                    ));
                }
            }
            None => return Err(format!("Expected header {} not found", key)),
        }
    }
    if let Some(test_body) = test_response.body {
        let mut diff_config = Config::new(CompareMode::Strict);
        for path in test_response.ignore_paths.iter() {
            diff_config = diff_config
                .ignore_path(Path::from_jsonpath(path.as_str()).map_err(|e| e.to_string())?);
        }
        let response_body = response.text().await.map_err(|e| e.to_string())?;
        let lhs = &serde_json::from_str::<serde_json::Value>(response_body.as_str())
            .map_err(|e| e.to_string())?;
        let rhs = &serde_json::from_str::<serde_json::Value>(test_body.as_str())
            .map_err(|e| e.to_string())?;
        let diff_result = diff(lhs, rhs, diff_config);
        if !diff_result.is_empty() {
            return Err(format!(
                "Expected response differs from actual {}",
                diff_result
                    .iter()
                    .map(|d| d.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ));
        }
    }
    Ok(())
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

fn map_method(http_method: HttpMethod) -> Method {
    match http_method {
        HttpMethod::Get => Method::GET,
        HttpMethod::Post => Method::POST,
        HttpMethod::Put => Method::PUT,
        HttpMethod::Delete => Method::DELETE,
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::{HttpMethod, Request, Response, TestCase};
    use crate::executor::execute;

    #[tokio::test]
    async fn test_execute() {
        let users_endpoint = "/users";
        let header_name = "Content-Type";
        let header_value = "application/json";
        let request_body = "{\"name\":\"test\"}";
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
                body: Some(request_body.to_string()),
            },
            response: Response {
                code: response_status as u16,
                headers: vec![(header_name.to_string(), header_value.to_string())]
                    .into_iter()
                    .collect(),
                ignore_paths: vec!["$.id".to_string()],
                body: Some(response_body.to_string()),
            },
        };
        let result = execute(server.url(), test_case).await;
        match result {
            Ok(_) => {}
            Err(ref err) => {
                println!("{}", err)
            }
        }
        assert!(result.is_ok());
    }
}
