//! Which theme a dialog wears.
//!
//! The user's saved Pulsar theme (`state.json` in the per-user app-data
//! directory, themes in its `themes/` folder: the same files the engine and
//! the Hub use), or dark when there is none or it cannot be found. A request
//! may name its own via [`ThemeHint`].
//!
//! This only reads. It never writes `state.json` or the themes folder, so a
//! dialog cannot change what the user chose.

use gpui::{App, SharedString, WindowAppearance};
use pulsar_dialog_protocol::ThemeHint;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use ui::{Theme, ThemeRegistry};

/// The per-user app-data directory (the layout `pulsar_install_layout` in
/// Pulsar-Native defines: `com` / `Pulsar` / `Pulsar_Engine`).
pub fn appdata_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("com", "Pulsar", "Pulsar_Engine")
        .map(|dirs| dirs.data_dir().to_path_buf())
}

#[derive(Deserialize, Default)]
struct SavedState {
    theme: Option<String>,
}

/// The theme name in `state.json`, if there is one.
pub fn saved_theme_name(state_file: &Path) -> Option<String> {
    let text = std::fs::read_to_string(state_file).ok()?;
    serde_json::from_str::<SavedState>(&text)
        .ok()?
        .theme
        .filter(|name| !name.trim().is_empty())
}

/// The theme name and themes directory to use: the hint's, else the saved.
pub fn resolve(hint: Option<&ThemeHint>) -> (Option<String>, Option<PathBuf>) {
    let appdata = appdata_dir();
    let name = hint
        .and_then(|hint| hint.name.clone())
        .or_else(|| appdata.as_ref().and_then(|dir| saved_theme_name(&dir.join("state.json"))));
    let dir = hint
        .and_then(|hint| hint.themes_dir.clone())
        .or_else(|| appdata.map(|dir| dir.join("themes")));
    (name, dir)
}

/// Dress the app: dark first (the fallback), then the saved theme over it if
/// it is among the themes in the folder.
pub fn apply(hint: Option<&ThemeHint>, cx: &mut App) {
    Theme::change(WindowAppearance::Dark, None, cx);
    let (Some(name), Some(dir)) = resolve(hint) else {
        return;
    };
    if !dir.is_dir() {
        tracing::debug!("no themes directory at {}; using the dark theme", dir.display());
        return;
    }
    let wanted = SharedString::from(name.clone());
    let applied = ThemeRegistry::watch_dir(dir, cx, move |cx| {
        let Some(config) = ThemeRegistry::global(cx).themes().get(&wanted).cloned() else {
            tracing::debug!("theme {wanted} not found; using the dark theme");
            return;
        };
        Theme::change(config.mode, None, cx);
        Theme::global_mut(cx).apply_config(&config);
    });
    if let Err(error) = applied {
        tracing::warn!("could not read the themes directory: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_saved_theme_is_read_and_a_missing_or_broken_file_is_none() {
        let dir = std::env::temp_dir().join(format!("pulsar-dialog-theme-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let state = dir.join("state.json");
        std::fs::write(&state, r#"{"theme":"Tokyo Night","scrollbar_show":"Scrolling"}"#).unwrap();
        assert_eq!(saved_theme_name(&state).as_deref(), Some("Tokyo Night"));
        std::fs::write(&state, "not json").unwrap();
        assert_eq!(saved_theme_name(&state), None);
        std::fs::write(&state, r#"{"theme":"  "}"#).unwrap();
        assert_eq!(saved_theme_name(&state), None);
        assert_eq!(saved_theme_name(&dir.join("missing.json")), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_hint_wins_over_the_saved_theme() {
        let hint = ThemeHint { name: Some("Forced".into()), themes_dir: Some(PathBuf::from("x")) };
        let (name, dir) = resolve(Some(&hint));
        assert_eq!(name.as_deref(), Some("Forced"));
        assert_eq!(dir, Some(PathBuf::from("x")));
    }
}
