//! Pulsar's dialogs, drawn with the engine's UI framework.
//!
//! * [`theme`]: which theme a dialog wears (the user's saved Pulsar theme, or
//!   dark when there is none).
//! * [`view::DialogView`]: the dialog itself, as a GPUI view.
//! * [`run::run_blocking`]: opens one dialog in its own GPUI app and returns
//!   the answer; what the `pulsar-dialog` helper does.
//! * [`run::open_dialog`]: opens one in an app that already exists (the editor).

pub mod layout;
pub mod run;
pub mod theme;
pub mod view;

pub use pulsar_dialog_protocol as protocol;
pub use run::{open_dialog, run_blocking};
pub use view::{DialogDone, DialogView};
