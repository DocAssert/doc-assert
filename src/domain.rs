use std::collections::HashMap;

struct TestCase {
    name: String,
    request: Request,
    response: Response,
}

// TODO consider using client's enums?
enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
}

struct Request {
    // TODO add cert
    http_method: HttpMethod,
    headers: HashMap<String, String>,
    url: String,
    body: String,
}

struct Response {
    code: i32,
    headers: HashMap<String, String>,
    body: String,
}