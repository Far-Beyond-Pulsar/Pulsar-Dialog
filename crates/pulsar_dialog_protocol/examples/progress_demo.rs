//! Drives the helper's progress window: `progress_demo [finish|fail]`.
//! Needs PULSAR_DIALOG_EXE (or the helper beside this exe / on PATH).
use pulsar_dialog_protocol::progress::{start, ProgressRequest};
use pulsar_dialog_protocol::{ButtonKind, DialogButton};

fn main() {
    let fail = std::env::args().nth(1).as_deref() == Some("fail");
    let mut window = match start(&ProgressRequest::new("Bootstrap Engine", "Rebuilding engine").status("Waiting for the editor to close")) {
        Ok(window) => window,
        Err(error) => { println!("no window: {error}"); return; }
    };
    for step in 1..=10 {
        std::thread::sleep(std::time::Duration::from_millis(400));
        window.update(Some(&format!("Building release engine ({step}/10)")), Some(step as f32 / 10.0));
    }
    let response = if fail {
        window.fail(Some("Engine rebuild failed"), "The release engine build failed with exit code: 101", Some("Bootstrap log: pulsar-engine-bootstrap.log"), vec![
            DialogButton::new("relaunch", "Relaunch last stable").kind(ButtonKind::Primary).default(),
            DialogButton::new("quit", "Quit").cancel(),
        ])
    } else {
        window.finish()
    };
    println!("{}", serde_json::to_string(&response).unwrap());
}
