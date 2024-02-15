use clap::Parser;
use doc_assert::variables::Variables;
use doc_assert::DocAssert;
use std::convert::From;
use std::path::PathBuf;
use std::str::FromStr;

use serde_json::Value;
use std::io::Write;

#[macro_export]
macro_rules! write_to_file {
    ($writer:expr, $msg:expr) => {
        if let Err(err) = writeln!($writer, $msg) {
            eprintln!("Error: {}", err);
            std::process::exit(Code::INTERAL_ERROR);
        }
    };

    ($writer:expr, $msg:expr, $($arg:tt)*) => {
        if let Err(err) = writeln!($writer, $msg, $($arg)*) {
            eprintln!("Error: {}", err);
            std::process::exit(Code::INTERAL_ERROR);
        }
    };
}

#[derive(Debug, Clone)]
struct JSONVars(Value);

impl FromStr for JSONVars {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = serde_json::from_str(s)?;
        Ok(JSONVars(value))
    }
}

#[macro_export]
macro_rules! handle_error {
    ($writer:expr, $code:expr, $msg:expr, $($arg:tt)*) => {
        write_to_file!($writer, $msg, $($arg)*);
        std::process::exit($code);
    };

    ($writer:expr, $code:expr, $msg:expr) => {
        write_to_file!($writer, $msg);
        std::process::exit($code);
    };
}

struct Code;

impl Code {
    const SUCCESS: i32 = 0;
    const INTERAL_ERROR: i32 = 1;
    const INVALID_ARGUMENT: i32 = 2;
    const DOC_VALIDATION_ERROR: i32 = 3;
}

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

    /// Output file (optional, default to stdout)
    #[clap(short, long)]
    output: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let mut writter = Box::new(std::io::stdout()) as Box<dyn Write>;

    match &cli.variables {
        Some(vars) => {
            if let Value::String(_) = vars.0 {
                handle_error!(
                    writter,
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
                handle_error!(writter, Code::INVALID_ARGUMENT, "Error: {}", e);
            }
        },
        None => Variables::new(),
    };

    let mut doc_assert = DocAssert::new()
        .with_url(cli.url.as_str())
        .with_variables(variables);

    for file in cli.files.iter() {
        let Some(file) = file.to_str() else {
            handle_error!(writter, Code::INVALID_ARGUMENT, "Error: Invalid file path");
        };

        doc_assert = doc_assert.with_doc_path(file);
    }

    let result = doc_assert.assert().await;

    if let Err(errors) = result {
        for error in errors {
            if let Err(err) = writeln!(writter, "{}", error) {
                eprintln!("{}", err);
                std::process::exit(Code::INTERAL_ERROR);
            }
        }
        std::process::exit(Code::DOC_VALIDATION_ERROR);
    }

    write_to_file!(writter, "All files are OK");
    std::process::exit(Code::SUCCESS);
}
