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

mod common;

use common::{TempDoc, DOC};
use doc_assert::{DocAssert, Error, Failure, Mismatch};

struct Server {
    server: mockito::ServerGuard,
    passing: mockito::Mock,
    failing: mockito::Mock,
}

/// Serves `/passing` with a 200 and `/failing` with a 500.
async fn server() -> Server {
    let mut server = mockito::Server::new_async().await;
    let passing = server
        .mock("GET", "/passing")
        .with_status(200)
        .create_async()
        .await;
    let failing = server
        .mock("GET", "/failing")
        .with_status(500)
        .create_async()
        .await;
    Server {
        server,
        passing,
        failing,
    }
}

#[tokio::test]
async fn test_test_cases_are_executed_one_at_a_time() {
    let server = server().await;
    let doc = TempDoc::new(DOC);

    let mut run = DocAssert::new(server.server.url())
        .with_doc_path(doc.path())
        .start()
        .unwrap();

    // the count is known before anything has been executed
    assert_eq!(2, run.total_count());
    assert!(!server.passing.matched_async().await);

    let first = run.next().await.unwrap();
    assert!(first.passed());
    assert_eq!("GET", first.id().http_method());
    assert_eq!("/passing", first.id().uri());
    assert!(server.passing.matched_async().await);
    // the result of the first is available before the second has been sent
    assert!(!server.failing.matched_async().await);

    let second = run.next().await.unwrap();
    assert!(!second.passed());
    assert_eq!("/failing", second.id().uri());
    assert!(server.failing.matched_async().await);

    assert!(run.next().await.is_none());
    assert!(run.next().await.is_none());

    let report = run.finish();
    assert_eq!(2, report.executed_count());
    assert_eq!(1, report.passed_count());
    assert_eq!(1, report.failed_count());
    assert!(!report.passed());
}

#[tokio::test]
async fn test_failure_says_where_and_why_it_failed() {
    let server = server().await;
    let doc = TempDoc::new(DOC);

    let report = DocAssert::new(server.server.url())
        .with_doc_path(doc.path())
        .run()
        .await
        .unwrap();

    let failures = report.failures().collect::<Vec<_>>();
    assert_eq!(1, failures.len());
    let (id, failure) = failures[0];
    assert_eq!(doc.path(), id.doc_path());
    assert_eq!(7, id.line_number());
    assert_eq!(
        &Failure::ResponseMismatch {
            line_number: 10,
            cause: Mismatch::StatusCode {
                expected: 200,
                actual: 500
            }
        },
        failure
    );
}

#[tokio::test]
async fn test_run_stopped_early_reports_what_was_not_run() {
    let server = server().await;
    let doc = TempDoc::new(DOC);

    let mut run = DocAssert::new(server.server.url())
        .with_doc_path(doc.path())
        .start()
        .unwrap();

    run.next().await.unwrap();
    let report = run.finish();

    assert_eq!(2, report.total_count());
    assert_eq!(1, report.executed_count());
    assert_eq!(1, report.not_run_count());
    // nothing failed, but not everything passed either
    assert!(!report.passed());
    assert!(!server.failing.matched_async().await);
}

#[tokio::test]
async fn test_documentation_files_run_in_order_and_share_variables() {
    let mut server = mockito::Server::new_async().await;
    server
        .mock("POST", "/users")
        .with_status(201)
        .with_body(r#"{"id": 42}"#)
        .create_async()
        .await;
    server
        .mock("GET", "/users/42")
        .with_status(200)
        .create_async()
        .await;
    let create = TempDoc::new(
        "```docassertrequest\n\
         POST /users\n\
         ```\n\
         ```docassertresponse\n\
         HTTP 201\n\
         {\"id\": 0}\n\
         ```\n\
         [ignore]: # ($.id)\n\
         [let id]: # ($.id)\n",
    );
    let get = TempDoc::new(
        "```docassertrequest\n\
         GET /users/`id`\n\
         ```\n\
         ```docassertresponse\n\
         HTTP 200\n\
         ```\n",
    );

    let report = DocAssert::new(server.url())
        .with_doc_path(create.path())
        .with_doc_path(get.path())
        .run()
        .await
        .unwrap();

    assert!(report.passed(), "{}", report);
    let doc_paths = report
        .results()
        .iter()
        .map(|r| r.id().doc_path())
        .collect::<Vec<_>>();
    assert_eq!(vec![create.path(), get.path()], doc_paths);
}

#[tokio::test]
async fn test_parsing_error_stops_the_run_before_any_request() {
    let server = server().await;
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

    let err = DocAssert::new(server.server.url())
        .with_doc_path(valid.path())
        .with_doc_path(invalid.path())
        .start()
        .unwrap_err();

    match err {
        Error::Parse { doc_path, reason } => {
            assert_eq!(invalid.path(), doc_path);
            assert!(reason.contains("at line 7"), "{}", reason);
            assert!(reason.contains("at least 1"), "{}", reason);
        }
        other => panic!("unexpected error: {:?}", other),
    }
    assert!(!server.passing.matched_async().await);
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

#[tokio::test]
async fn test_assert_passes_when_every_test_case_does() {
    let server = server().await;
    let doc = TempDoc::new(
        "```docassertrequest\n\
         GET /passing\n\
         ```\n\
         ```docassertresponse\n\
         HTTP 200\n\
         ```\n",
    );

    DocAssert::new(server.server.url())
        .with_doc_path(doc.path())
        .assert()
        .await;
}

#[tokio::test]
#[should_panic(expected = "1 of 2 documentation test cases failed")]
async fn test_assert_panics_when_a_test_case_fails() {
    let server = server().await;
    let doc = TempDoc::new(DOC);

    DocAssert::new(server.server.url())
        .with_doc_path(doc.path())
        .assert()
        .await;
}

#[tokio::test]
#[should_panic(expected = "cannot parse this/file/does/not/exist.md")]
async fn test_assert_panics_when_the_run_cannot_be_performed() {
    DocAssert::new("http://localhost:8080")
        .with_doc_path("this/file/does/not/exist.md")
        .assert()
        .await;
}
