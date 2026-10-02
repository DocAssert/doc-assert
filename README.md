[![DocAssert](https://github.com/DocAssert/doc-assert/actions/workflows/doc-assert.yml/badge.svg)](https://github.com/DocAssert/doc-assert/actions/workflows/doc-assert.yml)
[![crates.io](https://img.shields.io/crates/v/doc-assert.svg)](https://crates.io/crates/doc-assert)

**DocAssert** is a documentation testing tool that offers a completely new approach.
Write your documentation as a story you want to tell your users and test it against your API.
It can be really usefule for AI assited development when you want to provide more context for your agent.


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

// a test of your own, under `#[tokio::test]`
async fn test_docs() {
    DocAssert::new("http://localhost:8080")
        .with_doc_path("README.md")
        .assert()
        .await;
}
```

`assert` prints every test case as it is executed, the same way the [command line tool](#using-command-line-tool)
does, and fails the test unless all of them passed. Like any test output, it is shown when the test fails, or as it
is printed with `cargo test -- --nocapture`.

If you would rather handle the outcome yourself, `run` hands back a `Report` without printing anything:

```rust
use doc_assert::DocAssert;

async fn print_failures() {
    let report = DocAssert::new("http://localhost:8080")
        .with_doc_path("README.md")
        .run()
        .await
        .unwrap();

    println!("{} of {} passed", report.passed_count(), report.total_count());
    for (id, failure) in report.failures() {
        println!("{} failed: {}", id, failure);
    }
}
```

`run` returns `Err` only when the run could not be performed at all, for instance because a documentation file
could not be parsed. Test cases that failed are reported by the `Report`, and `Report::passed` tells whether every
one of them passed.

To handle every test case as soon as it has been executed, drive the run yourself with `start`, which parses the
documentation and hands back a `Run`. This one stops at the first failure:

```rust
use doc_assert::DocAssert;

async fn fail_fast() {
    let mut run = DocAssert::new("http://localhost:8080")
        .with_doc_path("README.md")
        .start()
        .unwrap();

    while let Some(result) = run.next().await {
        println!("{}", result);
        if !result.passed() {
            break;
        }
    }

    // the test cases left are reported as not run
    println!("{}", run.finish().summary());
}
```

Stop the run between calls to `next` rather than racing it against a timeout or selecting on it: a test case
dropped part way through is abandoned and reported as not run, even though its request may already have reached
the server.

Failures are structured, so you can inspect them, or render them your own way, by matching on them:

```rust
use doc_assert::{Failure, Mismatch};

fn is_server_error(failure: &Failure) -> bool {
    matches!(
        failure,
        Failure::ResponseMismatch {
            cause: Mismatch::StatusCode { actual: 500..=599, .. },
            ..
        }
    )
}
```

#### Variables

In some case we may need to set some value which will be shared between requests. For instance test auth token.

We can define variable in the API before we run the tests:

```rust
use doc_assert::{DocAssert, Variables};

// a test of your own, under `#[tokio::test]`
async fn test_docs() {
    let mut variables = Variables::new();
    variables.insert("auth_token", "some_token");

    DocAssert::new("http://localhost:8080")
        .with_doc_path("README.md")
        .with_variables(variables)
        .assert()
        .await;
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

The first number in the retry policy is the number of attempts, the first one included, and the second number is the
delay between attempts in milliseconds: `(3,4500)` sends the request up to 3 times, waiting 4.5 seconds after each
attempt that failed. The number of attempts must be at least 1.

### Using command line tool

Instead of integrating DocAssert into your tests, you can also use it as a standalone command-line tool:

```bash
doc-assert --url http://localhost:8081 --variables '{"auth_token": "some_token"}' README.md
```

Every documentation file is parsed before any request is sent. The test cases are then printed as they are executed,
the way `cargo test` prints its own: each one is named as soon as its request is sent and marked once it is done, so
a slow or retried request shows what is being waited for. The details of the failures follow at the end:

```text
2 tests
GET /blog (README.md:12) ✅
POST /blog (README.md:30) ❌

failures:
-------------
POST /blog (README.md:30): response at line 36: expected response code 201, got 500

test result: FAILED. 1 passed; 1 failed
```

The exit code tells how the run went, and errors preventing it are printed to stderr:

| Code | Meaning |
|------|---------|
| 0    | every test case passed |
| 2    | invalid arguments, such as no documentation file, or variables that are not a JSON object |
| 3    | a documentation file could not be read or parsed, in which case no request was sent |
| 4    | at least one test case failed |

### Using DocAssert for AI-assisted development

DocAssert is a good fit for AI-assisted workflows where an agent updates API docs and code in the same task.
After AI-generated changes, run DocAssert to verify that examples in your `README.md` still match real API behavior.

It can also be included in AI "skills" or reusable task templates, for example:

- add a step like `doc-assert --url http://localhost:8081 README.md` to your skill's verification checklist
- require DocAssert to pass before accepting AI-generated documentation updates
- run DocAssert in CI for PRs created by AI agents, so docs and API behavior stay aligned

## Installation

To use DocAssert as a CLI tool you can install it using cargo:

```bash
cargo install doc-assert --features="binary"
```

In order to build it directly from the source code run:

```bash
cargo build --features="binary"
```
