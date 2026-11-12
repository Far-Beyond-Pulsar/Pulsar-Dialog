//! A progress window: a long job shown while it runs, and what to ask if it fails.
//!
//! Unlike a dialog, which is one question and one answer, a progress window
//! lives as long as the job. The host sends a [`ProgressRequest`] as the
//! first line of the helper's stdin (`pulsar-dialog --progress`), then one
//! [`ProgressUpdate`] per line as the job moves, and reads one
//! [`ProgressResponse`] line from stdout when the window ends.
//!
//! ```no_run
//! use pulsar_dialog_protocol::progress::{start, ProgressRequest};
//!
//! let mut window = start(&ProgressRequest::new("Bootstrap Engine", "Rebuilding engine")).unwrap();
//! window.update(Some("Building release engine"), Some(0.4));
//! // ... the job ...
//! window.finish();
//! ```

use crate::client::{has_display, helper_path, HelperError};
use crate::{DialogButton, ThemeHint};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::process::{Child, ChildStdin, Command, Stdio};

/// The window to open.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProgressRequest {
    /// The window's title (the taskbar shows it).
    pub title: String,
    /// The large line: what is happening ("Rebuilding engine").
    pub heading: String,
    /// The smaller line under it: the current step.
    #[serde(default)]
    pub status: String,
    /// How far along, `0.0..=1.0`.
    #[serde(default)]
    pub fraction: f32,
    #[serde(default)]
    pub theme: Option<ThemeHint>,
}

impl ProgressRequest {
    pub fn new(title: impl Into<String>, heading: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            heading: heading.into(),
            status: String::new(),
            fraction: 0.0,
            theme: None,
        }
    }

    pub fn status(mut self, status: impl Into<String>) -> Self {
        self.status = status.into();
        self
    }
}

/// What the host tells the window as the job moves.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProgressUpdate {
    /// Change what is shown; a field left out stays as it was.
    Update {
        #[serde(default)]
        heading: Option<String>,
        #[serde(default)]
        status: Option<String>,
        #[serde(default)]
        fraction: Option<f32>,
    },
    /// The job failed: replace the bar with the message, and the buttons to
    /// choose from. The window stays until one is pressed (or it is closed).
    Failed {
        #[serde(default)]
        heading: Option<String>,
        message: String,
        #[serde(default)]
        detail: Option<String>,
        #[serde(default)]
        buttons: Vec<DialogButton>,
    },
    /// The job finished: close the window.
    Done,
}

/// How the window ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgressOutcome {
    /// The host said it was done.
    Finished,
    /// A button of a [`ProgressUpdate::Failed`] was pressed.
    Answered,
    /// The window was closed, or the host went away, with nothing pressed.
    Closed,
    /// No window could be shown (no helper, no display).
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressResponse {
    pub outcome: ProgressOutcome,
    #[serde(default)]
    pub button: Option<String>,
}

impl ProgressResponse {
    pub fn new(outcome: ProgressOutcome, button: Option<String>) -> Self {
        Self { outcome, button }
    }

    /// The answer was this button.
    pub fn is(&self, id: &str) -> bool {
        self.outcome == ProgressOutcome::Answered && self.button.as_deref() == Some(id)
    }
}

/// A running progress window, driven from the host.
pub struct ProgressWindow {
    child: Child,
    stdin: Option<ChildStdin>,
    /// Writing failed: the window is gone (the user closed it).
    gone: bool,
}

/// Open a progress window through the helper.
pub fn start(request: &ProgressRequest) -> Result<ProgressWindow, HelperError> {
    if !has_display() {
        return Err(HelperError::NoDisplay);
    }
    let helper = helper_path().ok_or(HelperError::HelperNotFound)?;
    let mut child = Command::new(helper)
        .arg("--progress")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(HelperError::Spawn)?;
    let stdin = child.stdin.take();
    let mut window = ProgressWindow { child, stdin, gone: false };
    let line = serde_json::to_string(request).map_err(|e| HelperError::BadResponse(e.to_string()))?;
    window.send_line(&line);
    if window.gone {
        return Err(HelperError::Failed("the helper closed at once".into()));
    }
    Ok(window)
}

impl ProgressWindow {
    fn send_line(&mut self, line: &str) {
        let Some(stdin) = self.stdin.as_mut() else {
            self.gone = true;
            return;
        };
        if writeln!(stdin, "{line}").and_then(|()| stdin.flush()).is_err() {
            self.gone = true;
        }
    }

    fn send(&mut self, update: &ProgressUpdate) {
        if let Ok(line) = serde_json::to_string(update) {
            self.send_line(&line);
        }
    }

    /// Change the status line and/or how far along the job is. Returns false
    /// once the window is gone (closed by the user): the job carries on, unseen.
    pub fn update(&mut self, status: Option<&str>, fraction: Option<f32>) -> bool {
        self.send(&ProgressUpdate::Update {
            heading: None,
            status: status.map(str::to_owned),
            fraction,
        });
        !self.gone
    }

    pub fn set_heading(&mut self, heading: &str) {
        self.send(&ProgressUpdate::Update {
            heading: Some(heading.to_owned()),
            status: None,
            fraction: None,
        });
    }

    /// Whether the window has gone away (closed by the user).
    pub fn is_gone(&self) -> bool {
        self.gone
    }

    /// The job failed. Shows `message` with `buttons` and waits for the user's
    /// choice, which is the answer (a closed window is [`ProgressOutcome::Closed`]).
    pub fn fail(
        mut self,
        heading: Option<&str>,
        message: &str,
        detail: Option<&str>,
        buttons: Vec<DialogButton>,
    ) -> ProgressResponse {
        if self.gone {
            return ProgressResponse::new(ProgressOutcome::Closed, None);
        }
        self.send(&ProgressUpdate::Failed {
            heading: heading.map(str::to_owned),
            message: message.to_owned(),
            detail: detail.map(str::to_owned),
            buttons,
        });
        self.wait()
    }

    /// The job finished: close the window and wait for it to go.
    pub fn finish(mut self) -> ProgressResponse {
        self.send(&ProgressUpdate::Done);
        self.wait()
    }

    fn wait(mut self) -> ProgressResponse {
        // Closing our end of the pipe tells a helper that never heard `Done`
        // that the host is gone; with `Done` or `Failed` sent it already knows.
        drop(self.stdin.take());
        match self.child.wait_with_output() {
            Ok(output) => parse_response(&output.stdout),
            Err(_) => ProgressResponse::new(ProgressOutcome::Closed, None),
        }
    }
}

/// The response is the last JSON line of the helper's stdout.
pub fn parse_response(stdout: &[u8]) -> ProgressResponse {
    String::from_utf8_lossy(stdout)
        .lines()
        .rev()
        .find_map(|line| serde_json::from_str::<ProgressResponse>(line.trim()).ok())
        .unwrap_or_else(|| ProgressResponse::new(ProgressOutcome::Closed, None))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ButtonKind;

    #[test]
    fn updates_round_trip_and_leave_out_what_they_do_not_change() {
        let update = ProgressUpdate::Update { heading: None, status: Some("Building".into()), fraction: Some(0.4) };
        let json = serde_json::to_string(&update).unwrap();
        assert_eq!(serde_json::from_str::<ProgressUpdate>(&json).unwrap(), update);
        let minimal: ProgressUpdate = serde_json::from_str(r#"{"kind":"update","status":"x"}"#).unwrap();
        assert_eq!(
            minimal,
            ProgressUpdate::Update { heading: None, status: Some("x".into()), fraction: None }
        );
        assert_eq!(serde_json::from_str::<ProgressUpdate>(r#"{"kind":"done"}"#).unwrap(), ProgressUpdate::Done);
    }

    #[test]
    fn a_failure_carries_its_message_and_buttons() {
        let failed = ProgressUpdate::Failed {
            heading: Some("Engine rebuild failed".into()),
            message: "cargo exited with 101".into(),
            detail: None,
            buttons: vec![DialogButton::new("relaunch", "Relaunch last stable").kind(ButtonKind::Primary).default()],
        };
        let json = serde_json::to_string(&failed).unwrap();
        assert_eq!(serde_json::from_str::<ProgressUpdate>(&json).unwrap(), failed);
    }

    #[test]
    fn the_response_is_the_last_json_line_and_a_mute_helper_is_closed() {
        let stdout = b"noise\n{\"outcome\":\"answered\",\"button\":\"quit\"}\n";
        assert!(parse_response(stdout).is("quit"));
        assert_eq!(parse_response(b"").outcome, ProgressOutcome::Closed);
        assert_eq!(parse_response(b"not json").outcome, ProgressOutcome::Closed);
        let finished = parse_response(b"{\"outcome\":\"finished\"}\n");
        assert_eq!(finished.outcome, ProgressOutcome::Finished);
        assert!(!finished.is("quit"));
    }

    #[test]
    fn with_no_helper_there_is_no_window_and_no_panic() {
        std::env::set_var(crate::client::HELPER_ENV, "definitely-not-a-real-helper-xyz");
        let result = start(&ProgressRequest::new("t", "h"));
        std::env::remove_var(crate::client::HELPER_ENV);
        assert!(result.is_err());
    }
}
