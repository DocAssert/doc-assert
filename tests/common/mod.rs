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

//! Helpers shared by the integration tests.

use std::sync::atomic::{AtomicUsize, Ordering};

/// One test case requesting `GET /passing`, then one requesting `GET /failing`, both
/// expecting a 200. The requests are defined at lines 1 and 7, the responses at 4 and 10.
pub const DOC: &str = "```docassertrequest\n\
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

/// Documentation file removing itself once it goes out of scope.
pub struct TempDoc {
    path: std::path::PathBuf,
}

impl TempDoc {
    pub fn new(content: &str) -> Self {
        // tests run in parallel, so every file of this process gets a number of its own
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "doc_assert_{}_{}.md",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::write(&path, content).unwrap();
        Self { path }
    }

    pub fn path(&self) -> &str {
        self.path.to_str().unwrap()
    }
}

impl Drop for TempDoc {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
