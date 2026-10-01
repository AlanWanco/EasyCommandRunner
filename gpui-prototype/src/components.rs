//! GPUI Kit 负责输入、焦点、选择、IME 和按钮行为；这里只约束 ECR 的尺寸与配色。
use crate::{
    i18n::tr,
    theme::{self, palette, Fonts},
    tokens::*,
};
use gpui::assets::IconName;
use gpui::{
    canvas,
    component::{
        button::{Button, ButtonCustomVariant, ButtonVariants},
        input::{Input, InputState, Textarea, TextareaState},
        Colorize, FocusableExt, Icon, Sizable, Size, Theme as KitTheme,
    },
    div,
    prelude::*,
    px, rgb, App, Div, ElementId, Entity, EntityId, FocusHandle, Focusable, RenderOnce, Stateful,
    StyleRefinement, Window,
};

pub fn row() -> Div {
    div().flex().items_center().gap(px(GAP)).min_w_0()
}
pub fn column() -> Div {
    div().flex().flex_col().min_w_0().min_h_0()
}

#[cfg(feature = "ui-test")]
pub fn frame(id: impl Into<ElementId>) -> gpui::base::test_support::Observed<Stateful<Div>> {
    use gpui::test::TestSupportExt;
    div().id(id).test_support()
}

#[cfg(not(feature = "ui-test"))]
pub fn frame(id: impl Into<ElementId>) -> Stateful<Div> {
    div().id(id)
}

pub fn button(id: impl Into<ElementId>, label: &str, icon: Option<IconName>, cx: &App) -> Button {
    // Kit 的 Medium 按钮会在内部把文字重置为 16px；用子元素显式覆盖，
    // 让所有按钮与正文同字号，并随工作区字号设置一起变化（但不改变 32px 高度）。
    Button::new(id)
        .accessibility_label(tr(cx, label))
        .with_size(Size::Medium)
        .h(px(CONTROL))
        .flex_shrink_0()
        .px(px(BUTTON_PADDING))
        .gap(px(GAP))
        .when_some(icon, |b, icon| b.icon(Icon::new(icon).size(px(ICON))))
        .child(
            div()
                .text_size(px(theme::font_size(cx)))
                .font_weight(theme::font_weight(cx))
                .child(tr(cx, label)),
        )
}
pub fn icon_button(id: impl Into<ElementId>, label: &str, icon: IconName, cx: &App) -> Button {
    Button::new(id)
        .icon(Icon::new(icon).size(px(ICON)))
        .ghost()
        .tooltip(tr(cx, label))
        .accessibility_label(tr(cx, label))
        .with_size(Size::Medium)
        .size(px(CONTROL))
        .flex_shrink_0()
}

pub fn action_button(
    id: impl Into<ElementId>,
    label: &str,
    icon: IconName,
    compact: bool,
    cx: &App,
) -> Button {
    if compact {
        icon_button(id, label, icon, cx)
    } else {
        button(id, label, Some(icon), cx)
    }
}

pub fn delete_button(id: impl Into<ElementId>, cx: &App) -> Button {
    let p = palette(cx);
    icon_button(id, "删除此参数", IconName::Trash, cx).custom(
        ButtonCustomVariant::new(cx)
            .foreground(rgb(p.muted).into())
            .hover(rgb(p.danger_hover).into())
            .active(rgb(p.danger_hover).into()),
    )
}

/// 菜单也使用 32px 行高；Kit 的普通菜单项默认只有 26px。
pub fn menu_item(label: &'static str) -> gpui::component::menu::PopupMenuItem {
    gpui::component::menu::PopupMenuItem::element(move |_, cx| {
        frame(label)
            .flex()
            .items_center()
            .h(px(CONTROL))
            .text_size(px(BODY))
            .child(tr(cx, label))
    })
}

/// 仅绘制输入框的焦点过渡；编辑、选择、剪贴板和 IME 仍由 Kit 原控件处理。
#[derive(IntoElement)]
pub struct FocusTextField<T: Styled + IntoElement + 'static> {
    inner: T,
    state_id: EntityId,
    focus_handle: FocusHandle,
    multiline: bool,
    single_line_height: f32,
    quiet: bool,
}

impl<T: Styled + IntoElement + 'static> Styled for FocusTextField<T> {
    fn style(&mut self) -> &mut StyleRefinement {
        self.inner.style()
    }
}

impl FocusTextField<Textarea> {
    pub fn readonly(mut self, readonly: bool) -> Self {
        self.inner = self.inner.readonly(readonly);
        self
    }
}

fn text_focus_progress(id: EntityId, focused: bool, window: &mut Window, cx: &mut App) -> f32 {
    gpui::base::transition(
        ElementId::from(("text-focus-motion", id)),
        if focused { 1_f32 } else { 0_f32 },
        gpui::base::Transition::new(KitTheme::global(cx).motion.duration_normal)
            .ease(|t| t * t * (3. - 2. * t)), // 从中心匀滑延伸，末端放缓。
        window,
        cx,
    )
}

/// 只裁切外环的笔画，不裁切输入文本或光标；左右两侧总是关于中心对称。
fn focus_ring_mask(
    bounds: gpui::Bounds<gpui::Pixels>,
    progress: f32,
) -> gpui::ContentMask<gpui::Pixels> {
    let ring = bounds.dilate(px(3.));
    let half_width = px(f32::from(ring.size.width) * progress.clamp(0., 1.) / 2.);
    gpui::ContentMask {
        bounds: gpui::Bounds::from_corners(
            gpui::point(ring.center().x - half_width, ring.top()),
            gpui::point(ring.center().x + half_width, ring.bottom()),
        ),
    }
}

impl<T: Styled + IntoElement + 'static> RenderOnce for FocusTextField<T> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // 复用 Kit 的键盘焦点句柄，点击输入内容和 Tab 聚焦边框使用同一动效。
        let frame_focus = window
            .use_keyed_state(("input-frame-focus", self.state_id), cx, |_, cx| {
                cx.focus_handle()
            })
            .read(cx)
            .clone();
        let focused =
            self.focus_handle.is_focused(window) || frame_focus.contains_focused(window, cx);
        let progress = text_focus_progress(self.state_id, focused, window, cx);
        let p = palette(cx);
        let focus = gpui::Hsla::from(rgb(p.focus));
        let border = if self.quiet {
            focus.opacity(progress)
        } else {
            focus.mix_oklab(rgb(p.border).into(), progress)
        };
        let ring_visible = KitTheme::global(cx).focus_ring;
        div()
            .relative()
            .w_full()
            .min_w_0()
            .flex_shrink_0()
            .when(self.multiline, |d| d.h_full().min_h_0())
            .when(!self.multiline, |d| d.h(px(self.single_line_height)))
            .child(
                self.inner
                    .border_1()
                    .border_color(border)
                    .rounded(px(RADIUS)),
            )
            // Canvas 只绘制外环，不增加布局尺寸或鼠标命中区域；不淡化文字和光标。
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        if ring_visible && progress > 0. {
                            let ring = bounds.dilate(px(3.));
                            window.with_content_mask(
                                Some(focus_ring_mask(bounds, progress)),
                                |window| {
                                    window.paint_quad(
                                        gpui::outline(ring, focus.opacity(0.5), Default::default())
                                            .corner_radii(px(RADIUS + 3.))
                                            .border_widths(px(3.)),
                                    );
                                },
                            );
                        }
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .bottom_0(),
            )
    }
}

pub fn input(
    id: impl Into<ElementId>,
    state: &Entity<InputState>,
    label: &str,
    code: bool,
    cx: &App,
) -> FocusTextField<Input> {
    let fonts = cx.global::<Fonts>();
    let mut input = Input::new(state)
        .bordered(false) // 去掉 Kit 即时边框，由 wrapper 平滑调整同一条边框。
        .focus_ring(false)
        .id(id)
        .aria_label(tr(cx, label))
        .with_size(Size::Medium)
        .w_full()
        .min_w_0()
        .flex_shrink_0()
        .bg(rgb(palette(cx).control))
        .font_family(if code {
            fonts.code.clone()
        } else {
            fonts.body.clone()
        })
        .text_size(px(if code { CODE } else { theme::font_size(cx) }))
        .font_weight(theme::font_weight(cx));
    // Input::h 是多行高度接口，单行尺寸直接写 Styled，避免被 rem 或内部 padding 改写。
    input.style().size.height = Some(px(CONTROL).into());
    input.style().padding.left = Some(px(FIELD_PADDING).into());
    input.style().padding.right = Some(px(FIELD_PADDING).into());
    input.style().padding.top = Some(px(0.).into());
    input.style().padding.bottom = Some(px(0.).into());
    FocusTextField {
        inner: input,
        state_id: state.entity_id(),
        focus_handle: state.focus_handle(cx),
        multiline: false,
        single_line_height: CONTROL,
        quiet: false,
    }
}

/// An editable page title, retaining the same InputState, focus ring, selection and IME handling.
pub fn heading_input(state: &Entity<InputState>, cx: &App) -> FocusTextField<Input> {
    let mut title = input("command-name", state, "命令名称", false, cx);
    title.inner = title
        .inner
        .text_size(px((theme::font_size(cx) + 12.).clamp(26., 32.)))
        // Kit otherwise uses a fixed 1.25rem (20px) text line, clipping the larger glyphs.
        .line_height(px(COMMAND_NAME_LINE_HEIGHT))
        .font_weight(gpui::FontWeight(theme::font_weight(cx).0.max(700.)))
        .bg(gpui::transparent_black());
    title.inner.style().size.height = Some(px(COMMAND_NAME_HEIGHT).into());
    title.single_line_height = COMMAND_NAME_HEIGHT;
    title.quiet = true;
    title
}

pub fn textarea(
    state: &Entity<TextareaState>,
    id: &str,
    code: bool,
    cx: &App,
) -> FocusTextField<Textarea> {
    let fonts = cx.global::<Fonts>();
    let inner = Textarea::new(state)
        .bordered(false) // 关闭 Kit 的即时边框/外环，由同一焦点过渡绘制，避免高光叠加。
        .accessibility_id(id.to_owned())
        .aria_label(id.to_owned())
        .bg(rgb(palette(cx).control))
        .w_full()
        .h_full()
        .min_h_0()
        .font_family(if code {
            fonts.code.clone()
        } else {
            fonts.body.clone()
        })
        .text_size(px(if code { CODE } else { theme::font_size(cx) }))
        .font_weight(theme::font_weight(cx))
        .line_height(px(if code {
            LINE
        } else {
            LINE.max(theme::font_size(cx) * 1.4)
        }))
        .p(px(FIELD_PADDING));
    FocusTextField {
        inner,
        state_id: state.entity_id(),
        focus_handle: state.focus_handle(cx),
        multiline: true,
        single_line_height: CONTROL,
        quiet: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::component::Root;
    use gpui::test::TestWindowExt;
    use gpui::{AppContext, Context, Render, TestAppContext};
    use std::time::Duration;

    struct FocusMotionProbe {
        focused: bool,
        progress: f32,
    }

    impl Render for FocusMotionProbe {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            self.progress = text_focus_progress(cx.entity_id(), self.focused, window, cx);
            div()
        }
    }

    #[gpui::test]
    fn heading_has_room_for_large_glyphs_without_recreating_input(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(theme::Theme::Dark, cx);
        });
        let handle = cx.open_window(gpui::size(px(400.), px(200.)), |w, cx| {
            Root::new(
                cx.new(|_| FocusMotionProbe {
                    focused: false,
                    progress: 0.,
                }),
                w,
                cx,
            )
        });
        cx.update_window(handle.into(), |_, w, cx| {
            let state = cx.new(|cx| InputState::new(w, cx).default_value("中文 Ågjpq"));
            let original_id = state.entity_id();
            for font_size in [10, 14, 24] {
                theme::set_font_size(font_size, cx);
                let mut heading = heading_input(&state, cx);
                assert_eq!(heading.state_id, original_id);
                assert_eq!(heading.single_line_height, COMMAND_NAME_HEIGHT);
                let style = heading.inner.style();
                assert_eq!(style.size.height, Some(px(COMMAND_NAME_HEIGHT).into()));
                assert_eq!(
                    style.text.line_height,
                    Some(px(COMMAND_NAME_LINE_HEIGHT).into())
                );
                assert!(
                    COMMAND_NAME_LINE_HEIGHT >= (font_size as f32 + 12.).clamp(26., 32.) * 1.25
                );
                assert!(COMMAND_NAME_HEIGHT - COMMAND_NAME_LINE_HEIGHT >= 8.);
            }
            assert_eq!(state.read(cx).value().as_ref(), "中文 Ågjpq");
            w.remove_window();
        })
        .unwrap();
    }

    #[test]
    fn focus_mask_opens_symmetrically_without_moving_the_outline() {
        let field = gpui::Bounds::new(
            gpui::point(px(100.), px(20.)),
            gpui::size(px(240.), px(32.)),
        );
        let full = field.dilate(px(3.));
        for progress in [0., 0.1, 0.5, 1., 1.5] {
            let mask = focus_ring_mask(field, progress).bounds;
            assert_eq!(mask.center().x, full.center().x);
            assert_eq!(mask.top(), full.top());
            assert_eq!(mask.bottom(), full.bottom());
            let expected = px(f32::from(full.size.width) * progress.clamp(0., 1.));
            assert!((mask.size.width - expected).abs() < px(0.001));
        }
        assert_eq!(focus_ring_mask(field, 1.).bounds, full);
    }

    #[gpui::test]
    fn focus_transition_reverses_smoothly_and_respects_reduced_motion(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(theme::Theme::Dark, cx);
        });
        let mut probe = None;
        let handle = cx.open_window(gpui::size(px(400.), px(200.)), |w, cx| {
            let entity = cx.new(|_| FocusMotionProbe {
                focused: false,
                progress: 0.,
            });
            probe = Some(entity.clone());
            Root::new(entity, w, cx)
        });
        let probe = probe.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            assert_eq!(probe.read(cx).progress, 0.);
            probe.update(cx, |p, cx| {
                p.focused = true;
                cx.notify();
            });
            w.render_frame(cx);
            assert_eq!(probe.read(cx).progress, 0., "聚焦不应瞬间变亮");
        })
        .unwrap();
        cx.background_executor
            .advance_clock(Duration::from_millis(40));
        let halfway = cx
            .update_window(handle.into(), |_, w, cx| {
                w.render_frame(cx);
                let midway = probe.read(cx).progress;
                assert!(midway > 0. && midway < 1., "必须存在中间帧");
                w.render_frame(cx);
                assert_eq!(probe.read(cx).progress, midway, "重绘不得重新启动过渡");
                probe.update(cx, |p, cx| {
                    p.focused = false;
                    cx.notify();
                });
                w.render_frame(cx);
                assert_eq!(probe.read(cx).progress, midway, "快速失焦应从当前亮度退回");
                midway
            })
            .unwrap();
        cx.background_executor
            .advance_clock(Duration::from_millis(30));
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            assert!(probe.read(cx).progress < halfway);
        })
        .unwrap();
        cx.background_executor
            .advance_clock(Duration::from_millis(200));
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            assert_eq!(probe.read(cx).progress, 0.);
            theme::set_reduced_motion(true, cx);
            probe.update(cx, |p, cx| {
                p.focused = true;
                cx.notify();
            });
            w.render_frame(cx);
            assert_eq!(probe.read(cx).progress, 1., "减少动效应立即显示焦点");
            theme::set_reduced_motion(false, cx);
            cx.set_reduce_motion(true);
            probe.update(cx, |p, cx| {
                p.focused = false;
                cx.notify();
            });
            w.render_frame(cx);
            assert_eq!(probe.read(cx).progress, 0., "系统减少动效同样生效");
            w.remove_window();
        })
        .unwrap();
    }
}

pub fn field(label: &str, child: impl IntoElement, cx: &App) -> Div {
    row()
        .gap(px(INPUT_GAP))
        .h(px(CONTROL))
        .flex_shrink_0()
        .child(
            div()
                .w(px(LABEL.max(theme::font_size(cx) * 4.)))
                .flex_shrink_0()
                .child(tr(cx, label)),
        )
        .child(div().flex_1().min_w_0().child(child))
}
