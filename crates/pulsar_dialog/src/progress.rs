//! The progress window: a heading, the current step, a bar, and, if the job
//! fails, the message and the buttons to choose from.

use gpui::prelude::*;
use gpui::*;
use pulsar_dialog_protocol::progress::{
    ProgressOutcome, ProgressRequest, ProgressResponse, ProgressUpdate,
};
use pulsar_dialog_protocol::{ButtonKind, DialogButton};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use ui::{
    ActiveTheme as _, TitleBar,
    button::{Button, ButtonVariants as _},
    h_flex,
    progress::Progress,
    v_flex,
};

use crate::theme;

const WIDTH: f32 = 620.0;
const HEIGHT: f32 = 280.0;

#[derive(Clone, Debug, Default)]
struct Failure {
    message: String,
    detail: Option<String>,
    buttons: Vec<DialogButton>,
}

#[derive(Clone, Debug, Default)]
struct State {
    heading: String,
    status: String,
    fraction: f32,
    failure: Option<Failure>,
}

type Answer = Arc<Mutex<ProgressResponse>>;

/// Fold one update from the host into what the window shows. `Some` ends the
/// window with that outcome.
fn apply(state: &mut State, update: ProgressUpdate) -> Option<ProgressOutcome> {
    match update {
        ProgressUpdate::Update { heading, status, fraction } => {
            if let Some(heading) = heading {
                state.heading = heading;
            }
            if let Some(status) = status {
                state.status = status;
            }
            if let Some(fraction) = fraction {
                state.fraction = fraction.clamp(0.0, 1.0);
            }
            None
        }
        ProgressUpdate::Failed { heading, message, detail, buttons } => {
            if let Some(heading) = heading {
                state.heading = heading;
            }
            // A failure with no way to answer would trap the user in the window.
            let buttons = if buttons.is_empty() {
                vec![DialogButton::new("close", "Close").kind(ButtonKind::Primary).default().cancel()]
            } else {
                buttons
            };
            state.failure = Some(Failure { message, detail, buttons });
            None
        }
        ProgressUpdate::Done => Some(ProgressOutcome::Finished),
    }
}

struct ProgressView {
    focus: FocusHandle,
    state: State,
    answer: Answer,
    title: SharedString,
    _poll: Task<()>,
}

impl ProgressView {
    fn new(
        request: ProgressRequest,
        updates: Receiver<ProgressUpdate>,
        answer: Answer,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let state = State {
            heading: request.heading.clone(),
            status: request.status.clone(),
            fraction: request.fraction.clamp(0.0, 1.0),
            failure: None,
        };
        let result = answer.clone();
        let poll = cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_millis(80)).await;
            let mut ended = None;
            let mut host_gone = false;
            loop {
                match updates.try_recv() {
                    Ok(update) => {
                        let outcome = this
                            .update(cx, |view, cx| {
                                cx.notify();
                                apply(&mut view.state, update)
                            })
                            .ok()
                            .flatten();
                        if outcome.is_some() {
                            ended = outcome;
                            break;
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        host_gone = true;
                        break;
                    }
                }
            }
            // The host went away while the job was running: nobody is left to
            // show progress for. After a failure the user still has buttons.
            if host_gone && ended.is_none() {
                let failing = this.update(cx, |view, _| view.state.failure.is_some()).unwrap_or(false);
                if !failing {
                    ended = Some(ProgressOutcome::Closed);
                }
            }
            if let Some(outcome) = ended {
                // A pressed button already set Answered; keep it.
                let mut current = result.lock().unwrap();
                if current.outcome == ProgressOutcome::Closed {
                    *current = ProgressResponse::new(outcome, None);
                }
                drop(current);
                let _ = cx.update(|cx| cx.quit());
                break;
            }
        });
        Self {
            focus,
            state,
            answer,
            title: SharedString::from(request.title),
            _poll: poll,
        }
    }

    fn press(&mut self, id: String, cx: &mut Context<Self>) {
        *self.answer.lock().unwrap() = ProgressResponse::new(ProgressOutcome::Answered, Some(id));
        cx.quit();
    }
}

impl Render for ProgressView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let state = self.state.clone();
        let failing = state.failure.is_some();

        v_flex()
            .id("pulsar-progress")
            .key_context("PulsarProgress")
            .track_focus(&self.focus)
            // After a failure Enter presses the default button and Escape the
            // cancel button, as in a dialog.
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                let Some(failure) = this.state.failure.clone() else {
                    return;
                };
                let chosen = match event.keystroke.key.as_str() {
                    "enter" => failure
                        .buttons
                        .iter()
                        .find(|button| button.default)
                        .or_else(|| failure.buttons.first()),
                    "escape" => failure.buttons.iter().find(|button| button.cancel),
                    _ => None,
                };
                if let Some(button) = chosen {
                    this.press(button.id.clone(), cx);
                }
            }))
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(
                TitleBar::new().child(
                    div()
                        .flex_1()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(self.title.clone()),
                ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .px_6()
                    .pt_6()
                    .gap_4()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(if failing { theme.danger } else { theme.foreground })
                            .child(state.heading.clone()),
                    )
                    .when_some(state.failure.clone(), |this, failure| {
                        this.child(
                            div()
                                .text_sm()
                                .text_color(theme.foreground)
                                .child(failure.message),
                        )
                        .when_some(failure.detail, |this, detail| {
                            this.child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(detail),
                            )
                        })
                    })
                    .when(!failing, |this| {
                        this.child(
                            div()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child(state.status.clone()),
                        )
                        .child(Progress::new().value(state.fraction * 100.0))
                    }),
            )
            .when_some(state.failure, |this, failure| {
                this.child(
                    h_flex()
                        .w_full()
                        .px_6()
                        .py_4()
                        .gap_2()
                        .justify_end()
                        .children(failure.buttons.into_iter().map(|button| {
                            let id = button.id.clone();
                            let base = Button::new(SharedString::from(format!("progress-{}", button.id)))
                                .label(button.label);
                            let styled = match button.kind {
                                ButtonKind::Primary => base.primary(),
                                ButtonKind::Danger => base.danger(),
                                ButtonKind::Secondary => base,
                            };
                            styled.on_click(cx.listener(move |this, _, _, cx| this.press(id.clone(), cx)))
                        })),
                )
            })
    }
}

/// Show a progress window in its own GPUI app, fed by `updates`, until the
/// job is done, fails and is answered, or the window is closed. Blocks. For a
/// process of its own (the helper).
pub fn run_progress_blocking(request: ProgressRequest, updates: Receiver<ProgressUpdate>) -> ProgressResponse {
    // Closed unless something says otherwise: closing the window is the default.
    let answer: Answer = Arc::new(Mutex::new(ProgressResponse::new(ProgressOutcome::Closed, None)));
    let shared = answer.clone();
    Application::new().with_assets(ui::Assets).run(move |cx| {
        ui::init(cx);
        theme::apply(request.theme.as_ref(), cx);
        cx.activate(true);
        let size = size(px(WIDTH), px(HEIGHT));
        let title = SharedString::from(request.title.clone());
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size, cx))),
            titlebar: Some(TitlebarOptions {
                title: Some(title),
                ..ui::TitleBar::title_bar_options()
            }),
            window_decorations: Some(WindowDecorations::Client),
            window_min_size: Some(size),
            is_resizable: false,
            kind: WindowKind::Normal,
            ..Default::default()
        };
        let closed = shared.clone();
        let opened = cx.open_window(options, move |window, cx| {
            window.on_window_should_close(cx, |_, _| true);
            let view = cx.new(|cx| ProgressView::new(request, updates, shared, window, cx));
            cx.new(|cx| ui::Root::new(view.into(), window, cx))
        });
        if let Err(error) = opened {
            tracing::error!("could not open the progress window: {error}");
            *closed.lock().unwrap() = ProgressResponse::new(ProgressOutcome::Unavailable, None);
            cx.quit();
            return;
        }
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
    });
    let response = answer.lock().unwrap().clone();
    response
}

#[cfg(test)]
mod tests {
    use super::{apply, State};
    use pulsar_dialog_protocol::progress::{ProgressOutcome, ProgressUpdate};
    use pulsar_dialog_protocol::DialogButton;

    #[test]
    fn updates_change_only_what_they_name_and_clamp_the_fraction() {
        let mut state = State { heading: "H".into(), status: "S".into(), fraction: 0.1, failure: None };
        let ended = apply(
            &mut state,
            ProgressUpdate::Update { heading: None, status: Some("T".into()), fraction: Some(7.0) },
        );
        assert_eq!(ended, None);
        assert_eq!((state.heading.as_str(), state.status.as_str(), state.fraction), ("H", "T", 1.0));
    }

    #[test]
    fn done_ends_the_window_and_a_failure_always_has_a_way_out() {
        let mut state = State::default();
        assert_eq!(apply(&mut state, ProgressUpdate::Done), Some(ProgressOutcome::Finished));
        let failed = ProgressUpdate::Failed {
            heading: Some("Failed".into()),
            message: "boom".into(),
            detail: None,
            buttons: Vec::new(),
        };
        assert_eq!(apply(&mut state, failed), None);
        assert_eq!(state.heading, "Failed");
        assert_eq!(state.failure.unwrap().buttons.len(), 1, "a Close button is added");
        let mut state = State::default();
        let with_buttons = ProgressUpdate::Failed {
            heading: None,
            message: "m".into(),
            detail: None,
            buttons: vec![DialogButton::new("a", "A"), DialogButton::new("b", "B")],
        };
        apply(&mut state, with_buttons);
        assert_eq!(state.failure.unwrap().buttons.len(), 2);
    }
}
