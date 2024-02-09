use clap::Parser;
use doc_assert::DocAssert;
use std::convert::From;
use std::path::PathBuf;

use std::fs::File;
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
    const CANNOT_WRITE_FILE: i32 = 4;
}

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Documentation files to process
    files: Vec<PathBuf>,

    /// URL to test against
    #[clap(short, long, required = true)]
    url: String,

    /// Output file (optional, default to stdout)
    #[clap(short, long)]
    output: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let mut writter = Box::new(std::io::stdout()) as Box<dyn Write>;

    if let Some(output) = cli.output {
        match File::create(output) {
            Ok(file) => {
                writter = Box::new(file) as Box<dyn Write>;
            }
            Err(e) => {
                handle_error!(writter, Code::CANNOT_WRITE_FILE, "Error: {}", e);
            }
        }
    }

    let mut catched_error = false;

    for file in cli.files.iter() {
        let Some(file) = file.to_str() else {
            handle_error!(writter, Code::INVALID_ARGUMENT, "Error: Invalid file path");
        };

        let result = DocAssert::new()
            .with_url(cli.url.as_str())
            .with_doc_path(file)
            .assert()
            .await;

        if let Err(errors) = result {
            catched_error = true;
            for error in errors {
                if let Err(err) = writeln!(writter, "{}: Error: {}", file, error) {
                    eprintln!("Error: {}", err);
                    std::process::exit(Code::INTERAL_ERROR);
                }
            }
        } else {
            write_to_file!(writter, "{}: OK", file)
        }
    }

    if catched_error {
        std::process::exit(Code::DOC_VALIDATION_ERROR);
    }

    write_to_file!(writter, "All files are OK");
    std::process::exit(Code::SUCCESS);
}
