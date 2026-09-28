//! Emoji-style icon browser: only visible rows are built, names live in tooltips/search.
use crate::{
    app::CommandWorkspace,
    components::*,
    i18n::{self, tr},
    tab_icons::{self, Category},
    theme::palette,
    tokens::*,
};
use gpui::component::{
    input::{InputEvent, InputState},
    tooltip::Tooltip,
    WindowExt,
};
use gpui::{
    prelude::*, px, rgb, uniform_list, AppContext, Context, Entity, FocusHandle, Render,
    ScrollStrategy, Subscription, UniformListScrollHandle, WeakEntity, Window,
};
use std::rc::Rc;

const COLUMNS: usize = 10;
const CELL: f32 = 44.;

pub struct IconPicker {
    owner: WeakEntity<CommandWorkspace>,
    tab_id: usize,
    current: String,
    search: Entity<InputState>,
    source: Entity<InputState>,
    category: Category,
    results: Rc<Vec<usize>>,
    scroll: UniformListScrollHandle,
    focus: FocusHandle,
    cursor: usize,
    _subscription: Subscription,
}
impl IconPicker {
    pub fn new(
        owner: WeakEntity<CommandWorkspace>,
        tab_id: usize,
        current: String,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(w, cx).placeholder(tr(cx, "搜索图标…")));
        let source = cx.new(|cx| {
            InputState::new(w, cx)
                .default_value(if current.starts_with("builtin:") {
                    String::new()
                } else {
                    current.clone()
                })
                .placeholder(tr(cx, "本地 SVG 路径或 HTTPS SVG 地址"))
        });
        let subscription = cx.subscribe_in(&search, w, |this, _, event, _, cx| {
            if let InputEvent::Change = event {
                this.filter(cx);
            }
        });
        Self {
            owner,
            tab_id,
            current,
            search,
            source,
            category: Category::All,
            results: Rc::new(tab_icons::search("", Category::All)),
            scroll: UniformListScrollHandle::new(),
            focus: cx.focus_handle(),
            cursor: 0,
            _subscription: subscription,
        }
    }
    fn filter(&mut self, cx: &mut Context<Self>) {
        self.results = Rc::new(tab_icons::search(
            self.search.read(cx).value().as_ref(),
            self.category,
        ));
        self.cursor = 0;
        self.scroll.scroll_to_item_strict(0, ScrollStrategy::Top);
        cx.notify();
    }
    // Dialog owns the Enter/Confirm key binding; handle it here rather than relying on raw key-down.
    pub fn confirm_selection(&self, w: &mut Window, cx: &mut Context<Self>) -> bool {
        use gpui::Focusable as _;
        if !(self.focus.is_focused(w) || self.search.focus_handle(cx).is_focused(w)) {
            return false;
        }
        let Some(index) = self.results.get(self.cursor) else {
            return false;
        };
        self.owner
            .update(cx, |v, cx| {
                v.set_tab_icon_from_source(
                    self.tab_id,
                    format!("builtin:{}", tab_icons::CATALOG[*index].key),
                    cx,
                )
            })
            .is_ok()
    }

    fn choose(&self, source: String, w: &mut Window, cx: &mut Context<Self>) {
        let _ = self.owner.update(cx, |v, cx| {
            v.set_tab_icon_from_source(self.tab_id, source, cx)
        });
        w.close_dialog(cx);
    }
}
impl Render for IconPicker {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let mut categories = row().gap(px(4.));
        for (ix, category) in Category::ALL.into_iter().enumerate() {
            categories = categories.child(
                icon_button(
                    format!("icon-category-{ix}"),
                    category.label(),
                    category.icon(),
                    cx,
                )
                .when(self.category == category, |b| b.bg(rgb(p.selection)))
                .on_click(cx.listener(move |v, _, _, cx| {
                    v.category = category;
                    v.filter(cx);
                })),
            );
        }
        let results = self.results.clone();
        let owner = self.owner.clone();
        let tab_id = self.tab_id;
        let current = self.current.clone();
        let cursor = self.cursor;
        let grid_focused = self.focus.is_focused(w);
        let height = (f32::from(w.viewport_size().height) - 350.).clamp(132., 264.);
        let grid = uniform_list(
            "icon-grid-list",
            results.len().div_ceil(COLUMNS),
            move |range, _, _| {
                range
                    .map(|row_index| {
                        let mut row = row().gap_0().h(px(CELL));
                        for position in
                            row_index * COLUMNS..((row_index + 1) * COLUMNS).min(results.len())
                        {
                            let icon = &tab_icons::CATALOG[results[position]];
                            let source = format!("builtin:{}", icon.key);
                            let selected = current == source;
                            let name = icon.name.clone();
                            let target = owner.clone();
                            row = row.child(
                                frame(format!("icon-choice-{}", icon.key))
                                    .role(gpui::Role::Button)
                                    .aria_label(name.clone())
                                    .size(px(CELL))
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(RADIUS))
                                    .cursor(gpui::CursorStyle::PointingHand)
                                    .border_1()
                                    .border_color(
                                        if selected || (grid_focused && position == cursor) {
                                            rgb(p.focus).into()
                                        } else {
                                            gpui::transparent_black()
                                        },
                                    )
                                    .hover(|s| s.bg(rgb(p.hover)))
                                    .tooltip(move |w, cx| Tooltip::new(name.clone()).build(w, cx))
                                    .child(
                                        frame(format!("icon-choice-symbol-{}", icon.key))
                                            .size(px(24.))
                                            .child(
                                                gpui::svg()
                                                    .data(
                                                        tab_icons::builtin(&icon.key)
                                                            .expect("catalog icon"),
                                                    )
                                                    .size(px(24.))
                                                    .text_color(rgb(p.text)),
                                            ),
                                    )
                                    .on_click(move |_, w, cx| {
                                        let _ = target.update(cx, |v, cx| {
                                            v.set_tab_icon_from_source(tab_id, source.clone(), cx)
                                        });
                                        w.close_dialog(cx);
                                    }),
                            );
                        }
                        row
                    })
                    .collect::<Vec<_>>()
            },
        )
        .track_scroll(&self.scroll)
        .h(px(height))
        .w_full();
        let count = i18n::format(cx, "{} 个图标", &[&self.results.len().to_string()]);
        column()
            .capture_key_down(cx.listener(|v, event: &gpui::KeyDownEvent, w, cx| {
                use gpui::Focusable as _;
                if v.search.focus_handle(cx).is_focused(w) && event.keystroke.key == "down" {
                    v.focus.focus(w, cx);
                    cx.stop_propagation();
                    cx.notify();
                    return;
                }
                if !v.focus.is_focused(w) {
                    return;
                }
                let next = match event.keystroke.key.as_str() {
                    "left" => v.cursor.saturating_sub(1),
                    "right" => v.cursor + 1,
                    "up" => v.cursor.saturating_sub(COLUMNS),
                    "down" => v.cursor + COLUMNS,
                    "home" => 0,
                    "end" => v.results.len().saturating_sub(1),
                    "space" => {
                        if let Some(index) = v.results.get(v.cursor) {
                            v.choose(format!("builtin:{}", tab_icons::CATALOG[*index].key), w, cx);
                        }
                        cx.stop_propagation();
                        return;
                    }
                    _ => return,
                };
                v.cursor = next.min(v.results.len().saturating_sub(1));
                v.scroll
                    .scroll_to_item(v.cursor / COLUMNS, ScrollStrategy::Nearest);
                cx.stop_propagation();
                cx.notify();
            }))
            .gap(px(GAP))
            .text_color(rgb(p.text))
            .child(
                row()
                    .child(gpui::div().flex_1().min_w_0().child(input(
                        "icon-search",
                        &self.search,
                        "搜索图标…",
                        false,
                        cx,
                    )))
                    .child(
                        icon_button(
                            "icon-choice-default",
                            "默认图标",
                            gpui::assets::IconName::RotateCcw,
                            cx,
                        )
                        .on_click(cx.listener(|v, _, w, cx| v.choose(String::new(), w, cx))),
                    ),
            )
            .child(categories)
            .child(
                frame("icon-grid")
                    .track_focus(&self.focus)
                    .w_full()
                    .h(px(height))
                    .overflow_hidden()
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|v, _, w, cx| v.focus.focus(w, cx)),
                    )
                    .when(self.results.is_empty(), |d| {
                        d.child(gpui::div().p(px(SECTION)).child(tr(cx, "没有匹配的图标")))
                    })
                    .when(!self.results.is_empty(), |d| d.child(grid)),
            )
            .child(
                gpui::div()
                    .text_size(px(SMALL))
                    .text_color(rgb(p.muted))
                    .child(count),
            )
            .child(
                row()
                    .child(gpui::div().flex_1().min_w_0().child(input(
                        "icon-source",
                        &self.source,
                        "SVG 路径或网址",
                        true,
                        cx,
                    )))
                    .child(
                        icon_button(
                            "icon-source-apply",
                            "使用此 SVG",
                            gpui::assets::IconName::Check,
                            cx,
                        )
                        .on_click(cx.listener(|v, _, w, cx| {
                            v.choose(v.source.read(cx).value().to_string(), w, cx)
                        })),
                    ),
            )
    }
}
