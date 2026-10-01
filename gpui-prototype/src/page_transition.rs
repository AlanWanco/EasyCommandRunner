//! Short-lived, non-interactive page visuals. Live editors retain their original entities and hitboxes.
use crate::{
    components::{column, field, row},
    i18n::{self, tr, Language},
    state::TabData,
    theme::{self, palette, Fonts},
    tokens::*,
};
use gpui::{
    assets::IconName, component::Icon, div, prelude::*, px, rgb, AnyElement, App, Div, FontWeight,
};
use std::{
    rc::Rc,
    time::{Duration, Instant},
};

pub const DURATION: Duration = Duration::from_millis(260);

pub struct PageTransition {
    pub started_at: Instant,
    pub outgoing: Rc<PageVisual>,
    pub incoming: Option<Rc<PageSnapshot>>,
}

pub enum PageVisual {
    Empty,
    Page(Rc<PageSnapshot>),
    Limited {
        incoming: Rc<PageSnapshot>,
        opacity: f32,
        offset: f32,
    },
    Composite {
        outgoing: Rc<PageVisual>,
        incoming: Rc<PageSnapshot>,
        direction: i8,
        progress: f32,
        depth: usize,
    },
}

impl PageTransition {
    pub fn freeze(&self, direction: i8, progress: f32) -> Rc<PageVisual> {
        let Some(incoming) = &self.incoming else {
            return self.outgoing.clone(); // A target not painted yet never needs an outgoing copy.
        };
        let depth = match self.outgoing.as_ref() {
            PageVisual::Empty | PageVisual::Page(_) | PageVisual::Limited { .. } => 0,
            PageVisual::Composite { depth, .. } => *depth,
        };
        if progress >= 1. {
            return Rc::new(PageVisual::Page(incoming.clone()));
        }
        if depth >= 8 {
            // Drop old nested visuals under extreme repeat clicks, without restoring
            // a fully bright destination before the fade-in has reached it.
            return Rc::new(PageVisual::Limited {
                incoming: incoming.clone(),
                opacity: opacities(progress).1,
                offset: offsets(1., direction, progress).1,
            });
        }
        Rc::new(PageVisual::Composite {
            outgoing: self.outgoing.clone(),
            incoming: incoming.clone(),
            direction,
            progress,
            depth: depth + 1,
        })
    }
}

impl PageVisual {
    pub fn render(&self, width: f32, compact: bool, compact_actions: bool, cx: &App) -> AnyElement {
        match self {
            Self::Empty => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(rgb(palette(cx).panel))
                .text_color(rgb(palette(cx).muted))
                .child(tr(cx, "从左侧配置列表打开一个标签，或新建配置"))
                .into_any_element(),
            Self::Page(page) => div()
                .id(("page-visual", page.tab_id))
                .size_full()
                .child(page.render(width, compact, compact_actions, cx))
                .into_any_element(),
            Self::Limited {
                incoming,
                opacity,
                offset,
            } => div()
                .relative()
                .size_full()
                .overflow_hidden()
                .bg(rgb(palette(cx).panel))
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(px(offset * width))
                        .w(px(width))
                        .opacity(*opacity)
                        .overflow_hidden()
                        .child(incoming.render(width, compact, compact_actions, cx)),
                )
                .into_any_element(),
            Self::Composite {
                outgoing,
                incoming,
                direction,
                progress,
                ..
            } => {
                let (old_x, new_x) = offsets(width, *direction, *progress);
                let (old_alpha, new_alpha) = opacities(*progress);
                div()
                    .relative()
                    .size_full()
                    .overflow_hidden()
                    .bg(rgb(palette(cx).panel))
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left(px(old_x))
                            .w(px(width))
                            .opacity(old_alpha)
                            .overflow_hidden()
                            .child(outgoing.render(width, compact, compact_actions, cx)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left(px(new_x))
                            .w(px(width))
                            .opacity(new_alpha)
                            .overflow_hidden()
                            .child(incoming.render(width, compact, compact_actions, cx)),
                    )
                    .into_any_element()
            }
        }
    }
}

pub struct PageSnapshot {
    pub tab_id: usize,
    pub data: TabData,
    pub command: String,
    pub append: String,
    pub rows_height: f32,
    pub rows_offset: f32,
    pub description_width: Option<f32>,
    pub enabled_first: bool,
}

/// Both full-width sheets travel together; do not cross-fade or slide an accent sweep.
pub fn offsets(width: f32, direction: i8, progress: f32) -> (f32, f32) {
    let progress = progress.clamp(0., 1.);
    let eased = 1. - (1. - progress).powi(3);
    let direction = if direction < 0 { -1. } else { 1. };
    (-direction * width * eased, direction * width * (1. - eased))
}

/// Fade through the editor background, rather than keeping both sheets opaque.
/// Smoothstep flattens the opacity velocity at the start, midpoint, and end.
pub fn opacities(progress: f32) -> (f32, f32) {
    let progress = progress.clamp(0., 1.);
    let smooth = |t: f32| t * t * (3. - 2. * t);
    if progress <= 0.5 {
        (1. - smooth(progress * 2.), 0.)
    } else {
        (0., smooth((progress - 0.5) * 2.))
    }
}

fn symbol(icon: IconName, cx: &App) -> Div {
    div()
        .size(px(CONTROL))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .child(
            Icon::new(icon)
                .size(px(ICON))
                .text_color(rgb(palette(cx).text)),
        )
}

fn visual_button(label: &str, icon: Option<IconName>, primary: bool, cx: &App) -> Div {
    let p = palette(cx);
    row()
        .h(px(CONTROL))
        .flex_shrink_0()
        .px(px(BUTTON_PADDING))
        .rounded(px(RADIUS))
        .border_1()
        .border_color(rgb(if primary { p.primary_border } else { p.border }))
        .bg(rgb(if primary { p.primary } else { p.button }))
        .text_color(rgb(if primary { p.on_primary } else { p.text }))
        .when_some(icon, |d, icon| d.child(Icon::new(icon).size(px(ICON))))
        .child(tr(cx, label))
}

fn ghost_button(label: &str, icon: Option<IconName>, cx: &App) -> Div {
    visual_button(label, icon, false, cx)
        .border_0()
        .bg(gpui::transparent_black())
}

fn visual_input(value: &str, placeholder: &str, code: bool, cx: &App) -> Div {
    let p = palette(cx);
    div()
        .w_full()
        .min_w_0()
        .h(px(CONTROL))
        .flex_shrink_0()
        .flex()
        .items_center()
        .px(px(FIELD_PADDING))
        .border_1()
        .border_color(rgb(p.border))
        .rounded(px(RADIUS))
        .bg(rgb(p.control))
        .overflow_hidden()
        .font_family(if code {
            cx.global::<Fonts>().code.clone()
        } else {
            cx.global::<Fonts>().body.clone()
        })
        .text_size(px(if code { CODE } else { theme::font_size(cx) }))
        .text_color(rgb(if value.is_empty() { p.muted } else { p.text }))
        .child(div().truncate().child(if value.is_empty() {
            tr(cx, placeholder)
        } else {
            value.to_owned()
        }))
}

fn visual_textarea(value: &str, placeholder: &str, code: bool, cx: &App) -> Div {
    let p = palette(cx);
    div()
        .w_full()
        .h_full()
        .min_w_0()
        .min_h_0()
        .p(px(FIELD_PADDING))
        .border_1()
        .border_color(rgb(p.border))
        .rounded(px(RADIUS))
        .bg(rgb(if code { p.subtle } else { p.control }))
        .overflow_hidden()
        .font_family(if code {
            cx.global::<Fonts>().code.clone()
        } else {
            cx.global::<Fonts>().body.clone()
        })
        .text_size(px(if code { CODE } else { theme::font_size(cx) }))
        .line_height(px(if code {
            LINE
        } else {
            LINE.max(theme::font_size(cx) * 1.4)
        }))
        .text_color(rgb(if value.is_empty() { p.muted } else { p.text }))
        .child(if value.is_empty() {
            tr(cx, placeholder)
        } else {
            value.to_owned()
        })
}

impl PageSnapshot {
    pub fn render(&self, width: f32, compact: bool, compact_actions: bool, cx: &App) -> Div {
        let p = palette(cx);
        let content_width = (width - PAGE_INSET * 2.).max(0.);
        let grid = ParameterGrid::new((content_width - TABLE_INSET * 2.).max(0.), compact);
        let parameter_compact_actions = content_width < 850.
            && (theme::font_size(cx) > 16. || *cx.global::<Language>() == Language::English);
        let data = &self.data;
        let form = column()
            .relative()
            .gap(px(GAP))
            .flex_shrink_0()
            .child(
                div()
                    .h(px(COMMAND_NAME_HEIGHT))
                    .w_full()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .px(px(FIELD_PADDING + 1.))
                    .overflow_hidden()
                    .text_size(px((theme::font_size(cx) + 12.).clamp(26., 32.)))
                    .line_height(px(COMMAND_NAME_LINE_HEIGHT))
                    .font_weight(FontWeight(theme::font_weight(cx).0.max(700.)))
                    .text_color(rgb(if data.name.is_empty() {
                        p.muted
                    } else {
                        p.text
                    }))
                    .child(div().truncate().child(if data.name.is_empty() {
                        tr(cx, "给这条命令起个名字")
                    } else {
                        data.name.clone()
                    })),
            )
            .child(field(
                "工作目录",
                row()
                    .child(div().flex_1().min_w_0().child(visual_input(
                        &data.directory,
                        "为空则使用程序当前目录",
                        true,
                        cx,
                    )))
                    .child(symbol(IconName::FolderOpen, cx)),
                cx,
            ))
            .child(field(
                "程序",
                row()
                    .child(div().flex_1().min_w_0().child(visual_input(
                        &data.program,
                        "输入程序路径，或粘贴完整命令后解析",
                        true,
                        cx,
                    )))
                    .child(visual_button("解析", Some(IconName::ScanText), false, cx)),
                cx,
            ))
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom(px(-GAP - 1.))
                    .h(px(1.))
                    .bg(rgb(p.divider)),
            );
        let header_cell = |label: &str, width| {
            div()
                .w(px(width))
                .flex_shrink_0()
                .px(px(FIELD_PADDING))
                .child(tr(cx, label))
        };
        let header = row()
            .gap(px(INPUT_GAP))
            .px(px(TABLE_INSET))
            .h(px(HEADER_HEIGHT))
            .flex_shrink_0()
            .text_size(px(SMALL))
            .text_color(rgb(p.muted))
            .child(div().w(px(GRIP)).flex_shrink_0())
            .child(div().w(px(CONTROL)).flex_shrink_0())
            .child(header_cell("选项 / 功能", grid.option))
            .child(header_cell("参数值", grid.value))
            .child(header_cell(
                if compact { "" } else { "备注" },
                grid.note.unwrap_or(CONTROL),
            ))
            .child(div().w(px(CONTROL)).flex_shrink_0());
        let mut rows = column().gap(px(GAP)).relative().top(px(self.rows_offset));
        let first_row = ((-self.rows_offset).max(0.) / (CONTROL + GAP)).floor() as usize;
        let last_row = (first_row + (self.rows_height / (CONTROL + GAP)).ceil() as usize + 2)
            .min(data.rows.len());
        rows = rows.top(px(self.rows_offset + first_row as f32 * (CONTROL + GAP)));
        for parameter in data
            .rows
            .iter()
            .skip(first_row)
            .take(last_row.saturating_sub(first_row))
        {
            let r = row()
                .gap(px(INPUT_GAP))
                .h(px(CONTROL))
                .flex_shrink_0()
                .child(
                    div()
                        .size(px(GRIP))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            Icon::new(IconName::GripVertical)
                                .size(px(ICON))
                                .text_color(rgb(p.muted)),
                        ),
                )
                .child(symbol(
                    if parameter.enabled {
                        IconName::Check
                    } else {
                        IconName::Minus
                    },
                    cx,
                ))
                .child(
                    div()
                        .w(px(grid.option))
                        .flex_shrink_0()
                        .opacity(if parameter.enabled { 1. } else { 0.55 })
                        .child(visual_input(&parameter.option, "选项 / 功能", true, cx)),
                )
                .child(
                    div()
                        .w(px(grid.value))
                        .flex_shrink_0()
                        .opacity(if parameter.enabled { 1. } else { 0.55 })
                        .child(visual_input(&parameter.value, "参数值", true, cx)),
                )
                .when_some(grid.note, |d, width| {
                    d.child(div().w(px(width)).flex_shrink_0().child(visual_input(
                        &parameter.note,
                        "备注",
                        false,
                        cx,
                    )))
                })
                .when(grid.note.is_none(), |d| {
                    d.child(symbol(IconName::MessageSquare, cx))
                })
                .child(symbol(IconName::Trash, cx));
            rows = rows.child(r);
        }
        if data.rows.is_empty() {
            rows = rows.child(
                div()
                    .h(px(CONTROL))
                    .px(px(GAP))
                    .text_color(rgb(p.muted))
                    .child(tr(cx, "暂无参数，点击下方添加。")),
            );
        }
        let parameters = column()
            .gap(px(GAP))
            .flex_shrink_0()
            .child(
                column().gap(px(HEADER_GAP)).child(header).child(
                    div()
                        .h(px(self.rows_height))
                        .flex_shrink_0()
                        .p(px(TABLE_INSET))
                        .rounded(px(RADIUS))
                        .bg(rgb(p.subtle))
                        .child(
                            div()
                                .h(px(self.rows_height - TABLE_INSET * 2.))
                                .overflow_hidden()
                                .child(rows),
                        ),
                ),
            )
            .child(
                row()
                    .h(px(CONTROL))
                    .flex_shrink_0()
                    .child(if parameter_compact_actions {
                        symbol(IconName::Plus, cx)
                    } else {
                        visual_button("添加参数", Some(IconName::Plus), false, cx)
                    })
                    .child(
                        visual_button("勾选参数置顶", None, self.enabled_first, cx)
                            .w(px(theme::font_size(cx) * 6. + BUTTON_PADDING * 2. + 4.)),
                    )
                    .child(div().flex_1())
                    .child(if parameter_compact_actions {
                        symbol(IconName::CheckCheck, cx)
                    } else {
                        ghost_button("全选", Some(IconName::CheckCheck), cx)
                    })
                    .child(if parameter_compact_actions {
                        symbol(IconName::Square, cx)
                    } else {
                        ghost_button("全不选", Some(IconName::Square), cx)
                    })
                    .child(
                        div()
                            .text_size(px(SMALL))
                            .text_color(rgb(p.muted))
                            .child(i18n::format(
                                cx,
                                "{} / {} 已启用",
                                &[
                                    &data
                                        .rows
                                        .iter()
                                        .filter(|row| row.enabled)
                                        .count()
                                        .to_string(),
                                    &data.rows.len().to_string(),
                                ],
                            )),
                    ),
            );
        let extras = row()
            .h(px(EXTRAS_HEIGHT))
            .child(
                div()
                    .w(px(content_width * 0.5))
                    .flex_shrink_0()
                    .child(visual_input(
                        &data.other,
                        "其他参数 / 重定向 / 管道（按原文追加）",
                        true,
                        cx,
                    )),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .items_center()
                    .gap(px(GAP))
                    .flex_1()
                    .min_w_0()
                    .h(px(EXTRAS_HEIGHT))
                    .border_1()
                    .border_color(rgb(p.focus))
                    .p(px(4.))
                    .rounded(px(RADIUS))
                    .bg(gpui::Hsla::from(rgb(p.focus)).opacity(0.10))
                    .child(div().flex_1().min_w_0().child(visual_input(
                        &self.append,
                        "仅解析 · 粘贴命令",
                        true,
                        cx,
                    )))
                    .child(
                        visual_button("追加解析", None, false, cx)
                            .w(px(104.))
                            .px(px(4.))
                            .justify_center(),
                    )
                    .child(
                        div()
                            .absolute()
                            .right_0()
                            .bottom(px(-15.))
                            .h(px(13.))
                            .text_size(px(10.))
                            .text_color(rgb(p.focus))
                            .child(tr(cx, "仅用于追加解析 · 不直接运行")),
                    ),
            );
        let split_available = (content_width - GAP).max(0.);
        let description_width = self
            .description_width
            .unwrap_or(split_available * 0.40)
            .clamp(120., (split_available - PREVIEW_MIN).max(120.));
        let preview = column()
            .flex_1()
            .min_h(px(PREVIEW_HEADER_HEIGHT + GAP * 2. + CONTROL + PREVIEW_MIN))
            .gap(px(GAP))
            .child(
                row()
                    .h(px(PREVIEW_HEADER_HEIGHT))
                    .flex_shrink_0()
                    .child(
                        div()
                            .flex_1()
                            .truncate()
                            .text_size(px(SMALL))
                            .text_color(rgb(p.muted))
                            .child(tr(cx, "命令预览·实时更新")),
                    )
                    .child(
                        div()
                            .w(px(description_width))
                            .flex_shrink_0()
                            .text_size(px(SMALL))
                            .text_color(rgb(p.muted))
                            .child(tr(cx, "描述 / 使用说明")),
                    ),
            )
            .child(
                row()
                    .gap_0()
                    .flex_1()
                    .min_h(px(PREVIEW_MIN))
                    .child(div().flex_1().min_w_0().h_full().child(visual_textarea(
                        &self.command,
                        "命令预览 · 修改参数后实时更新",
                        true,
                        cx,
                    )))
                    .child(
                        div()
                            .w(px(GAP))
                            .h_full()
                            .flex_shrink_0()
                            .flex()
                            .justify_center()
                            .child(div().w(px(2.)).h_full().bg(rgb(p.divider))),
                    )
                    .child(
                        div()
                            .w(px(description_width))
                            .h_full()
                            .flex_shrink_0()
                            .child(visual_textarea(
                                &data.description,
                                "描述 / 使用说明",
                                false,
                                cx,
                            )),
                    ),
            )
            .child(
                row()
                    .h(px(CONTROL))
                    .flex_shrink_0()
                    .child(if compact_actions {
                        symbol(IconName::Terminal, cx)
                    } else {
                        ghost_button("运行日志", Some(IconName::Terminal), cx)
                    })
                    .child(div().flex_1())
                    .child(if compact_actions {
                        symbol(IconName::RefreshCw, cx)
                    } else {
                        ghost_button("重新加载配置", Some(IconName::RefreshCw), cx)
                    })
                    .child(if compact_actions {
                        symbol(IconName::Copy, cx)
                    } else {
                        visual_button("复制命令", Some(IconName::Copy), false, cx)
                    })
                    .child(
                        visual_button("运行", Some(IconName::Play), true, cx).opacity(
                            if self.command.trim().is_empty() {
                                0.5
                            } else {
                                1.
                            },
                        ),
                    ),
            );
        // Plain divs/text/icons only: no InputState, subscriptions, focus scopes, or action handlers.
        column()
            .size_full()
            .min_w_0()
            .px(px(PAGE_INSET))
            .py(px(SECTION))
            .gap(px(SECTION))
            .bg(rgb(p.panel))
            .text_color(rgb(p.text))
            .font_family(cx.global::<Fonts>().body.clone())
            .text_size(px(theme::font_size(cx)))
            .font_weight(theme::font_weight(cx))
            .line_height(px(LINE))
            .child(form)
            .child(
                column()
                    .flex_shrink_0()
                    .gap(px(GAP))
                    .child(parameters)
                    .child(extras),
            )
            .child(preview)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rapid_retarget_cache_is_bounded_and_unpainted_targets_are_skipped() {
        let incoming = Rc::new(PageSnapshot {
            tab_id: 1,
            data: TabData::example(),
            command: "echo preview".into(),
            append: String::new(),
            rows_height: parameter_height(3),
            rows_offset: 0.,
            description_width: None,
            enabled_first: false,
        });
        let mut transition = PageTransition {
            started_at: Instant::now(),
            outgoing: Rc::new(PageVisual::Empty),
            incoming: None,
        };
        assert!(Rc::ptr_eq(&transition.freeze(1, 0.), &transition.outgoing));
        transition.incoming = Some(incoming.clone());
        match transition.freeze(-1, 0.25).as_ref() {
            PageVisual::Composite {
                progress,
                direction,
                ..
            } => {
                assert_eq!(*direction, -1);
                assert_eq!(opacities(*progress), (0.5, 0.));
            }
            _ => panic!("快速反向时须冻结当前渐隐画面"),
        }
        for _ in 0..8 {
            transition.outgoing = transition.freeze(1, 0.5);
        }
        assert!(matches!(
            transition.outgoing.as_ref(),
            PageVisual::Composite { depth: 8, .. }
        ));
        match transition.freeze(1, 0.5).as_ref() {
            PageVisual::Limited { opacity, .. } => assert_eq!(*opacity, 0.),
            _ => panic!("缓存限额不能把中点还原成全亮页面"),
        }
        match transition.freeze(1, 0.75).as_ref() {
            PageVisual::Limited { opacity, .. } => assert_eq!(*opacity, 0.5),
            _ => panic!("缓存限额仍须保留淡入透明度"),
        }
        assert!(matches!(
            transition.freeze(1, 1.).as_ref(),
            PageVisual::Page(_)
        ));
    }

    #[test]
    fn sheets_fade_out_then_in_with_the_midpoint_transparent() {
        assert_eq!(opacities(-1.), (1., 0.));
        assert_eq!(opacities(0.), (1., 0.));
        assert_eq!(opacities(0.25), (0.5, 0.));
        assert_eq!(opacities(0.5), (0., 0.));
        assert_eq!(opacities(0.75), (0., 0.5));
        assert_eq!(opacities(1.), (0., 1.));
        assert_eq!(opacities(2.), (0., 1.));
        let mut previous = (1., 0.);
        for step in 0..=100 {
            let alpha = opacities(step as f32 / 100.);
            assert!(alpha.0 <= previous.0 && alpha.1 >= previous.1);
            assert!(
                alpha.0 == 0. || alpha.1 == 0.,
                "两页不能同时不透明地交叉覆盖"
            );
            previous = alpha;
        }
    }

    #[test]
    fn complete_sheets_travel_one_viewport_apart_and_finish_on_target() {
        for direction in [-1, 1] {
            let (old, new) = offsets(600., direction, 0.);
            assert_eq!(old, 0.);
            assert_eq!(new, direction as f32 * 600.);
            let (old, new) = offsets(600., direction, 0.5);
            assert!((new - old - direction as f32 * 600.).abs() < 0.001);
            assert!(new.abs() > 0. && new.abs() < 600.);
            assert_eq!(offsets(600., direction, 1.), (-direction as f32 * 600., 0.));
        }
    }
}
