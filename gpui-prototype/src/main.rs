// Real Windows application builds must not open a console window on double-click.
// Keep test/visual harness builds on the console subsystem for useful diagnostics.
#![cfg_attr(
    all(windows, not(test), not(feature = "ui-test")),
    windows_subsystem = "windows"
)]

extern crate gpui_kit as gpui;

mod app;
mod app_assets;
mod backend;
mod components;
mod core;
mod i18n;
mod icon_picker;
mod log_window;
mod notifications;
mod settings;
mod state;
mod tab_icons;
mod theme;
mod tokens;
mod tray;
#[cfg(feature = "ui-test")]
mod ui_tests;

use gpui::component::{Root, TitleBar};
use gpui::{prelude::*, px, size, App, Bounds, WindowBounds, WindowDecorations, WindowOptions};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

fn main() {
    #[cfg(feature = "ui-test")]
    if let Some(directory) = std::env::args().skip(1).next() {
        ui_tests::snapshots(&directory);
        return;
    }
    let store = backend::ConfigStore::default_location();
    let (startup_config, load_error) = match store.load() {
        Ok(config) => (config, None),
        Err(error) => {
            eprintln!("{error}");
            (None, Some(error))
        }
    };
    gpui::application()
        .with_assets(app_assets::AppAssets)
        .run(move |cx: &mut App| {
            gpui::init(cx);
            notifications::init(cx);
            theme::apply(theme::Theme::Dark, cx);
            let bounds = Bounds::centered(None, size(px(850.), px(800.)), cx);
            let mut titlebar = TitleBar::title_bar_options();
            titlebar.title = Some("EasyCommandRunner — GPUI".into());
            titlebar.traffic_light_position = Some(gpui::point(px(12.), px(14.)));
            let tray_active = Rc::new(Cell::new(false));
            let close_to_tray = tray_active.clone();
            let window_tray_active = tray_active.clone();
            let workspace_slot = Rc::new(RefCell::new(None));
            let workspace_slot_for_window = workspace_slot.clone();
            let startup_store = store.clone();
            let startup_config = startup_config.clone();
            let startup_error = load_error.clone();
            let window_handle = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        titlebar: Some(titlebar),
                        window_min_size: Some(size(px(640.), px(660.))),
                        window_decorations: Some(WindowDecorations::Client),
                        ..TitleBar::window_options()
                    },
                    move |window, cx| {
                        window.on_window_should_close(cx, move |window, cx| {
                            if close_to_tray.get() {
                                tray::hide_to_tray(window, cx);
                                false
                            } else {
                                tray::can_close_without_tray(window, cx)
                            }
                        });
                        let view = cx.new(|cx| {
                            let mut view = app::CommandWorkspace::new_with_backend(
                                window,
                                cx,
                                Some(startup_store.clone()),
                                startup_config.clone(),
                                startup_error.clone(),
                            );
                            view.set_close_to_tray(window_tray_active.clone());
                            view
                        });
                        *workspace_slot_for_window.borrow_mut() = Some(view.clone());
                        cx.new(|cx| Root::new(view, window, cx))
                    },
                )
                .expect("无法创建 GPUI 窗口");
            let workspace_handle = workspace_slot.borrow().clone().expect("未创建 GPUI 工作区");
            let main_window_id = window_handle.window_id();
            cx.on_window_closed(move |cx, closed| {
                // 独立日志窗口不是主窗口的替代品。无托盘时主窗口真正关闭后，
                // 即使日志窗口还开着也必须退出并清理子进程。
                if closed == main_window_id || cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            cx.spawn(async move |cx| {
                // macOS 要在 NSApplication 事件循环启动后才能创建状态栏图标。
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                let language = cx.update(|cx| *cx.global::<i18n::Language>());
                let mut tray =
                    match tray::TrayRuntime::new(window_handle, workspace_handle, language) {
                        Ok(tray) => tray,
                        Err(error) => {
                            eprintln!("无法创建系统托盘，关闭窗口将正常退出：{error}");
                            return;
                        }
                    };
                let mut tick = 0;
                loop {
                    match tray.ready() {
                        Ok(ready) => {
                            let was_ready = tray_active.replace(ready);
                            #[cfg(target_os = "windows")]
                            if was_ready && !ready {
                                cx.update(|app| tray.recover_if_hidden(app));
                            }
                            #[cfg(not(target_os = "windows"))]
                            let _ = was_ready;
                        }
                        Err(error) => {
                            tray_active.set(false);
                            eprintln!("系统托盘不可用，关闭窗口将正常退出：{error}");
                            break;
                        }
                    }
                    if cx.update(|app| tray.poll(app, tick))
                        && cx.update(|app| tray.request_exit(app))
                    {
                        // Drop the Windows tray icon before GPUI shuts down its message loop.
                        break;
                    }
                    tick = tick.wrapping_add(1);
                    cx.background_executor()
                        .timer(Duration::from_millis(200))
                        .await;
                }
            })
            .detach();
            cx.activate(true);
        });
}
