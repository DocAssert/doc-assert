use std::collections::HashMap;
use std::str::FromStr;

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
}

#[derive(Debug, Clone)]
pub(crate) struct Response {
    pub code: u16,
    pub headers: HashMap<String, String>,
    pub ignore_paths: Vec<String>,
    pub body: Option<String>,
}