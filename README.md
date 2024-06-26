[![DocAssert](https://github.com/DocAssert/doc-assert/actions/workflows/doc-assert.yml/badge.svg)](https://github.com/DocAssert/doc-assert/actions/workflows/doc-assert.yml)
[![crates.io](https://img.shields.io/crates/v/doc-assert.svg)](https://crates.io/crates/doc-assert)

**DocAssert** is a documentation testing tool that offers a completely new approach.
Write your documentation as a story you want to tell your users and test it against your API.


## How it works?

DocAssert reads the specified `README.md` file and scans it for code blocks containing descriptions of requests
and responses. It then sends these requests to your API and verifies whether the responses match those specified
in the documentation.

### Using test API

First, you need to define your documentation in the `README.md` file. A request can look like this:

~~~markdown
```docassertrequest
POST /blog
Content-Type: application/json
{
    "title": "My First Blog",
    "body": "Blog content"
}
```
~~~

The above definition instructs DocAssert to send a `POST` request to `/blog` with the
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
    "id": "d8f7d454-c436-4e0f-9613-1d69036ad421",
    "title": "My First Blog",
    "body": "Blog content"
}
```

[ignore]: # ($.id)
[ignore]: # ($.date_upd)
[ignore]: # ($.comments)
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
        match result {
            Ok(report) => {
                // handle report
            }
            Err(err) => {
                // handle error
            }
        }
    }
}
```

In case of `Err` the result will contain a list of errors with detailed information about what went wrong.

#### Variables

In some case we may need to set some value which will be shared between requests. For instance test auth token.

We can define variable in the API before we run the tests:

```rust
use doc_assert::DocAssert;

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_docs() {
        let result = DocAssert::new()
            .with_url("http://localhost:8080")
            .with_doc_path("README.md")
            .with_variable("auth_token", "some_token")
            .assert()
            .await;
        match result {
            Ok(report) => {
                // handle report
            }
            Err(err) => {
                // handle error
            }
        }
    }
}
```

Variables can be also dynamically defined in the documentation. First we have a request:

~~~markdown
```docassertrequest
POST /blog
Content-Type: application/json
{
    "title": "My First Blog",
    "body": "Blog content"
}
```
~~~

This will result in a response:

~~~markdown
```docassertresponse
HTTP 201
Content-Type: application/json
{
    "id": "d8f7d454-c436-4e0f-9613-1d69036ad421",
    "title": "My First Blog",
    "body": "Blog content"
}
```

[ignore]: # ($.id)
[ignore]: # ($.date_upd)
[ignore]: # ($.comments)
[let id]: # ($.id)
~~~

In the example above some of the fields are ignored but we also define a variable `id` which will be used in the next
request. Notice that variable can be defined on ignored field.

Now we can use this variable in the next request:

~~~markdown
```docassertrequest
GET /blog/`id`
```
~~~

Which will result in a response:

~~~markdown
```docassertresponse
HTTP 200
Content-Type: application/json
{
    "id": `id`,
    "title": "My First Blog",
    "body": "Blog content"
}
```
[ignore]: # ($.date_upd)
[ignore]: # ($.comments)
~~~

Notice that `id` is also used in response and will be evaluated during assertions.

#### Retry policy

In some cases, you may want to retry the request if it fails. You can define a retry policy in the documentation:

~~~markdown
```docassertrequest
GET /blog/`id`
```
~~~

~~~markdown
```docassertresponse
HTTP 200
Content-Type: application/json
```
[retry]: # (3,4500)
~~~

The first number in the retry policy is the number of retries, and the second number is the delay between retries in milliseconds.

### Using command line tool

Instead of integrating DocAssert into your tests, you can also use it as a standalone command-line tool:

```bash
doc-assert --url http://localhost:8081 --variables '{"auth_token": "some_token"}' README.md
```

## Installation

To use DocAssert as a CLI tool you can install it using cargo:

```bash
cargo install doc-assert --features="binary"
```

In order to build it directly from the source code run:

```bash
cargo build --features="binary"
```
