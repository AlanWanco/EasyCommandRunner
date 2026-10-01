//! Auto-follow for read-only log textareas, using their newly painted layout.
use gpui::component::input::TextareaState;
use gpui::{canvas, prelude::*, App, Context, Entity, Pixels, Size, Window};
use std::{cell::Cell, rc::Rc};

#[derive(Default)]
pub(crate) struct LogAutoScroll {
    pending: Rc<Cell<bool>>,
    viewport: Rc<Cell<Option<(Size<Pixels>, Pixels)>>>,
}

impl LogAutoScroll {
    pub fn request_tail(&self) {
        self.pending.set(true);
    }

    pub fn set_value(
        &self,
        output: &Entity<TextareaState>,
        value: String,
        window: &mut Window,
        cx: &mut App,
    ) {
        if output.read(cx).value().as_ref() == value {
            return; // Caption/status changes must not reset a manually browsed log.
        }
        self.request_tail();
        output.update(cx, |state, cx: &mut Context<TextareaState>| {
            let offset = state.scroll_offset();
            state.set_value(value, window, cx);
            // set_value resets to the top. Keep the old viewport until the new
            // wrapping and scroll extent are available, instead of flashing row 0.
            state.set_scroll_offset(offset, cx);
        });
    }

    /// Place this non-drawing canvas after the textarea so its paint has already
    /// committed the new scroll extent. No focus change or synthetic input is used.
    pub fn after_layout(&self, output: &Entity<TextareaState>) -> impl IntoElement {
        let pending = self.pending.clone();
        let viewport = self.viewport.clone();
        let output = output.clone();
        canvas(
            |_, _, _| {},
            move |_, _, _, cx| {
                let state = output.read(cx);
                let Some(line_height) = state.line_height() else {
                    return;
                };
                let metrics = (state.input_bounds().size, line_height);
                let resized = viewport.replace(Some(metrics)) != Some(metrics);
                if pending.replace(false) || resized {
                    output.update(cx, |state, cx| {
                        let end = state.value().len();
                        state.set_selected_range(end..end, cx);
                    });
                }
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    }
}
