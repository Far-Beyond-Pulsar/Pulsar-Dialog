//! What Pulsar asks of its dialogs and context menus.
//!
//! A [`DialogRequest`] is plain data: it says what to show, never how. The
//! `pulsar-dialog` helper (or the `pulsar_dialog` library, in a process that
//! already has a GPUI app) draws it in the engine's own look and answers with
//! a [`DialogResponse`]. This crate has no UI or GPU dependency, so a process
//! can ask before it has a window, or a working GPU.
//!
//! ```no_run
//! use pulsar_dialog_protocol::{client, DialogRequest, Severity};
//!
//! let answer = client::show(
//!     &DialogRequest::new("No Discrete GPU Detected", "Continue using the available GPU?")
//!         .severity(Severity::Warning)
//!         .yes_no(),
//! );
//! if answer.is_yes() {
//!     // carry on
//! }
//! ```

pub mod client;
pub mod menu;
pub mod progress;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub use menu::{ContextMenuRequest, ContextMenuResponse, MenuItem};

/// How serious a dialog is: sets its icon and accent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    #[default]
    Info,
    Question,
    Warning,
    Error,
}

/// A button's emphasis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ButtonKind {
    /// The main answer; drawn filled.
    Primary,
    #[default]
    Secondary,
    /// An answer that destroys something.
    Danger,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialogButton {
    /// What the response names when this button is pressed.
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub kind: ButtonKind,
    /// Pressed by Enter.
    #[serde(default)]
    pub default: bool,
    /// Pressed by Escape, and what closing the window means.
    #[serde(default)]
    pub cancel: bool,
}

impl DialogButton {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind: ButtonKind::Secondary,
            default: false,
            cancel: false,
        }
    }

    pub fn kind(mut self, kind: ButtonKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn default(mut self) -> Self {
        self.default = true;
        self
    }

    pub fn cancel(mut self) -> Self {
        self.cancel = true;
        self
    }
}

/// A "Don't ask again" style option.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckboxOption {
    pub label: String,
    #[serde(default)]
    pub checked: bool,
}

/// Where the dialog finds its theme. Without it the helper reads the user's
/// saved theme and themes folder, and with no usable theme the dialog is dark.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeHint {
    pub name: Option<String>,
    pub themes_dir: Option<PathBuf>,
}

/// The id of the button [`DialogRequest::yes_no`] adds for "Yes".
pub const YES: &str = "yes";
pub const NO: &str = "no";
pub const OK: &str = "ok";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialogRequest {
    pub title: String,
    pub message: String,
    /// Smaller print under the message: a path, an error's text.
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub severity: Severity,
    /// With none, the dialog gets an "OK".
    #[serde(default)]
    pub buttons: Vec<DialogButton>,
    #[serde(default)]
    pub checkbox: Option<CheckboxOption>,
    #[serde(default)]
    pub theme: Option<ThemeHint>,
}

impl DialogRequest {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            detail: None,
            severity: Severity::Info,
            buttons: Vec::new(),
            checkbox: None,
            theme: None,
        }
    }

    pub fn severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn button(mut self, button: DialogButton) -> Self {
        self.buttons.push(button);
        self
    }

    pub fn checkbox(mut self, label: impl Into<String>, checked: bool) -> Self {
        self.checkbox = Some(CheckboxOption { label: label.into(), checked });
        self
    }

    pub fn theme(mut self, theme: ThemeHint) -> Self {
        self.theme = Some(theme);
        self
    }

    /// "Yes" (the default, filled) and "No" (cancel).
    pub fn yes_no(self) -> Self {
        self.button(DialogButton::new(YES, "Yes").kind(ButtonKind::Primary).default())
            .button(DialogButton::new(NO, "No").cancel())
    }

    /// A single "OK".
    pub fn ok(self) -> Self {
        self.button(DialogButton::new(OK, "OK").kind(ButtonKind::Primary).default().cancel())
    }

    /// The buttons to draw: the request's, or an "OK" when it has none.
    pub fn effective_buttons(&self) -> Vec<DialogButton> {
        if self.buttons.is_empty() {
            vec![DialogButton::new(OK, "OK").kind(ButtonKind::Primary).default().cancel()]
        } else {
            self.buttons.clone()
        }
    }

    /// The button Enter presses: the one marked default, else the first.
    pub fn default_button(&self) -> Option<DialogButton> {
        let buttons = self.effective_buttons();
        buttons
            .iter()
            .find(|button| button.default)
            .or_else(|| buttons.first())
            .cloned()
    }

    /// The button Escape presses, and what closing the window answers.
    pub fn cancel_button(&self) -> Option<DialogButton> {
        self.effective_buttons().into_iter().find(|button| button.cancel)
    }
}

/// How the question ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// The user pressed a button.
    Answered,
    /// The window was closed with no cancel button to stand for it.
    Dismissed,
    /// Nothing could show the dialog (no helper, no display): the caller
    /// decides what the silence means.
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialogResponse {
    pub outcome: Outcome,
    /// The pressed button's id.
    #[serde(default)]
    pub button: Option<String>,
    /// The checkbox's state, when the request had one.
    #[serde(default)]
    pub checkbox: Option<bool>,
}

impl DialogResponse {
    pub fn answered(button: impl Into<String>, checkbox: Option<bool>) -> Self {
        Self { outcome: Outcome::Answered, button: Some(button.into()), checkbox }
    }

    pub fn dismissed(checkbox: Option<bool>) -> Self {
        Self { outcome: Outcome::Dismissed, button: None, checkbox }
    }

    pub fn unavailable() -> Self {
        Self { outcome: Outcome::Unavailable, button: None, checkbox: None }
    }

    /// The answer was this button.
    pub fn is(&self, id: &str) -> bool {
        self.outcome == Outcome::Answered && self.button.as_deref() == Some(id)
    }

    pub fn is_yes(&self) -> bool {
        self.is(YES)
    }

    pub fn is_ok(&self) -> bool {
        self.is(OK)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_round_trips_through_json_and_defaults_what_it_omits() {
        let request = DialogRequest::new("Title", "Body")
            .severity(Severity::Warning)
            .detail("C:\\path")
            .yes_no()
            .checkbox("Don't ask again", false);
        let json = serde_json::to_string(&request).unwrap();
        assert_eq!(serde_json::from_str::<DialogRequest>(&json).unwrap(), request);
        let minimal: DialogRequest =
            serde_json::from_str(r#"{"title":"t","message":"m"}"#).unwrap();
        assert_eq!(minimal.severity, Severity::Info);
        assert!(minimal.buttons.is_empty() && minimal.checkbox.is_none() && minimal.theme.is_none());
    }

    #[test]
    fn enter_and_escape_pick_the_default_and_cancel_buttons() {
        let request = DialogRequest::new("t", "m").yes_no();
        assert_eq!(request.default_button().unwrap().id, YES);
        assert_eq!(request.cancel_button().unwrap().id, NO);
        let none_marked = DialogRequest::new("t", "m")
            .button(DialogButton::new("a", "A"))
            .button(DialogButton::new("b", "B"));
        assert_eq!(none_marked.default_button().unwrap().id, "a");
        assert!(none_marked.cancel_button().is_none());
    }

    #[test]
    fn a_request_with_no_buttons_gets_an_ok() {
        let request = DialogRequest::new("t", "m");
        let buttons = request.effective_buttons();
        assert_eq!(buttons.len(), 1);
        assert_eq!(buttons[0].id, OK);
        assert!(request.cancel_button().is_some(), "Escape closes it");
    }

    #[test]
    fn only_an_answer_counts_as_a_button() {
        assert!(DialogResponse::answered(YES, None).is_yes());
        assert!(!DialogResponse::dismissed(None).is_yes());
        assert!(!DialogResponse::unavailable().is_yes());
        assert!(!DialogResponse::answered(NO, None).is_yes());
    }
}
