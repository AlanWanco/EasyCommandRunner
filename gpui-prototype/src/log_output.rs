//! Default tail-follow, paused by manual upward scrolling until the tail button is used.
use gpui::component::input::TextareaState;
use gpui::{
    canvas, point, prelude::*, px, App, DispatchPhase, Entity, EntityId, HitboxBehavior, Pixels,
    Point, ScrollWheelEvent, Size, Window,
};
use std::{cell::Cell, rc::Rc};

type Viewport = (Size<Pixels>, Pixels);

pub(crate) struct LogAutoScroll {
    following: Rc<Cell<bool>>,
    paused_offset: Rc<Cell<Point<Pixels>>>,
    pending: Rc<Cell<bool>>,
    viewport: Rc<Cell<Option<Viewport>>>,
    observed_offset: Rc<Cell<Option<Point<Pixels>>>>,
}

impl Default for LogAutoScroll {
    fn default() -> Self {
        Self {
            following: Rc::new(Cell::new(true)),
            paused_offset: Rc::new(Cell::new(point(px(0.), px(0.)))),
            pending: Rc::new(Cell::new(false)),
            viewport: Rc::new(Cell::new(None)),
            observed_offset: Rc::new(Cell::new(None)),
        }
    }
}

impl LogAutoScroll {
    pub fn new_view(&self) -> Self {
        Self {
            following: self.following.clone(),
            paused_offset: self.paused_offset.clone(),
            ..Self::default()
        }
    }

    pub fn shares_follow(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.following, &other.following)
    }

    pub fn is_following(&self) -> bool {
        self.following.get()
    }

    pub fn reopen(&self) {
        self.viewport.set(None);
        self.observed_offset.set(None);
    }

    pub fn request_tail(&self) {
        self.following.set(true);
        self.pending.set(true);
    }

    fn detect_upward_scroll(&self, output: &TextareaState) -> bool {
        let metrics = output
            .line_height()
            .map(|height| (output.input_bounds().size, height));
        let moved_up = !self.pending.get()
            && self.viewport.get() == metrics
            && self
                .observed_offset
                .get()
                .is_some_and(|old| output.scroll_offset().y > old.y + px(0.5));
        if moved_up {
            return self.following.replace(false);
        }
        false
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
        self.detect_upward_scroll(output.read(cx));
        let following = self.is_following();
        if following {
            self.pending.set(true);
        }
        output.update(cx, |state, cx| {
            let offset = state.scroll_offset();
            let selected = state.selected_range();
            state.set_value(value, window, cx);
            if !following {
                state.set_selected_range(selected, cx);
            }
            // set_value resets to the top. Preserve the old viewport through layout,
            // then follow the tail only when the user has not paused it.
            state.set_scroll_offset(offset, cx);
        });
    }

    /// Paint after the textarea, when wrapping and scroll extent are available.
    /// The capture listener observes only this viewport and never consumes input.
    pub fn after_layout(
        &self,
        output: &Entity<TextareaState>,
        owner: EntityId,
    ) -> impl IntoElement {
        let following = self.following.clone();
        let paused_offset = self.paused_offset.clone();
        let pending = self.pending.clone();
        let viewport = self.viewport.clone();
        let observed_offset = self.observed_offset.clone();
        let output = output.clone();
        canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |_, hitbox, window, cx| {
                let mode = following.clone();
                let pending_scroll = pending.clone();
                let state = output.clone();
                window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, cx| {
                    if phase == DispatchPhase::Capture
                        && hitbox.should_handle_scroll(window)
                        && event
                            .delta
                            .pixel_delta(state.read(cx).line_height().unwrap_or(px(20.)))
                            .y
                            > px(0.)
                        && state.read(cx).scroll_offset().y < px(0.)
                    {
                        let changed = mode.replace(false);
                        pending_scroll.set(false);
                        // A tail request from the preceding paint may still be deferred.
                        // User input takes precedence over that queued cursor reveal.
                        state.update(cx, |state, cx| {
                            let delta = event
                                .delta
                                .pixel_delta(state.line_height().unwrap_or(px(20.)));
                            let mut offset = state.scroll_offset() + delta;
                            offset.x = offset.x.min(px(0.));
                            offset.y = offset.y.min(px(0.));
                            state.set_scroll_offset(offset, cx);
                        });
                        if changed {
                            cx.notify(owner);
                        }
                    }
                });
                let state = output.read(cx);
                let Some(line_height) = state.line_height() else {
                    return;
                };
                let metrics = (state.input_bounds().size, line_height);
                let previous = viewport.replace(Some(metrics));
                let offset = state.scroll_offset();
                // Scrollbar dragging and keyboard scrolling do not produce wheel events.
                if previous == Some(metrics)
                    && !pending.get()
                    && observed_offset
                        .get()
                        .is_some_and(|old| offset.y > old.y + px(0.5))
                    && following.replace(false)
                {
                    cx.notify(owner);
                }
                observed_offset.set(Some(offset));
                let should_follow = pending.replace(false) || previous != Some(metrics);
                if following.get() && should_follow {
                    output.update(cx, |state, cx| {
                        let end = state.value().len();
                        state.set_selected_range(end..end, cx);
                    });
                } else if !following.get() {
                    if previous.is_none() {
                        output.update(cx, |state, cx| {
                            state.set_scroll_offset(paused_offset.get(), cx)
                        });
                    } else {
                        paused_offset.set(offset);
                    }
                }
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    }
}
