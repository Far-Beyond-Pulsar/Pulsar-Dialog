//! `pulsar-dialog`: show one dialog, print the answer.
//!
//! Dialog mode (the default): reads a `DialogRequest` as JSON from stdin (to
//! the end of its input, or from the file named by `--request-file`), shows it
//! in the engine's look, and prints one line of JSON, the `DialogResponse`, to
//! stdout. Exit status 0 means a response was printed; 2 means the request
//! could not be read.
//!
//! Progress mode (`--progress`): the first line of stdin is a `ProgressRequest`,
//! each further line a `ProgressUpdate`; the window stays until the host says
//! `done`, the job fails and a button is pressed, or the window is closed. One
//! line of JSON, the `ProgressResponse`, goes to stdout when it ends.
//!
//! Log lines go to stderr.

use pulsar_dialog::protocol::progress::{ProgressOutcome, ProgressRequest, ProgressResponse, ProgressUpdate};
use pulsar_dialog::protocol::{DialogRequest, DialogResponse};
use std::io::{BufRead, Read};

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

fn progress_mode() {
    // `Stdin` itself (not its lock) so the reader can move to the thread below.
    let mut reader = std::io::BufReader::new(std::io::stdin());
    let mut first = String::new();
    let request: Result<ProgressRequest, String> = match reader.read_line(&mut first) {
        Ok(read) if read > 0 => {
            serde_json::from_str(&first).map_err(|e| format!("the request is not valid: {e}"))
        }
        _ => Err("no request on stdin".to_string()),
    };
    let request = match request {
        Ok(request) => request,
        Err(error) => {
            eprintln!("pulsar-dialog: {error}");
            let unavailable = ProgressResponse::new(ProgressOutcome::Unavailable, None);
            println!("{}", serde_json::to_string(&unavailable).unwrap());
            std::process::exit(2);
        }
    };
    // The host's updates arrive on a thread of their own; the window polls them.
    // When the host closes its end the channel disconnects, which the window
    // reads as "the host went away".
    let (sender, receiver) = std::sync::mpsc::channel::<ProgressUpdate>();
    std::thread::spawn(move || {
        for line in reader.lines().map_while(Result::ok) {
            match serde_json::from_str::<ProgressUpdate>(&line) {
                Ok(update) => {
                    if sender.send(update).is_err() {
                        break;
                    }
                }
                Err(error) => eprintln!("pulsar-dialog: ignoring an update that is not valid: {error}"),
            }
        }
    });
    let response = pulsar_dialog::run_progress_blocking(request, receiver);
    println!("{}", serde_json::to_string(&response).unwrap());
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--progress") {
        progress_mode();
        return;
    }
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
