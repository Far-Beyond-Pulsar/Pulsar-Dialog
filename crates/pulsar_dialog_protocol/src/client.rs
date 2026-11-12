//! Showing a dialog from a process that has no window, or no GPU yet.
//!
//! [`show`] starts the `pulsar-dialog` helper, sends it the request as JSON on
//! its stdin, waits, and reads the [`DialogResponse`] from its stdout. The
//! helper is a separate process on purpose: it draws with its own GPU
//! context, so a broken graphics setup in the caller (the reason many of
//! these dialogs exist) cannot take the dialog down with it.
//!
//! When the helper cannot run, the answer is [`Outcome::Unavailable`], or with
//! the `native-fallback` feature the platform's own message box.

use crate::{DialogRequest, DialogResponse, Outcome};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Names the helper executable, overriding the search.
pub const HELPER_ENV: &str = "PULSAR_DIALOG_EXE";

const HELPER_NAME: &str = if cfg!(windows) { "pulsar-dialog.exe" } else { "pulsar-dialog" };

/// The helper's file name on this platform (`pulsar-dialog.exe`, `pulsar-dialog`).
/// A host that ships the helper puts it under this name beside its own
/// executable, which is where [`helper_path`] looks.
pub const fn helper_file_name() -> &'static str {
    HELPER_NAME
}

static HELPER_OVERRIDE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Say where the helper is, for a host that put it somewhere other than
/// beside its executable (a per-user cache, when that directory is read-only).
/// Set once, early; later calls are ignored. Preferred over the environment
/// variable, which cannot be set safely once other threads run.
pub fn set_helper_path(path: PathBuf) {
    let _ = HELPER_OVERRIDE.set(path);
}

/// Where the helper is: `PULSAR_DIALOG_EXE`, else the path a host gave with
/// [`set_helper_path`], else beside the running executable, else on `PATH`.
pub fn helper_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(HELPER_ENV).filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(path));
    }
    if let Some(path) = HELPER_OVERRIDE.get().filter(|path| path.is_file()) {
        return Some(path.clone());
    }
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(HELPER_NAME)))
        .filter(|path| path.is_file());
    beside.or_else(|| {
        std::env::var_os("PATH").and_then(|path| {
            std::env::split_paths(&path)
                .map(|dir| dir.join(HELPER_NAME))
                .find(|candidate| candidate.is_file())
        })
    })
}

/// Whether a window could open at all. Linux without a display server (WSL
/// without WSLg, an SSH session) cannot show anything.
pub fn has_display() -> bool {
    if cfg!(target_os = "linux") {
        ["DISPLAY", "WAYLAND_DISPLAY", "WLR_RDP_BACKENDS"]
            .iter()
            .any(|name| std::env::var_os(name).is_some())
    } else {
        true
    }
}

/// Why the helper could not answer.
#[derive(Debug)]
pub enum HelperError {
    NoDisplay,
    HelperNotFound,
    Spawn(std::io::Error),
    Failed(String),
    BadResponse(String),
}

impl std::fmt::Display for HelperError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoDisplay => write!(f, "there is no display to open a window on"),
            Self::HelperNotFound => write!(f, "the pulsar-dialog helper was not found"),
            Self::Spawn(error) => write!(f, "the pulsar-dialog helper did not start: {error}"),
            Self::Failed(why) => write!(f, "the pulsar-dialog helper failed: {why}"),
            Self::BadResponse(why) => write!(f, "the pulsar-dialog helper's answer was not understood: {why}"),
        }
    }
}

impl std::error::Error for HelperError {}

/// Ask through the helper and report why it could not, instead of falling back.
pub fn try_show(request: &DialogRequest) -> Result<DialogResponse, HelperError> {
    if !has_display() {
        return Err(HelperError::NoDisplay);
    }
    let helper = helper_path().ok_or(HelperError::HelperNotFound)?;
    let mut child = Command::new(helper)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(HelperError::Spawn)?;
    let json = serde_json::to_vec(request).map_err(|e| HelperError::BadResponse(e.to_string()))?;
    if let Some(mut stdin) = child.stdin.take() {
        // The helper reads to the end of its input, then draws.
        stdin.write_all(&json).map_err(|e| HelperError::Failed(e.to_string()))?;
    }
    let output = child.wait_with_output().map_err(|e| HelperError::Failed(e.to_string()))?;
    parse_helper_output(&output.stdout, output.status.success(), &output.stderr)
}

fn parse_helper_output(stdout: &[u8], success: bool, stderr: &[u8]) -> Result<DialogResponse, HelperError> {
    let text = String::from_utf8_lossy(stdout);
    // The response is the last JSON line; anything before it is log noise.
    let response = text
        .lines()
        .rev()
        .find_map(|line| serde_json::from_str::<DialogResponse>(line.trim()).ok());
    match response {
        Some(response) => Ok(response),
        None if !success => Err(HelperError::Failed(
            String::from_utf8_lossy(stderr).lines().last().unwrap_or("it exited with an error").to_string(),
        )),
        None => Err(HelperError::BadResponse(text.trim().chars().take(200).collect())),
    }
}

/// Show the dialog and return the answer.
///
/// The helper answers; if it cannot run, `native-fallback` shows the
/// platform's message box, and without that feature the answer is
/// [`Outcome::Unavailable`] (callers choose what that means: a prompt that
/// guards a download should read it as "no").
pub fn show(request: &DialogRequest) -> DialogResponse {
    match try_show(request) {
        Ok(response) => response,
        Err(error) => {
            eprintln!("[pulsar-dialog] {error}");
            fallback(request)
        }
    }
}

#[cfg(feature = "native-fallback")]
fn fallback(request: &DialogRequest) -> DialogResponse {
    use crate::Severity;
    if !has_display() {
        return DialogResponse::unavailable();
    }
    let buttons = request.effective_buttons();
    let level = match request.severity {
        Severity::Info | Severity::Question => rfd::MessageLevel::Info,
        Severity::Warning => rfd::MessageLevel::Warning,
        Severity::Error => rfd::MessageLevel::Error,
    };
    let mut text = request.message.clone();
    if let Some(detail) = &request.detail {
        text.push_str("\n\n");
        text.push_str(detail);
    }
    // A platform box has one, two or three standard buttons: use the two
    // that matter, the default and the cancel, or a lone OK.
    let (yes, no) = (request.default_button(), request.cancel_button());
    let dialog = rfd::MessageDialog::new()
        .set_title(&request.title)
        .set_description(&text)
        .set_level(level);
    match (yes, no, buttons.len()) {
        (Some(yes), Some(no), n) if n >= 2 && yes.id != no.id => {
            let answered = dialog.set_buttons(rfd::MessageButtons::YesNo).show();
            let id = if answered == rfd::MessageDialogResult::Yes { yes.id } else { no.id };
            DialogResponse::answered(id, None)
        }
        (Some(only), _, _) => {
            dialog.set_buttons(rfd::MessageButtons::Ok).show();
            DialogResponse::answered(only.id, None)
        }
        _ => DialogResponse::unavailable(),
    }
}

#[cfg(not(feature = "native-fallback"))]
fn fallback(_request: &DialogRequest) -> DialogResponse {
    DialogResponse::unavailable()
}

/// `true` when the response is silence: nothing showed the dialog.
pub fn is_unavailable(response: &DialogResponse) -> bool {
    response.outcome == Outcome::Unavailable
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_response_is_the_last_json_line_after_any_log_noise() {
        let stdout = b"INFO starting\n{\"outcome\":\"answered\",\"button\":\"yes\"}\n";
        let response = parse_helper_output(stdout, true, b"").unwrap();
        assert!(response.is_yes());
    }

    #[test]
    fn a_failed_helper_reports_its_last_stderr_line() {
        let error = parse_helper_output(b"", false, b"warming up\nno GPU adapter\n").unwrap_err();
        assert!(error.to_string().contains("no GPU adapter"), "{error}");
        assert!(matches!(
            parse_helper_output(b"garbage", true, b"").unwrap_err(),
            HelperError::BadResponse(_)
        ));
    }

    #[test]
    fn a_missing_helper_is_an_error_not_a_hang_or_a_panic() {
        // `try_show`, not `show`: with `native-fallback`, `show` would put a real
        // message box on the screen.
        std::env::set_var(HELPER_ENV, "definitely-not-a-real-helper-xyz");
        let result = try_show(&DialogRequest::new("t", "m").ok());
        std::env::remove_var(HELPER_ENV);
        assert!(matches!(
            result,
            Err(HelperError::Spawn(_) | HelperError::NoDisplay | HelperError::HelperNotFound)
        ));
    }

    #[test]
    fn silence_is_recognised() {
        assert!(is_unavailable(&DialogResponse::unavailable()));
        assert!(!is_unavailable(&DialogResponse::answered("ok", None)));
    }
}
