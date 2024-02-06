use clap::Parser;
use doc_assert::DocAssert;
use std::convert::From;
use std::path::PathBuf;

use std::fs::File;
use std::io::Write;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    file: Vec<PathBuf>,

    #[clap(short, long, required = true)]
    url: String,

    #[clap(short, long)]
    output: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let mut writter = Box::new(std::io::stdout()) as Box<dyn Write>;

    if let Some(output) = cli.output {
        let file = File::create(output).unwrap();
        writter = Box::new(file) as Box<dyn Write>;
    }

    for file in cli.file.iter() {
        let result = DocAssert::new()
            .with_url(cli.url.clone())
            .with_doc_path(file.to_str().unwrap().to_string())
            .assert()
            .await;

        if let Err(errors) = result {
            for error in errors {
                writeln!(writter, "{}: Error: {}", file.to_str().unwrap(), error).unwrap();
            }
        }
    }
}
