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

//! Exercises the `doc-assert` binary: what it prints, when it prints it, and how it exits.

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::{TempDoc, DOC};

/// How long the binary is given to print what is expected of it
const TIMEOUT: Duration = Duration::from_secs(30);

/// Minimal HTTP server answering every request with the status `route` gives its path.
struct Server {
    url: String,
    requests: Arc<AtomicUsize>,
}

impl Server {
    fn start(route: impl Fn(&str) -> u16 + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(AtomicUsize::new(0));
        let route = Arc::new(route);

        let counter = requests.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { return };
                let route = route.clone();
                let counter = counter.clone();
                std::thread::spawn(move || {
                    let mut reader = BufReader::new(stream);
                    let mut request_line = String::new();
                    reader.read_line(&mut request_line).unwrap();
                    // the rest of the head is read before answering; none of the requests
                    // the tests send has a body
                    let mut line = String::new();
                    while reader.read_line(&mut line).unwrap() > 2 {
                        line.clear();
                    }
                    counter.fetch_add(1, Ordering::SeqCst);

                    let path = request_line.split_whitespace().nth(1).unwrap_or("");
                    let status = route(path);
                    let _ = write!(
                        reader.get_mut(),
                        "HTTP/1.1 {} Status\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        status
                    );
                });
            }
        });

        Self { url, requests }
    }

    fn requests(&self) -> usize {
        self.requests.load(Ordering::SeqCst)
    }
}

fn doc_assert(args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_doc-assert"));
    command.args(args);
    command
}

fn run(args: &[&str]) -> Output {
    doc_assert(args).output().unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

/// Kills the binary if a test gives up on it.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
    }
}

/// Forwards everything the binary prints as soon as it is printed.
fn forward_stdout(child: &mut Child) -> Receiver<Vec<u8>> {
    let mut stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = [0; 1024];
        while let Ok(n) = stdout.read(&mut buf) {
            if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                return;
            }
        }
    });
    rx
}

/// Accumulates the output into `printed` until it holds `expected`, failing the test if it
/// takes too long or the output does not match.
fn expect_printed(output: &Receiver<Vec<u8>>, printed: &mut Vec<u8>, expected: &str) {
    while printed.len() < expected.len() {
        let chunk = output.recv_timeout(TIMEOUT).unwrap_or_else(|_| {
            panic!(
                "expected {:?} to be printed, got {:?}",
                expected,
                String::from_utf8_lossy(printed)
            )
        });
        printed.extend(chunk);
    }
    assert_eq!(expected, String::from_utf8_lossy(printed));
}

#[test]
fn test_every_test_case_is_printed_as_it_is_executed() {
    // `/failing` is held until the test releases it, so whatever is printed before that
    // was printed while the run was still going on
    let (release, released) = mpsc::channel::<()>();
    let released = Mutex::new(released);
    let server = Server::start(move |path| {
        if path == "/failing" {
            let _ = released.lock().unwrap().recv();
        }
        200
    });
    let doc = TempDoc::new(DOC);

    let mut child = doc_assert(&["--url", &server.url, doc.path()])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let output = forward_stdout(&mut child);
    let mut child = Running(child);

    let mut printed = vec![];
    let while_running = format!(
        "2 tests\n\
         GET /passing ({path}:1) ✅\n\
         GET /failing ({path}:7) ",
        path = doc.path()
    );
    // the test case being executed is named before its response comes back
    expect_printed(&output, &mut printed, &while_running);

    release.send(()).unwrap();
    let status = child.0.wait().unwrap();
    while let Ok(chunk) = output.recv_timeout(TIMEOUT) {
        printed.extend(chunk);
    }

    assert_eq!(
        format!(
            "{}✅\n\
             \n\
             test result: PASSED. 2 passed; 0 failed\n",
            while_running
        ),
        String::from_utf8_lossy(&printed)
    );
    assert_eq!(Some(0), status.code());
}

#[test]
fn test_failures_are_detailed_once_every_test_case_was_executed() {
    let server = Server::start(|path| if path == "/failing" { 500 } else { 200 });
    let doc = TempDoc::new(DOC);

    let output = run(&["--url", &server.url, doc.path()]);

    assert_eq!(
        format!(
            "2 tests\n\
             GET /passing ({path}:1) ✅\n\
             GET /failing ({path}:7) ❌\n\
             \n\
             failures:\n\
             -------------\n\
             GET /failing ({path}:7): response at line 10: expected response code 200, got 500\n\
             \n\
             test result: FAILED. 1 passed; 1 failed\n",
            path = doc.path()
        ),
        stdout(&output)
    );
    assert_eq!(Some(4), output.status.code());
}

#[test]
fn test_variables_are_substituted() {
    let server = Server::start(|path| if path == "/users/42" { 200 } else { 404 });
    let doc = TempDoc::new(
        "```docassertrequest\n\
         GET /users/`id`\n\
         ```\n\
         ```docassertresponse\n\
         HTTP 200\n\
         ```\n",
    );

    let output = run(&["--url", &server.url, "-v", r#"{"id": 42}"#, doc.path()]);

    assert_eq!(Some(0), output.status.code(), "{}", stdout(&output));
}

#[test]
fn test_an_unreachable_server_fails_the_test_cases() {
    let doc = TempDoc::new(DOC);

    let output = run(&["--url", "http://127.0.0.1:1", doc.path()]);

    assert!(stdout(&output).contains("request failed: "));
    assert_eq!(Some(4), output.status.code());
}

#[test]
fn test_invalid_arguments_are_rejected() {
    let doc = TempDoc::new(DOC);
    let cases: [(&[&str], &str); 4] = [
        (&[], "Error: no documentation file to test"),
        (
            &["-v", r#""id""#, doc.path()],
            "Error: Variables must be a JSON object",
        ),
        (
            &["-v", "[1]", doc.path()],
            "Error: Variables must be a JSON object",
        ),
        (&["-v", "{", doc.path()], "invalid value"),
    ];

    for (args, message) in cases {
        let args = [&["--url", "http://127.0.0.1:1"], args].concat();
        let output = run(&args);

        assert_eq!(Some(2), output.status.code(), "{:?}", args);
        assert!(
            stderr(&output).contains(message),
            "{:?}: {}",
            args,
            stderr(&output)
        );
        assert_eq!("", stdout(&output), "{:?}", args);
    }
}

#[test]
fn test_documentation_that_cannot_be_parsed_is_rejected_before_any_request() {
    let server = Server::start(|_| 200);
    let valid = TempDoc::new(DOC);
    let invalid = TempDoc::new(
        "```docassertrequest\n\
         GET /passing\n\
         ```\n\
         ```docassertresponse\n\
         HTTP 200\n\
         ```\n\
         [retry]: # (0, 10)\n",
    );

    for args in [
        vec![valid.path(), "this/file/does/not/exist.md"],
        vec![valid.path(), invalid.path()],
    ] {
        let args = [vec!["--url", server.url.as_str()], args].concat();
        let output = run(&args);

        assert_eq!(Some(3), output.status.code(), "{:?}", args);
        assert!(
            stderr(&output).starts_with("Error: cannot parse "),
            "{:?}: {}",
            args,
            stderr(&output)
        );
    }
    assert_eq!(0, server.requests());
}
