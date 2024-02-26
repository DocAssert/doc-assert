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

use std::convert::From;
use std::path::PathBuf;
use std::str::FromStr;

use clap::Parser;
use serde_json::Value;

use doc_assert::variables::Variables;
use doc_assert::AssertionError;
use doc_assert::DocAssert;

#[doc(hidden)]
#[macro_export]
macro_rules! write_to_file {
    ($writer:expr, $msg:expr) => {
        if let Err(err) = writeln!($writer, $msg) {
            eprintln!("Error: {}", err);
            std::process::exit(Code::INTERNAL_ERROR);
        }
    };

    ($writer:expr, $msg:expr, $($arg:tt)*) => {
        if let Err(err) = writeln!($writer, $msg, $($arg)*) {
            eprintln!("Error: {}", err);
            std::process::exit(Code::INTERNAL_ERROR);
        }
    };
}

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
#[macro_export]
macro_rules! handle_error {
    ($code:expr, $msg:expr, $($arg:tt)*) => {
        println!($msg, $($arg)*);
        std::process::exit($code);
    };

    ($code:expr, $msg:expr) => {
        println!($msg);
        std::process::exit($code);
    };
}

#[doc(hidden)]
struct Code;

impl Code {
    const SUCCESS: i32 = 0;
    const INTERNAL_ERROR: i32 = 1;
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

    match &cli.variables {
        Some(vars) => {
            if let Value::String(_) = vars.0 {
                handle_error!(
                    Code::INVALID_ARGUMENT,
                    "Error: Variables must be a JSON object"
                );
            }
        }
        None => {}
    }

    let variables = match cli.variables {
        Some(vars) => match Variables::from_json(&vars.0) {
            Ok(vars) => vars,
            Err(e) => {
                handle_error!(Code::INVALID_ARGUMENT, "Error: {}", e);
            }
        },
        None => Variables::new(),
    };

    let mut doc_assert = DocAssert::new()
        .with_url(cli.url.as_str())
        .with_variables(variables);

    for file in cli.files.iter() {
        let Some(file) = file.to_str() else {
            handle_error!(Code::INVALID_ARGUMENT, "error: Invalid file path");
        };

        doc_assert = doc_assert.with_doc_path(file);
    }

    let result = doc_assert.assert().await;

    match result {
        Ok(report) => {
            println!("{}", report);
            std::process::exit(Code::SUCCESS);
        }
        Err(err) => match err {
            AssertionError::ParsingError(err) => {
                handle_error!(Code::DOC_PARSING_ERROR, "Error parsing file: {}", err);
            }
            AssertionError::TestSuiteError(report) => {
                handle_error!(Code::DOC_ASSERTION_ERROR, "{}", report);
            }
        },
    }
}
