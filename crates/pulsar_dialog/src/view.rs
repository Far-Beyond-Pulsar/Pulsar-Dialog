//! The dialog as a GPUI view: a title bar, a severity icon, the message and
//! its detail, an optional checkbox, and the buttons.

use gpui::prelude::*;
use gpui::*;
use pulsar_dialog_protocol::{ButtonKind, DialogRequest, DialogResponse, Severity};
use ui::{
    ActiveTheme as _, Icon, IconName, TitleBar,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex, v_flex,
};

/// Called once, with the answer.
pub type OnDone = Box<dyn FnOnce(DialogResponse, &mut App)>;

/// What a closed event means to an observer that wants it as an event.
#[derive(Clone, Debug)]
pub struct DialogDone(pub DialogResponse);

pub struct DialogView {
    request: DialogRequest,
    focus: FocusHandle,
    checkbox: Option<bool>,
    on_done: Option<OnDone>,
}

impl EventEmitter<DialogDone> for DialogView {}

impl DialogView {
    pub fn new(
        request: DialogRequest,
        on_done: OnDone,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        // Closing the window by any route (the title bar's X, the platform's
        // own close button, Alt+F4) is an answer too.
        let weak = cx.entity().downgrade();
        window.on_window_should_close(cx, move |_, cx| {
            let _ = weak.update(cx, |this, cx| this.complete(this.close_response(), cx));
            true
        });
        Self {
            checkbox: request.checkbox.as_ref().map(|option| option.checked),
            request,
            focus,
            on_done: Some(on_done),
        }
    }

    /// What closing the window answers: the cancel button, else "dismissed".
    fn close_response(&self) -> DialogResponse {
        match self.request.cancel_button() {
            Some(button) => DialogResponse::answered(button.id, self.checkbox),
            None => DialogResponse::dismissed(self.checkbox),
        }
    }

    /// Report the answer, once.
    fn complete(&mut self, response: DialogResponse, cx: &mut Context<Self>) {
        let Some(on_done) = self.on_done.take() else {
            return;
        };
        cx.emit(DialogDone(response.clone()));
        on_done(response, cx);
    }

    fn press(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.complete(DialogResponse::answered(id, self.checkbox), cx);
        window.remove_window();
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.complete(self.close_response(), cx);
        window.remove_window();
    }

    fn severity_icon(&self, cx: &App) -> (IconName, Hsla) {
        let theme = cx.theme();
        match self.request.severity {
            Severity::Info | Severity::Question => (IconName::Info, theme.accent),
            Severity::Warning => (IconName::WarningTriangle, theme.warning),
            Severity::Error => (IconName::WarningTriangle, theme.danger),
        }
    }
}

impl Render for DialogView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (icon, accent) = self.severity_icon(cx);
        let request = self.request.clone();
        let checked = self.checkbox;
        let buttons = request.effective_buttons();

        v_flex()
            .id("pulsar-dialog")
            .key_context("PulsarDialog")
            .track_focus(&self.focus)
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "enter" => {
                        if let Some(button) = this.request.default_button() {
                            this.press(button.id, window, cx);
                        }
                    }
                    "escape" => this.close(window, cx),
                    _ => {}
                }
            }))
            .child(
                TitleBar::new()
                    .on_close_window(cx.listener(|this, _, window, cx| this.close(window, cx)))
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.foreground)
                            .child(request.title.clone()),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .px_5()
                    .pt_5()
                    .gap_4()
                    .child(
                        h_flex()
                            .gap_4()
                            .items_start()
                            .child(Icon::new(icon).size_8().text_color(accent))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(theme.foreground)
                                            .child(request.message.clone()),
                                    )
                                    .when_some(request.detail.clone(), |this, detail| {
                                        this.child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.muted_foreground)
                                                .child(detail),
                                        )
                                    }),
                            ),
                    )
                    .when_some(request.checkbox.clone().zip(checked), |this, (option, state)| {
                        this.child(
                            Checkbox::new("pulsar-dialog-checkbox")
                                .label(option.label)
                                .checked(state)
                                .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                    this.checkbox = Some(*checked);
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .child(
                h_flex()
                    .w_full()
                    .px_5()
                    .py_4()
                    .gap_2()
                    .justify_end()
                    .children(buttons.into_iter().map(|button| {
                        let id = button.id.clone();
                        let base = Button::new(SharedString::from(format!("dialog-button-{}", button.id)))
                            .label(button.label);
                        let styled = match button.kind {
                            ButtonKind::Primary => base.primary(),
                            ButtonKind::Danger => base.danger(),
                            ButtonKind::Secondary => base,
                        };
                        styled.on_click(cx.listener(move |this, _, window, cx| {
                            this.press(id.clone(), window, cx)
                        }))
                    })),
            )
    }
}
