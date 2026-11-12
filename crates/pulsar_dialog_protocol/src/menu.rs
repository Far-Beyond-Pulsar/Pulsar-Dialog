//! Context menus: the request shape, agreed ahead of the renderer.
//!
//! Not drawn yet: the `pulsar-dialog` helper answers a menu request with
//! `item: None` until the menu window lands. The types are here so callers and
//! the renderer are written against one shape.

use crate::ThemeHint;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MenuItem {
    Item {
        id: String,
        label: String,
        /// An icon name from the UI framework's set.
        #[serde(default)]
        icon: Option<String>,
        /// Display text only (`Ctrl+S`); the caller binds the key.
        #[serde(default)]
        shortcut: Option<String>,
        #[serde(default = "enabled")]
        enabled: bool,
        /// Drawn with a check mark.
        #[serde(default)]
        checked: bool,
    },
    Separator,
    Submenu {
        label: String,
        items: Vec<MenuItem>,
    },
}

fn enabled() -> bool {
    true
}

impl MenuItem {
    pub fn item(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::Item {
            id: id.into(),
            label: label.into(),
            icon: None,
            shortcut: None,
            enabled: true,
            checked: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextMenuRequest {
    pub items: Vec<MenuItem>,
    /// Where to open it, in screen pixels.
    pub position: (i32, i32),
    #[serde(default)]
    pub theme: Option<ThemeHint>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextMenuResponse {
    /// The chosen item's id; `None` when the menu was dismissed.
    pub item: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_default_to_enabled_and_round_trip() {
        let request = ContextMenuRequest {
            items: vec![
                MenuItem::item("open", "Open"),
                MenuItem::Separator,
                MenuItem::Submenu { label: "More".into(), items: vec![MenuItem::item("x", "X")] },
            ],
            position: (10, 20),
            theme: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert_eq!(serde_json::from_str::<ContextMenuRequest>(&json).unwrap(), request);
        let item: MenuItem = serde_json::from_str(r#"{"kind":"item","id":"a","label":"A"}"#).unwrap();
        assert!(matches!(item, MenuItem::Item { enabled: true, checked: false, .. }));
    }
}
