//! Second window is only a view of CommandWorkspace's session logs. It owns no run handles.
use crate::{
    app::{log_zoom_delta, CommandWorkspace},
    components::{button, column, frame, icon_button, row, textarea},
    i18n::{tr, Language},
    state::LogItems,
    theme::{self, palette, Fonts},
    tokens::{CONTROL, GAP, LINE, SECTION},
};
use gpui::assets::IconName;
use gpui::component::{
    input::TextareaState,
    select::{Select, SelectEvent, SelectState},
    Disableable, IndexPath, Sizable, Size,
};
use gpui::{
    prelude::*, px, rgb, AppContext, Context, Entity, Focusable, Render, Subscription, Window,
};

pub(crate) struct DetachedLogWindow {
    workspace: Entity<CommandWorkspace>,
    selector: Entity<SelectState<LogItems>>,
    pub(crate) output: Entity<TextareaState>,
    last_items: Vec<(usize, String)>,
    last_selected: Option<usize>,
    language: Language,
    _subscriptions: Vec<Subscription>,
}

impl DetachedLogWindow {
    pub fn new(
        workspace: Entity<CommandWorkspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let snapshot = workspace.read(cx).log_snapshot();
        let selected = snapshot
            .selected_id
            .and_then(|id| snapshot.items.iter().position(|item| item.id == id));
        let last_items = snapshot
            .items
            .iter()
            .map(|item| (item.id, item.label.to_string()))
            .collect();
        let selector = cx.new(|cx| {
            SelectState::new(
                LogItems(snapshot.items),
                selected.map(IndexPath::new),
                window,
                cx,
            )
        });
        let output = cx.new(|cx| {
            TextareaState::new(window, cx)
                .default_value(snapshot.output)
                .placeholder(tr(cx, "暂无运行记录。"))
        });
        let observer = cx.observe_in(&workspace, window, |this, _, window, cx| {
            this.sync(window, cx)
        });
        let subscription = cx.subscribe_in(&selector, window, |this, _, event, window, cx| {
            if let SelectEvent::Confirm(Some(id)) = event {
                this.workspace
                    .update(cx, |workspace, cx| workspace.select_log(*id, window, cx));
            }
        });
        Self {
            workspace,
            selector,
            output,
            last_items,
            last_selected: snapshot.selected_id,
            language: *cx.global::<Language>(),
            _subscriptions: vec![observer, subscription],
        }
    }

    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let language = *cx.global::<Language>();
        if self.language != language {
            self.language = language;
            window.set_window_title(&tr(cx, "运行日志 - EasyCommandRunner"));
            self.output.update(cx, |output, cx| {
                output.set_placeholder(tr(cx, "暂无运行记录。"), window, cx)
            });
        }
        let snapshot = self.workspace.read(cx).log_snapshot();
        let items: Vec<_> = snapshot
            .items
            .iter()
            .map(|item| (item.id, item.label.to_string()))
            .collect();
        if self.last_items != items || self.last_selected != snapshot.selected_id {
            let selected = snapshot
                .selected_id
                .and_then(|id| snapshot.items.iter().position(|item| item.id == id));
            self.selector.update(cx, |selector, cx| {
                selector.set_items(LogItems(snapshot.items), window, cx);
                selector.set_selected_index(selected.map(IndexPath::new), window, cx);
            });
            self.last_items = items;
            self.last_selected = snapshot.selected_id;
        }
        if self.output.read(cx).value().as_ref() != snapshot.output {
            self.output.update(cx, |output, cx| {
                output.set_value(snapshot.output, window, cx)
            });
        }
        cx.notify();
    }
}

impl Render for DetachedLogWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let snapshot = self.workspace.read(cx).log_snapshot();
        let p = palette(cx);
        let reattach = self.workspace.clone();
        let stop = self.workspace.clone();
        let delete = self.workspace.clone();
        frame("detached-log-window")
            .size_full()
            .bg(rgb(p.panel))
            .text_color(rgb(p.text))
            .font_family(cx.global::<Fonts>().body.clone())
            .text_size(px(theme::font_size(cx)))
            .p(px(SECTION))
            .child(
                column()
                    .gap(px(GAP))
                    .size_full()
                    .child(row().h(px(CONTROL)).child(tr(cx, "运行日志")))
                    .child(
                        row()
                            .h(px(CONTROL))
                            .child(
                                frame("detached-log-select")
                                    .flex_1()
                                    .min_w_0()
                                    .h(px(CONTROL))
                                    .child(
                                        Select::new(&self.selector)
                                            .id("detached-log-selector")
                                            .accessibility_label(tr(cx, "选择运行记录"))
                                            .placeholder(tr(cx, "尚无运行记录"))
                                            .with_size(Size::Medium)
                                            .w_full()
                                            .h(px(CONTROL))
                                            .disabled(snapshot.items.is_empty()),
                                    ),
                            )
                            .child(
                                icon_button(
                                    "detached-stop-log",
                                    "停止当前运行",
                                    IconName::Square,
                                    cx,
                                )
                                .disabled(!snapshot.can_stop)
                                .on_click(move |_, _, cx| {
                                    stop.update(cx, |workspace, cx| {
                                        workspace.stop_selected_run(cx)
                                    });
                                }),
                            )
                            .child(
                                icon_button(
                                    "detached-delete-log",
                                    "删除当前运行记录",
                                    IconName::Trash,
                                    cx,
                                )
                                .disabled(!snapshot.can_delete)
                                .on_click(move |_, window, cx| {
                                    delete.update(cx, |workspace, cx| {
                                        workspace.delete_selected_log(window, cx)
                                    });
                                }),
                            )
                            .child(button("reattach-log", "停靠", None, cx).on_click(
                                move |_, window, cx| {
                                    reattach.update(cx, |workspace, cx| workspace.reattach_log(cx));
                                    window.remove_window();
                                },
                            )),
                    )
                    .child(
                        frame("detached-log-output")
                            .flex_1()
                            .min_h_0()
                            .capture_key_down(cx.listener(
                                |view, event: &gpui::KeyDownEvent, window, cx| {
                                    if !view.output.focus_handle(cx).is_focused(window) {
                                        return;
                                    }
                                    let Some(delta) = log_zoom_delta(event) else {
                                        return;
                                    };
                                    view.workspace
                                        .update(cx, |workspace, cx| workspace.zoom_log(delta, cx));
                                    cx.stop_propagation();
                                },
                            ))
                            .child(
                                textarea(&self.output, "运行输出", true, cx)
                                    .text_size(px(snapshot.font_size as f32))
                                    .line_height(px((snapshot.font_size as f32 * 1.4).max(LINE)))
                                    .readonly(true)
                                    .bg(rgb(p.subtle)),
                            ),
                    ),
            )
    }
}
