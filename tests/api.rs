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

//! Exercises the library the way a test suite depending on it would: through its public
//! API only, compiled as a separate crate.

use doc_assert::{DocAssert, Error, Failure, Mismatch};

/// Documentation file removing itself once it goes out of scope.
struct TempDoc {
    path: std::path::PathBuf,
}

impl TempDoc {
    fn new(content: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "doc_assert_{}_{}.md",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, content).unwrap();
        Self { path }
    }

    fn path(&self) -> &str {
        self.path.to_str().unwrap()
    }
}

impl Drop for TempDoc {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// One test case hitting `/passing`, then one hitting `/failing`.
const DOC: &str = "```docassertrequest\n\
                   GET /passing\n\
                   ```\n\
                   ```docassertresponse\n\
                   HTTP 200\n\
                   ```\n\
                   ```docassertrequest\n\
                   GET /failing\n\
                   ```\n\
                   ```docassertresponse\n\
                   HTTP 200\n\
                   ```\n";

async fn server() -> mockito::ServerGuard {
    let mut server = mockito::Server::new_async().await;
    server
        .mock("GET", "/passing")
        .with_status(200)
        .create_async()
        .await;
    server
        .mock("GET", "/failing")
        .with_status(500)
        .create_async()
        .await;
    server
}

#[tokio::test]
async fn test_every_test_case_is_available_as_soon_as_it_is_executed() {
    let server = server().await;
    let doc = TempDoc::new(DOC);

    let mut run = DocAssert::new(server.url())
        .with_doc_path(doc.path())
        .start()
        .unwrap();

    // the count is known before anything has been executed
    assert_eq!(2, run.total_count());
    assert_eq!(0, run.completed_count());

    let first = run.next().await.unwrap();
    assert!(first.passed());
    assert_eq!("GET", first.id().http_method());
    assert_eq!("/passing", first.id().uri());
    // the second test case has not been executed at the point the first is returned
    assert_eq!(1, run.completed_count());

    let second = run.next().await.unwrap();
    assert!(!second.passed());
    assert_eq!("/failing", second.id().uri());

    assert!(run.next().await.is_none());

    let report = run.finish();
    assert_eq!(2, report.executed_count());
    assert_eq!(1, report.passed_count());
    assert_eq!(1, report.failed_count());
    assert!(!report.passed());
    assert_eq!(1, report.failures().count());
}

#[tokio::test]
async fn test_failure_carries_the_reason_it_failed() {
    let server = server().await;
    let doc = TempDoc::new(DOC);

    let report = DocAssert::new(server.url())
        .with_doc_path(doc.path())
        .run()
        .await
        .unwrap();

    let failure = report.failures().next().unwrap().failure().unwrap();
    match failure {
        Failure::ResponseMismatch { cause, .. } => assert_eq!(
            &Mismatch::StatusCode {
                expected: 200,
                actual: 500
            },
            cause
        ),
        other => panic!("unexpected failure: {:?}", other),
    }
}

#[tokio::test]
async fn test_run_can_be_stopped_early_and_still_report() {
    let server = server().await;
    let doc = TempDoc::new(DOC);

    let mut run = DocAssert::new(server.url())
        .with_doc_path(doc.path())
        .start()
        .unwrap();

    run.next().await.unwrap();
    let report = run.finish();

    // only the test cases that were executed are reported
    assert_eq!(1, report.executed_count());
    assert!(report.passed());
}

#[test]
fn test_a_run_without_documentation_is_an_error() {
    assert_eq!(
        Some(Error::NoDocuments),
        DocAssert::new("http://localhost:8080").start().err()
    );
}

#[test]
fn test_parsing_error_names_the_file_it_comes_from() {
    let err = DocAssert::new("http://localhost:8080")
        .with_doc_path("this/file/does/not/exist.md")
        .start()
        .unwrap_err();

    match err {
        Error::Parse { doc_path, .. } => assert_eq!("this/file/does/not/exist.md", doc_path),
        other => panic!("unexpected error: {:?}", other),
    }
}
