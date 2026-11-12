//! Opening a dialog: in a GPUI app that exists, or in a new one that lives
//! exactly as long as the dialog.

use gpui::*;
use pulsar_dialog_protocol::{DialogRequest, DialogResponse};
use std::sync::{Arc, Mutex};
use ui::Root;

use crate::{layout, theme, view::DialogView};

/// Open a dialog window in a running app (the dialog wears the app's theme).
/// `on_done` is called once with the answer, however the window closes.
pub fn open_dialog(
    request: DialogRequest,
    cx: &mut App,
    on_done: impl FnOnce(DialogResponse, &mut App) + 'static,
) -> anyhow::Result<AnyWindowHandle> {
    let size = size(px(layout::WIDTH), px(layout::height(&request)));
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size, cx))),
        // The title names the window in the taskbar and the window switcher.
        titlebar: Some(TitlebarOptions {
            title: Some(SharedString::from(request.title.clone())),
            ..ui::TitleBar::title_bar_options()
        }),
        window_decorations: Some(WindowDecorations::Client),
        window_min_size: Some(size),
        is_resizable: false,
        kind: WindowKind::Normal,
        ..Default::default()
    };
    let on_done: crate::view::OnDone = Box::new(on_done);
    let on_done = std::cell::RefCell::new(Some(on_done));
    let handle = cx.open_window(options, move |window, cx| {
        let on_done = on_done.borrow_mut().take().expect("the window builds once");
        let dialog = cx.new(|cx| DialogView::new(request, on_done, window, cx));
        cx.new(|cx| Root::new(dialog.into(), window, cx))
    })?;
    Ok(handle.into())
}

/// Show one dialog in its own GPUI app and return the answer. Blocks until
/// the dialog is closed. Meant for a process of its own (the helper): GPUI
/// runs one app per process.
pub fn run_blocking(request: DialogRequest) -> DialogResponse {
    // A window closed by any means counts as dismissed until a button says more.
    let answer = Arc::new(Mutex::new(DialogResponse::dismissed(None)));
    let slot = answer.clone();
    Application::new().with_assets(ui::Assets).run(move |cx| {
        ui::init(cx);
        theme::apply(request.theme.as_ref(), cx);
        cx.activate(true);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let opened = open_dialog(request, cx, move |response, _| {
            *slot.lock().unwrap() = response;
        });
        if let Err(error) = opened {
            tracing::error!("could not open the dialog window: {error}");
            cx.quit();
        }
    });
    let response = answer.lock().unwrap().clone();
    response
}
