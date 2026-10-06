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

use std::path::PathBuf;
use std::str::FromStr;

use clap::Parser;
use serde_json::Value;

use doc_assert::DocAssert;
use doc_assert::Error;
use doc_assert::Variables;

#[doc(hidden)]
#[derive(Debug, Clone)]
struct JSONVars(Value);

impl FromStr for JSONVars {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = serde_json::from_str(s)?;
        Ok(JSONVars(value))
    }
}

#[doc(hidden)]
macro_rules! handle_error {
    ($code:expr, $msg:expr, $($arg:tt)*) => {
        eprintln!($msg, $($arg)*);
        std::process::exit($code);
    };

    ($code:expr, $msg:expr) => {
        eprintln!($msg);
        std::process::exit($code);
    };
}

#[doc(hidden)]
struct Code;

impl Code {
    const SUCCESS: i32 = 0;
    const INVALID_ARGUMENT: i32 = 2;
    const DOC_PARSING_ERROR: i32 = 3;
    const DOC_ASSERTION_ERROR: i32 = 4;
}

#[doc(hidden)]
#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Documentation files to process
    files: Vec<PathBuf>,

    /// URL to test against
    #[clap(short, long, required = true)]
    url: String,

    /// Variables to be used in the assertions in the JSON object format
    #[clap(short, long)]
    variables: Option<JSONVars>,
}

#[doc(hidden)]
#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let variables = match cli.variables {
        Some(vars) => match Variables::from_json(&vars.0) {
            Some(vars) => vars,
            None => {
                handle_error!(
                    Code::INVALID_ARGUMENT,
                    "Error: Variables must be a JSON object"
                );
            }
        },
        None => Variables::new(),
    };

    let mut doc_assert = DocAssert::new(cli.url).with_variables(variables);

    for file in cli.files.iter() {
        let Some(file) = file.to_str() else {
            handle_error!(Code::INVALID_ARGUMENT, "Error: invalid file path");
        };

        doc_assert = doc_assert.with_doc_path(file);
    }

    let run = match doc_assert.start() {
        Ok(run) => run,
        Err(err @ Error::NoDocuments) => {
            handle_error!(Code::INVALID_ARGUMENT, "Error: {}", err);
        }
        Err(err) => {
            handle_error!(Code::DOC_PARSING_ERROR, "Error: {}", err);
        }
    };

    if run.print_progress().await.passed() {
        std::process::exit(Code::SUCCESS);
    }
    std::process::exit(Code::DOC_ASSERTION_ERROR);
}
