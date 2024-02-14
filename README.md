[![DocAssert](https://github.com/DocAssert/doc-assert/actions/workflows/doc-assert.yml/badge.svg)](https://github.com/DocAssert/doc-assert/actions/workflows/doc-assert.yml)

# Welcome to **DocAssert** - Blazing Fast Documentation Testing!
DocAssert offers a blazing fast, incredibly reliable, and remarkably user-friendly approach to documentation testing. By ensuring your documentation accurately represents your API's behavior, DocAssert empowers you to deliver a trustworthy and engaging narrative to your users. Start transforming your documentation into a verified, living document that enhances user trust and comprehension with DocAssert.

## How it works?
DocAssert reads the specified `README.md` file and scans it for code blocks containing descriptions of requests and responses. It then sends these requests to your API and verifies whether the responses match those specified in the documentation. This process ensures that your documentation accurately reflects the capabilities and behavior of your API.

## Quick Start Guide
1. **Define your API interactions** in your `README.md` with detailed code blocks for requests and responses.
1. **Run DocAssert** through your tests or as a standalone command-line tool to verify each documented interaction against your live API.
1. **Adjust and improve** your documentation based on the test results to ensure accuracy and reliability.
  
**DocAssert** seamlessly integrates into your development workflow, offering a straightforward and effective method to enhance the quality and trustworthiness of your API documentation. Whether you're a developer, a technical writer, or a project manager, DocAssert simplifies the process of maintaining up-to-date and verified documentation, fostering confidence among your users and stakeholders.

### Using test API

First, you need to define your documentation in the `README.md` file. A request can look like this:

~~~markdown
```docassertrequest
POST /api/user
Content-Type: application/json
{
    "name": "test"
}
```
~~~

The above definition instructs DocAssert to send a `POST` request to `/api/user` with the
`Content-Type: application/json` header and the body as specified in the code block. Note the `docassertrequest`
at the beginning of the code block. Your documentation can contain any amount of text, code blocks, and other
elements between the DocAssert code blocks. Only the code blocks with `docassertrequest` and `docassertresponse`
will be parsed.

An expected response can be defined like this:

~~~markdown
```docassertresponse
HTTP 201
Content-Type: application/json
{
    "id": 1,
    "name": "test"
}
```

[ignore]: # ($.id)
~~~

This configuration tells DocAssert to expect a response with the status code `201` and the
`Content-Type: application/json` header. The response body will be checked as well, but you can specify JSONPaths
that you wish to ignore. This feature is useful if your responses contain random values like IDs or timestamps.
Remember to place `[ignore]: # (your_json_path)` after the response code block. You can include as many of these as
necessary.

Once your documentation is prepared, you can run DocAssert from your tests like so:

```rust
use doc_assert::DocAssert;

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_docs() {
        let result = DocAssert::new()
            .with_url("http://localhost:8080")
            .with_doc_path("README.md")
            .assert()
            .await;
        assert!(result.is_ok());
    }
}
```

In case of `Err` the result will contain a list of errors with detailed information about what went wrong.

### Using command line tool

Instead of integrating DocAssert into your tests, you can also use it as a standalone command-line tool:

```bash
doc-assert --url http://localhost:8081 lib/README.md
```
