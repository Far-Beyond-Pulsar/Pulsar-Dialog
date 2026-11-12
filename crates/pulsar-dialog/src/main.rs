//! `pulsar-dialog`: show one dialog, print the answer.
//!
//! Reads a `DialogRequest` as JSON from stdin (to the end of its input, or
//! from the file named by `--request-file`), shows it in the engine's look,
//! and prints one line of JSON, the `DialogResponse`, to stdout. Exit status
//! 0 means a response was printed; 2 means the request could not be read.
//! Log lines go to stderr.

use pulsar_dialog::protocol::{DialogRequest, DialogResponse};
use std::io::Read;

fn read_request() -> Result<DialogRequest, String> {
    let mut args = std::env::args().skip(1);
    let mut text = String::new();
    match args.next().as_deref() {
        Some("--request-file") => {
            let path = args.next().ok_or("--request-file needs a path")?;
            text = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
        }
        Some(other) => return Err(format!("unknown argument {other}; the request is read from stdin")),
        None => {
            std::io::stdin()
                .read_to_string(&mut text)
                .map_err(|e| format!("reading stdin: {e}"))?;
        }
    }
    serde_json::from_str(&text).map_err(|e| format!("the request is not valid: {e}"))
}

fn main() {
    let request = match read_request() {
        Ok(request) => request,
        Err(error) => {
            eprintln!("pulsar-dialog: {error}");
            println!("{}", serde_json::to_string(&DialogResponse::unavailable()).unwrap());
            std::process::exit(2);
        }
    };
    let response = pulsar_dialog::run_blocking(request);
    println!("{}", serde_json::to_string(&response).unwrap());
}
