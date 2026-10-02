//! Windows gets a rounded GPUI popup; native platforms never instantiate this view.
use crate::{
    app::CommandWorkspace,
    components::{column, frame, row},
    theme::{self, palette, Fonts},
    tray_actions::{TrayAction, TrayMenuState},
};
use gpui::assets::IconName;
use gpui::component::Icon;
#[cfg(target_os = "windows")]
use gpui::AppContext;
use gpui::{
    div, point, prelude::*, px, rgb, size, App, Bounds, Context, Entity, FocusHandle, Pixels,
    Render, ScrollHandle, Size, Subscription, Window,
};
use std::sync::mpsc::Sender;

const RADIUS: f32 = 12.;
const PADDING: f32 = 8.;
const WIDTH: f32 = 280.;

fn row_height(cx: &App) -> f32 {
    // Use the same 4-DIP grid as the gaps/insets so each row lands on physical
    // pixels at the common 100/125/150/175/200% Windows scales. Otherwise eight
    // separately rounded row heights accumulate into uneven bottom whitespace.
    ((theme::font_size(cx) * 1.4 + 12.).max(32.) / 4.).ceil() * 4.
}
pub(crate) fn menu_size(cx: &App) -> Size<Pixels> {
    size(
        px(WIDTH.max(theme::font_size(cx) * 15.)),
        px(row_height(cx) * 8. + 3. * 12. + PADDING * 2. + 2.),
    )
}

/// Works for negative-origin monitors and small work areas; taskbar bounds are excluded.
#[cfg(any(target_os = "windows", test))]
fn placement(
    anchor: gpui::Point<Pixels>,
    work: Bounds<Pixels>,
    requested: Size<Pixels>,
) -> Bounds<Pixels> {
    let available = size(
        (work.size.width - px(16.)).max(px(1.)),
        (work.size.height - px(16.)).max(px(1.)),
    );
    let dimensions = requested.min(&available);
    let min_x = work.left() + px(8.);
    let min_y = work.top() + px(8.);
    let x = (anchor.x - dimensions.width)
        .clamp(min_x, (work.right() - dimensions.width - px(8.)).max(min_x));
    let y = (anchor.y - dimensions.height).clamp(
        min_y,
        (work.bottom() - dimensions.height - px(8.)).max(min_y),
    );
    Bounds::new(point(x, y), dimensions)
}

fn next_enabled(state: &TrayMenuState, current: Option<usize>, delta: i32) -> Option<usize> {
    let count = TrayAction::ALL.len() as i32;
    let start = current.map_or(if delta < 0 { 0 } else { -1 }, |index| index as i32);
    (1..=count)
        .map(|step| (start + step * delta).rem_euclid(count) as usize)
        .find(|index| state.enabled(TrayAction::ALL[*index]))
}

pub(crate) struct TrayPopup {
    workspace: Entity<CommandWorkspace>,
    visible: bool,
    sender: Sender<TrayAction>,
    focus: FocusHandle,
    selected: Option<usize>,
    scroll: ScrollHandle,
    submitted: bool,
    activated: bool,
    _subscriptions: Vec<Subscription>,
}

impl TrayPopup {
    pub fn new(
        workspace: Entity<CommandWorkspace>,
        visible: bool,
        sender: Sender<TrayAction>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let observer = cx.observe_in(&workspace, window, |_, _, _, cx| cx.notify());
        let activation = cx.observe_window_activation(window, |view, window, _| {
            if window.is_window_active() {
                view.activated = true;
            } else if view.activated {
                window.remove_window();
            }
        });
        let state = workspace.read(cx).tray_menu_state(visible, cx);
        Self {
            workspace,
            visible,
            sender,
            focus,
            selected: next_enabled(&state, None, 1),
            scroll: ScrollHandle::new(),
            submitted: false,
            activated: window.is_window_active(),
            _subscriptions: vec![observer, activation],
        }
    }

    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        if self
            .workspace
            .read(cx)
            .tray_menu_state(self.visible, cx)
            .enabled(TrayAction::ALL[index])
        {
            self.selected = Some(index);
            self.scroll.scroll_to_item(index);
            cx.notify();
        }
    }

    fn choose(&mut self, action: TrayAction, window: &mut Window, cx: &mut Context<Self>) {
        // Revalidate live state; disabled/stale Stop/Run/Save actions cannot execute.
        if self.submitted
            || !self
                .workspace
                .read(cx)
                .tray_menu_state(self.visible, cx)
                .enabled(action)
        {
            return;
        }
        self.submitted = true;
        if let Err(error) = self.sender.send(action) {
            eprintln!("托盘操作队列已关闭：{error}");
        }
        // The runtime processes the action after this window is gone, never under a Root lease.
        window.remove_window();
    }
}

impl Render for TrayPopup {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let state = self.workspace.read(cx).tray_menu_state(self.visible, cx);
        let height = row_height(cx);
        let icons = [
            IconName::AppWindow,
            IconName::Play,
            IconName::Square,
            IconName::Terminal,
            IconName::Save,
            IconName::Settings2,
            IconName::Moon,
            IconName::LogOut,
        ];
        let items = TrayAction::ALL
            .into_iter()
            .enumerate()
            .map(|(index, action)| {
                let enabled = state.enabled(action);
                let selected = self.selected == Some(index) && enabled;
                let mut item = frame(action.id())
                    .h(px(height))
                    .w_full()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(12.))
                    .rounded(px(6.))
                    .text_color(rgb(if enabled { p.text } else { p.muted }))
                    .when(selected, |item| item.bg(rgb(p.hover)))
                    .child(Icon::new(icons[index]).size(px(18.)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(state.label(action)),
                    )
                    .child(div().w(px(18.)).when(state.checked(action), |item| {
                        item.child(Icon::new(IconName::Check).size(px(16.)))
                    }));
                if enabled {
                    item = item
                        .cursor_pointer()
                        .on_mouse_move(cx.listener(move |view, _, _, cx| view.select(index, cx)))
                        .on_click(
                            cx.listener(move |view, _, window, cx| view.choose(action, window, cx)),
                        );
                }
                column()
                    .flex_shrink_0()
                    .when([3, 6, 7].contains(&index), |column| {
                        column.child(
                            row()
                                .h(px(12.))
                                .child(div().w_full().h(px(1.)).bg(rgb(p.divider))),
                        )
                    })
                    .child(item)
            });
        frame("tray-popup-surface")
            .size_full()
            .flex()
            .flex_col()
            // Keep the four insets outside the scroll viewport. Revealing the
            // first/last item must never consume the card's outer whitespace.
            .p(px(PADDING))
            .rounded(px(RADIUS))
            .overflow_hidden()
            .bg(rgb(p.panel))
            .border_1()
            .border_color(rgb(p.border))
            .font_family(cx.global::<Fonts>().body.clone())
            .font_weight(theme::font_weight(cx))
            .text_size(px(theme::font_size(cx)))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|view, event: &gpui::KeyDownEvent, window, cx| {
                let state = view.workspace.read(cx).tray_menu_state(view.visible, cx);
                match event.keystroke.key.as_str() {
                    "escape" => {
                        window.remove_window();
                    }
                    "up" | "down" | "tab" => {
                        let delta = if event.keystroke.key == "up"
                            || (event.keystroke.key == "tab" && event.keystroke.modifiers.shift)
                        {
                            -1
                        } else {
                            1
                        };
                        if let Some(index) = next_enabled(&state, view.selected, delta) {
                            view.select(index, cx);
                        }
                    }
                    "home" => {
                        if let Some(index) = next_enabled(&state, None, 1) {
                            view.select(index, cx);
                        }
                    }
                    "end" => {
                        if let Some(index) = next_enabled(&state, None, -1) {
                            view.select(index, cx);
                        }
                    }
                    "enter" | "space" => {
                        if let Some(index) = view.selected {
                            view.choose(TrayAction::ALL[index], window, cx);
                        }
                    }
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .child(
                frame("tray-popup-items")
                    .w_full()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .overflow_x_hidden()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .children(items),
            )
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn open(
    workspace: Entity<CommandWorkspace>,
    visible: bool,
    sender: Sender<TrayAction>,
    physical: (f64, f64),
    cx: &mut App,
) -> Result<gpui::WindowHandle<TrayPopup>, String> {
    use gpui::{
        DisplayId, WindowBackgroundAppearance, WindowBounds, WindowDecorations, WindowKind,
        WindowOptions,
    };
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::mem::size_of;
    use windows_sys::Win32::{
        Foundation::{POINT, RECT},
        Graphics::Gdi::{
            CreateRoundRectRgn, DeleteObject, GetMonitorInfoW, MonitorFromPoint, SetWindowRgn,
            MONITORINFO, MONITOR_DEFAULTTONEAREST,
        },
        UI::{
            Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW},
            HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
            Input::KeyboardAndMouse::{GetFocus, SetActiveWindow, SetFocus},
            WindowsAndMessaging::{
                GetClientRect, GetForegroundWindow, SetForegroundWindow, SystemParametersInfoW,
                SPI_GETHIGHCONTRAST,
            },
        },
    };
    let mut contrast = HIGHCONTRASTW {
        cbSize: size_of::<HIGHCONTRASTW>() as u32,
        ..Default::default()
    };
    if unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            contrast.cbSize,
            &mut contrast as *mut _ as _,
            0,
        )
    } == 0
        || contrast.dwFlags & HCF_HIGHCONTRASTON != 0
    {
        return Err("Windows 高对比度或查询失败，使用系统原生菜单".into());
    }
    let monitor = unsafe {
        MonitorFromPoint(
            POINT {
                x: physical.0 as i32,
                y: physical.1 as i32,
            },
            MONITOR_DEFAULTTONEAREST,
        )
    };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
        return Err("无法获取托盘显示器工作区".into());
    }
    let mut dpi = 96;
    let mut y_dpi = 96;
    if unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut y_dpi) } != 0 {
        return Err("无法获取托盘显示器缩放".into());
    }
    let scale = dpi.max(96) as f32 / 96.;
    let work = Bounds::new(
        point(
            px(info.rcWork.left as f32 / scale),
            px(info.rcWork.top as f32 / scale),
        ),
        size(
            px((info.rcWork.right - info.rcWork.left) as f32 / scale),
            px((info.rcWork.bottom - info.rcWork.top) as f32 / scale),
        ),
    );
    let bounds = placement(
        point(px(physical.0 as f32 / scale), px(physical.1 as f32 / scale)),
        work,
        menu_size(cx),
    );
    let handle = cx
        .open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                kind: WindowKind::PopUp,
                focus: true,
                show: true,
                is_movable: false,
                is_resizable: false,
                is_minimizable: false,
                display_id: Some(DisplayId::new(monitor as u64)),
                window_background: WindowBackgroundAppearance::Transparent,
                window_decorations: Some(WindowDecorations::Client),
                ..Default::default()
            },
            move |window, cx| cx.new(|cx| TrayPopup::new(workspace, visible, sender, window, cx)),
        )
        .map_err(|error| error.to_string())?;
    // Clip the actual HWND as well as the rendered surface. Win10 and non-DComp
    // systems must not turn transparent round corners into an opaque square.
    let untyped: gpui::AnyWindowHandle = handle.into();
    let clipped = untyped
        .update(cx, |_, window, _| unsafe {
            let Ok(native) = HasWindowHandle::window_handle(window) else {
                return false;
            };
            let RawWindowHandle::Win32(native) = native.as_raw() else {
                return false;
            };
            let hwnd = native.hwnd.get() as *mut _;
            let mut rect = RECT::default();
            if GetClientRect(hwnd, &mut rect) == 0 {
                return false;
            }
            let diameter = (RADIUS * 2. * window.scale_factor()).round() as i32;
            let region =
                CreateRoundRectRgn(0, 0, rect.right + 1, rect.bottom + 1, diameter, diameter);
            if region.is_null() {
                return false;
            }
            if SetWindowRgn(hwnd, region, 1) == 0 {
                DeleteObject(region);
                return false;
            }
            // On success ownership of HRGN belongs to Windows, never DeleteObject it.
            // Do not call GPUI Windows activate(): it synthesizes Alt input.
            if GetForegroundWindow() != hwnd && SetForegroundWindow(hwnd) == 0 {
                return false;
            }
            SetActiveWindow(hwnd);
            SetFocus(hwnd);
            GetFocus() == hwnd
        })
        .map_err(|error| error.to_string())?;
    if !clipped {
        let _ = untyped.update(cx, |_, window, _| window.remove_window());
        return Err("无法设置托盘弹出菜单圆角；退回原生菜单".into());
    }
    Ok(handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::component::Root;
    use gpui::test::TestWindowExt;
    use gpui::{AppContext, TestAppContext};
    use std::sync::mpsc;

    #[gpui::test]
    fn popup_keyboard_skips_disabled_rows_submits_once_and_dismisses(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(crate::theme::Theme::Dark, cx);
        });
        let mut workspace = None;
        let main = cx.open_window(size(px(850.), px(800.)), |window, cx| {
            let view = cx.new(|cx| CommandWorkspace::new(window, cx));
            workspace = Some(view.clone());
            Root::new(view, window, cx)
        });
        let workspace = workspace.unwrap();
        let (sender, receiver) = mpsc::channel();
        let mut menu = None;
        let popup = cx.open_window(size(px(300.), px(360.)), |window, cx| {
            let view = cx.new(|cx| TrayPopup::new(workspace.clone(), true, sender, window, cx));
            menu = Some(view.clone());
            Root::new(view, window, cx)
        });
        let menu = menu.unwrap();
        cx.update_window(popup.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(menu.read(cx).selected, Some(0));
            window.click(TrayAction::Stop.id(), cx);
            assert!(!menu.read(cx).submitted, "禁用的停止菜单不能执行或关闭弹窗");
            window.press("down", cx);
            assert_eq!(menu.read(cx).selected, Some(1));
            window.press("down", cx); // Stop is disabled: no fabricated run handle.
            assert_eq!(menu.read(cx).selected, Some(3));
            window.press("down", cx); // Save is disabled for a fixture without a store.
            assert_eq!(menu.read(cx).selected, Some(5));
            window.press("enter", cx);
        })
        .unwrap();
        assert_eq!(receiver.try_recv().unwrap(), TrayAction::Settings);
        assert!(receiver.try_recv().is_err());
        assert_eq!(cx.windows().len(), 1, "选择菜单项应先关闭弹窗，主窗口保留");
        let (sender, receiver) = mpsc::channel();
        let popup = cx.open_window(size(px(300.), px(360.)), |window, cx| {
            let view = cx.new(|cx| TrayPopup::new(workspace, true, sender, window, cx));
            Root::new(view, window, cx)
        });
        cx.update_window(popup.into(), |_, window, cx| {
            window.render_frame(cx);
            window.press("escape", cx);
        })
        .unwrap();
        assert!(receiver.try_recv().is_err());
        assert_eq!(cx.windows().len(), 1);
        let (sender, receiver) = mpsc::channel();
        let popup = cx.open_window(size(px(300.), px(360.)), |window, cx| {
            let workspace = cx.new(|cx| CommandWorkspace::new(window, cx));
            let view = cx.new(|cx| TrayPopup::new(workspace, true, sender, window, cx));
            Root::new(view, window, cx)
        });
        cx.update_window(popup.into(), |_, window, cx| {
            window.activate_window();
            window.render_frame(cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(main.into(), |_, window, _| window.activate_window())
            .unwrap();
        cx.run_until_parked();
        assert_eq!(cx.windows().len(), 1, "失焦必须关闭弹窗而不是留下常驻浮窗");
        assert!(receiver.try_recv().is_err());
        cx.update_window(main.into(), |_, window, _| window.remove_window())
            .unwrap();
    }

    #[gpui::test]
    fn normal_popup_hover_insets_are_equal_on_all_four_sides(cx: &mut TestAppContext) {
        cx.update(gpui::init);
        for mode in [crate::theme::Theme::Dark, crate::theme::Theme::Light] {
            for font_size in [10, 14, 16, 24] {
                for scale_factor in [1., 1.25, 1.5, 1.75, 2.] {
                    cx.update(|cx| theme::apply(mode, cx));
                    let mut workspace = None;
                    let main = cx.open_window(size(px(850.), px(800.)), |window, cx| {
                        let view = cx.new(|cx| CommandWorkspace::new(window, cx));
                        workspace = Some(view.clone());
                        Root::new(view, window, cx)
                    });
                    let workspace = workspace.unwrap();
                    cx.update(|cx| theme::set_font_size(font_size, cx));
                    let dimensions = cx.update(|cx| menu_size(cx));
                    let (sender, _receiver) = mpsc::channel();
                    let mut menu = None;
                    let popup = cx.open_window(dimensions, |window, cx| {
                        let view = cx.new(|cx| TrayPopup::new(workspace, true, sender, window, cx));
                        menu = Some(view.clone());
                        Root::new(view, window, cx)
                    });
                    let menu = menu.unwrap();
                    cx.simulate_window_scale_factor_change(popup.into(), scale_factor);
                    cx.update_window(popup.into(), |_, window, cx| {
                        // Actual hit testing / hover in the test platform, no OS input.
                        for hovered in [TrayAction::ToggleWindow, TrayAction::Logs, TrayAction::Quit] {
                            window.hover(hovered.id(), cx);
                            window.render_frame(cx);
                            assert_eq!(menu.read(cx).selected.map(|index| TrayAction::ALL[index]), Some(hovered));
                            let surface = window.find("tray-popup-surface").bounds();
                            let first = window.find(TrayAction::ToggleWindow.id()).bounds();
                            let last = window.find(TrayAction::Quit.id()).bounds();
                            let expected = px(PADDING + 1.);
                            // Fractional DPI may round opposing edges by one physical pixel.
                            let tolerance = px(1. / scale_factor + 0.01);
                            for action in TrayAction::ALL {
                                let item = window.find(action.id()).bounds();
                                let left = item.left() - surface.left();
                                let right = surface.right() - item.right();
                                assert!((left - right).abs() <= tolerance,
                                    "{mode:?}/{font_size}px/{scale_factor}x {action:?}: 左右留白不对称：{left:?}/{right:?}");
                                assert!((left - expected).abs() <= tolerance && (right - expected).abs() <= tolerance);
                            }
                            let top = first.top() - surface.top();
                            let bottom = surface.bottom() - last.bottom();
                            assert!((top - bottom).abs() <= tolerance,
                                "{mode:?}/{font_size}px/{scale_factor}x: 上下留白不对称：{top:?}/{bottom:?}");
                            assert!((top - expected).abs() <= tolerance && (bottom - expected).abs() <= tolerance);
                            assert!(menu.read(cx).scroll.offset().y.abs() <= tolerance,
                                "内容可完整显示时 hover 不得触发滚动");
                        }
                        window.remove_window();
                    }).unwrap();
                    cx.update_window(main.into(), |_, window, _| window.remove_window())
                        .unwrap();
                }
            }
        }
    }

    #[gpui::test]
    fn constrained_popup_keeps_hover_padding_when_keyboard_scrolls_to_last_item(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui::init);
        for font_size in [14, 24] {
            for scale_factor in [1., 1.25, 1.5, 1.75, 2.] {
                cx.update(|cx| theme::apply(crate::theme::Theme::Dark, cx));
                let mut workspace = None;
                let main = cx.open_window(size(px(850.), px(800.)), |window, cx| {
                    let view = cx.new(|cx| CommandWorkspace::new(window, cx));
                    workspace = Some(view.clone());
                    Root::new(view, window, cx)
                });
                let workspace = workspace.unwrap();
                cx.update(|cx| theme::set_font_size(font_size, cx));
                let (sender, _receiver) = mpsc::channel();
                let mut menu = None;
                let popup = cx.open_window(size(px(240.), px(180.)), |window, cx| {
                    let view = cx.new(|cx| TrayPopup::new(workspace, true, sender, window, cx));
                    menu = Some(view.clone());
                    Root::new(view, window, cx)
                });
                let menu = menu.unwrap();
                cx.simulate_window_scale_factor_change(popup.into(), scale_factor);
                cx.update_window(popup.into(), |_, window, cx| {
                    window.render_frame(cx);
                    window.press("end", cx);
                    window.hover(TrayAction::Quit.id(), cx);
                    window.render_frame(cx);
                    let surface = window.find("tray-popup-surface").bounds();
                    let last = window.find(TrayAction::Quit.id()).bounds();
                    let left = last.left() - surface.left();
                    let right = surface.right() - last.right();
                    let bottom = surface.bottom() - last.bottom();
                    let tolerance = px(1. / scale_factor + 0.01);
                    assert!((left - right).abs() <= tolerance && (left - bottom).abs() <= tolerance,
                        "{font_size}px/{scale_factor}x: 滚动到末项后左右／底部留白不等：{left:?}/{right:?}/{bottom:?}");
                    assert!((left - px(PADDING + 1.)).abs() <= tolerance);
                    window.press("home", cx);
                    window.render_frame(cx);
                    let first = window.find(TrayAction::ToggleWindow.id()).bounds();
                    assert!((first.top() - surface.top() - left).abs() <= tolerance);
                    window.scroll("tray-popup-items", gpui::ScrollDelta::Pixels(point(px(-1000.),px(-1000.))), cx);
                    window.render_frame(cx);
                    assert_eq!(menu.read(cx).scroll.offset().x, px(0.), "不能横向滚动造成右侧留白错位");
                    let last = window.find(TrayAction::Quit.id()).bounds();
                    assert!((surface.bottom() - last.bottom() - left).abs() <= tolerance,
                        "滚轮到底也必须保留相同底部留白");
                    window.remove_window();
                }).unwrap();
                cx.update_window(main.into(), |_, window, _| window.remove_window())
                    .unwrap();
            }
        }
    }

    #[test]
    fn popup_bounds_stay_in_small_or_negative_origin_workareas() {
        for work in [
            Bounds::new(point(px(0.), px(0.)), size(px(800.), px(600.))),
            Bounds::new(point(px(-1600.), px(-500.)), size(px(1200.), px(700.))),
            Bounds::new(point(px(0.), px(0.)), size(px(200.), px(260.))),
        ] {
            for anchor in [work.origin, work.bottom_right()] {
                let result = placement(anchor, work, size(px(360.), px(460.)));
                assert!(result.left() >= work.left() && result.top() >= work.top());
                assert!(result.right() <= work.right() && result.bottom() <= work.bottom());
            }
        }
    }
    #[test]
    fn keyboard_navigation_skips_disabled_commands_and_wraps() {
        let state = TrayMenuState {
            language: crate::i18n::Language::Chinese,
            visible: true,
            can_hide: true,
            can_run: false,
            can_stop: false,
            can_save: false,
            theme: crate::theme::Theme::Dark,
        };
        assert_eq!(next_enabled(&state, None, 1), Some(0));
        assert_eq!(next_enabled(&state, Some(0), 1), Some(3));
        assert_eq!(next_enabled(&state, Some(0), -1), Some(7));
        assert_eq!(next_enabled(&state, Some(7), 1), Some(0));
    }
}
