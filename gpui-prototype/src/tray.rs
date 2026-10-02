//! 托盘图标保持系统配色；仅 Windows 的原生右键菜单复用主窗口外观。
//! macOS 使用模板图标及原生菜单；Linux 使用 GTK 桌面主题，不套应用内主题。
use crate::{
    app::CommandWorkspace,
    i18n::{self, Language},
};
use gpui::component::Root;
use gpui::{App, Entity, Window, WindowHandle};
use resvg::{tiny_skia, usvg};
#[cfg(target_os = "linux")]
use std::time::Duration;
use tray_icon::{
    menu::{ContextMenu, Menu, MenuEvent, MenuItem},
    Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
};

const SVG: &str = include_str!("../../resources/tray_icon.svg");
const ICON_ID: &str = "ecr-gpui-tray";
const SHOW_ID: &str = "ecr-tray-show";
const QUIT_ID: &str = "ecr-tray-quit";

// Linux 的 tray-icon 按 ID 写入 PNG；固定 ID 会让同一用户的多个实例相互覆盖/删除图标。
fn tray_id() -> String {
    format!("{ICON_ID}-{}", std::process::id())
}

fn rgba(width: u32, height: u32, dark: bool) -> Result<Vec<u8>, String> {
    let tree = usvg::Tree::from_str(SVG, &usvg::Options::default()).map_err(|e| e.to_string())?;
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or("无法创建托盘图标画布")?;
    let svg = tree.size();
    let scale = (width as f32 / svg.width()).min(height as f32 / svg.height());
    let x = (width as f32 - svg.width() * scale) / 2.;
    let y = (height as f32 - svg.height() * scale) / 2.;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_row(scale, 0., 0., scale, x, y),
        &mut pixmap.as_mut(),
    );
    // tiny-skia 使用预乘 RGBA；tray-icon 要求非预乘。只取形状的 alpha，
    // 颜色由系统深浅模式决定，不能把 SVG 自带的白色直接当作浅色模式图标。
    let color = if dark { 255 } else { 0 };
    let mut output = Vec::with_capacity((width * height * 4) as usize);
    for pixel in pixmap.data().chunks_exact(4) {
        output.extend_from_slice(&[color, color, color, pixel[3]]);
    }
    Ok(output)
}

fn icon(dark: bool) -> Result<Icon, String> {
    #[cfg(target_os = "macos")]
    let (width, height) = (64, 40); // 原图 1.6:1；菜单栏显示为约 29×18pt 的模板图标。
    #[cfg(not(target_os = "macos"))]
    let (width, height) = (64, 64); // 任务栏/状态区使用正方形图标，内容保持原始比例。
    Icon::from_rgba(rgba(width, height, dark)?, width, height).map_err(|e| e.to_string())
}

struct TrayLabels {
    show: MenuItem,
    quit: MenuItem,
}
impl TrayLabels {
    fn translate(&self, language: Language) {
        self.show.set_text(i18n::translate(language, "显示主窗口"));
        self.quit.set_text(i18n::translate(language, "退出…"));
    }
}

fn build_tray(
    dark: bool,
    language: Language,
    wrap_menu: impl FnOnce(Menu) -> Result<Box<dyn ContextMenu>, String>,
) -> Result<(TrayIcon, TrayLabels), String> {
    let menu = Menu::new();
    let labels = TrayLabels {
        show: MenuItem::with_id(SHOW_ID, i18n::translate(language, "显示主窗口"), true, None),
        quit: MenuItem::with_id(QUIT_ID, i18n::translate(language, "退出…"), true, None),
    };
    menu.append(&labels.show).map_err(|e| e.to_string())?;
    menu.append(&labels.quit).map_err(|e| e.to_string())?;
    let icon = TrayIconBuilder::new()
        .with_id(tray_id())
        .with_icon(icon(dark)?)
        .with_icon_as_template(cfg!(target_os = "macos"))
        .with_menu(wrap_menu(menu)?)
        .with_menu_on_left_click(cfg!(target_os = "macos"))
        .with_tooltip("EasyCommandRunner · GPUI")
        .build()
        .map_err(|e| e.to_string())?;
    Ok((icon, labels))
}

#[cfg(target_os = "windows")]
fn system_dark() -> bool {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
        .and_then(|key| key.get_value::<u32, _>("SystemUsesLightTheme"))
        .map(|value| value == 0)
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn system_dark() -> bool {
    use gtk::prelude::*;
    gtk::Settings::default().is_some_and(|settings| {
        settings.is_gtk_application_prefer_dark_theme()
            || settings
                .gtk_theme_name()
                .is_some_and(|name| name.to_ascii_lowercase().contains("dark"))
    })
}

#[cfg(target_os = "macos")]
fn system_dark() -> bool {
    false
} // NSImage 模板由 macOS 按菜单栏实际外观自动着色。

#[cfg(target_os = "windows")]
fn windows_hwnd(window: &Window) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = HasWindowHandle::window_handle(window).ok()?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return None;
    };
    Some(handle.hwnd.get())
}

#[cfg(target_os = "windows")]
fn windows_window_hidden(window: &Window) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::IsWindowVisible;
    windows_hwnd(window).is_some_and(|hwnd| unsafe { IsWindowVisible(hwnd as *mut _) == 0 })
}

#[cfg(target_os = "windows")]
fn set_windows_visibility(window: &Window, visible: bool) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE, SW_SHOW};
    let Some(hwnd) = windows_hwnd(window) else {
        return false;
    };
    unsafe {
        ShowWindow(hwnd as *mut _, if visible { SW_SHOW } else { SW_HIDE });
    }
    true
}

/// 仅在托盘实际可用时关闭转托盘；不可用时改走未保存确认／退出路径。
pub fn hide_to_tray(window: &mut Window, cx: &mut App) {
    #[cfg(target_os = "macos")]
    {
        let _ = window;
        cx.hide();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = cx;
        if !set_windows_visibility(window, false) {
            window.minimize_window();
        }
    }
    #[cfg(target_os = "linux")]
    {
        let _ = cx;
        window.minimize_window();
    } // GPUI Linux 尚未提供跨 X11/Wayland 的窗口 hide API。
}

/// 托盘失效时，关闭窗口仍须保护未保存的会话编辑。
pub fn can_close_without_tray(window: &mut Window, cx: &mut App) -> bool {
    let Some(root) = window.root::<Root>().flatten() else {
        return true;
    };
    let Ok(workspace) = root.read(cx).view().clone().downcast::<CommandWorkspace>() else {
        return true;
    };
    if workspace.read(cx).has_unsaved_edits_now(cx) || workspace.read(cx).has_running_commands() {
        workspace.update(cx, |workspace, cx| workspace.request_exit(window, cx));
        false
    } else {
        true
    }
}

fn restore(handle: WindowHandle<Root>, cx: &mut App) {
    #[cfg(target_os = "macos")]
    if let Some(mtm) = objc2_foundation::MainThreadMarker::new() {
        objc2_app_kit::NSApplication::sharedApplication(mtm).unhide(None);
    }
    cx.activate(true);
    if let Err(error) = handle.update(cx, |_, window, _| {
        #[cfg(target_os = "windows")]
        {
            set_windows_visibility(window, true);
        }
        window.activate_window();
    }) {
        eprintln!("托盘无法恢复主窗口：{error}");
    }
}

pub struct TrayRuntime {
    window: WindowHandle<Root>,
    workspace: Entity<CommandWorkspace>,
    language: Language,
    #[cfg(not(target_os = "linux"))]
    labels: TrayLabels,
    #[cfg(target_os = "linux")]
    language_updates: std::sync::mpsc::Sender<Language>,
    #[cfg(target_os = "windows")]
    menu_style: crate::windows_tray_menu::StyleHandle,
    #[cfg(target_os = "windows")]
    _theme_observers: Vec<gpui::Subscription>,
    #[cfg(not(target_os = "linux"))]
    _icon: TrayIcon,
    #[cfg(target_os = "windows")]
    dark: bool,
    #[cfg(target_os = "linux")]
    ready: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
    #[cfg(target_os = "linux")]
    alive: std::sync::Arc<std::sync::atomic::AtomicBool>,
    #[cfg(target_os = "linux")]
    shutdown: std::sync::mpsc::Sender<()>,
}

impl TrayRuntime {
    pub fn new(
        window: WindowHandle<Root>,
        workspace: Entity<CommandWorkspace>,
        language: Language,
        cx: &mut App,
    ) -> Result<Self, String> {
        #[cfg(not(target_os = "windows"))]
        let _ = cx;
        #[cfg(not(target_os = "linux"))]
        {
            let dark = system_dark();
            #[cfg(target_os = "windows")]
            let menu_style = crate::windows_tray_menu::StyleHandle::new(cx);
            #[cfg(target_os = "windows")]
            let (icon, labels) = build_tray(dark, language, |menu| {
                crate::windows_tray_menu::ThemedMenu::new(menu, menu_style.clone())
                    .map(|menu| Box::new(menu) as Box<dyn ContextMenu>)
            })?;
            #[cfg(not(target_os = "windows"))]
            let (icon, labels) = build_tray(dark, language, |menu| Ok(Box::new(menu)))?;
            Ok(Self {
                window,
                workspace,
                language,
                labels,
                #[cfg(target_os = "windows")]
                _theme_observers: menu_style.observe_changes(cx),
                #[cfg(target_os = "windows")]
                menu_style,
                _icon: icon,
                #[cfg(target_os = "windows")]
                dark,
            })
        }
        #[cfg(target_os = "linux")]
        {
            let (language_updates, language_rx) = std::sync::mpsc::channel();
            let (ready_tx, ready) = std::sync::mpsc::channel();
            let (shutdown, shutdown_rx) = std::sync::mpsc::channel();
            let alive = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let worker_alive = alive.clone();
            std::thread::Builder::new()
                .name("ecr-tray-gtk".into())
                .spawn(move || {
                    let setup = (|| {
                        gtk::init().map_err(|e| e.to_string())?;
                        let dark = system_dark();
                        let (tray, labels) = build_tray(dark, language, |menu| Ok(Box::new(menu)))?;
                        Ok::<_, String>((tray, labels, dark))
                    })();
                    let (tray, labels, mut dark) = match setup {
                        Ok(value) => {
                            worker_alive.store(true, std::sync::atomic::Ordering::Release);
                            let _ = ready_tx.send(Ok(()));
                            value
                        }
                        Err(error) => {
                            let _ = ready_tx.send(Err(error));
                            return;
                        }
                    };
                    let timer = gtk::glib::timeout_add_local(Duration::from_secs(1), move || {
                        if shutdown_rx.try_recv().is_ok() {
                            gtk::main_quit();
                            return gtk::glib::ControlFlow::Break;
                        }
                        for language in language_rx.try_iter() {
                            labels.translate(language);
                        }
                        let next = system_dark();
                        if next != dark {
                            if let Ok(icon) = icon(next) {
                                if let Err(error) = tray.set_icon(Some(icon)) {
                                    eprintln!("托盘换色失败：{error}");
                                } else {
                                    dark = next;
                                }
                            }
                        }
                        gtk::glib::ControlFlow::Continue
                    });
                    gtk::main();
                    // shutdown 定时器返回 Break 时 source 已被 GLib 移除，再 remove 会 panic。
                    // 如果 GTK 因其他原因退出，则主动释放尚存活的托盘/定时器。
                    if let Some(source) =
                        gtk::glib::MainContext::default().find_source_by_id(&timer)
                    {
                        source.destroy();
                    }
                    worker_alive.store(false, std::sync::atomic::Ordering::Release);
                })
                .map_err(|e| e.to_string())?;
            Ok(Self {
                window,
                workspace,
                language,
                language_updates,
                ready: Some(ready),
                alive,
                shutdown,
            })
        }
    }

    /// Linux 托盘运行在 GTK 事件线程，确认图标创建成功之后才允许关闭转托盘。
    pub fn ready(&mut self) -> Result<bool, String> {
        #[cfg(target_os = "windows")]
        return Ok(self._icon.rect().is_some()); // build() 不保证 Explorer 已成功注册图标。
        #[cfg(target_os = "linux")]
        if self.ready.is_none() && !self.alive.load(std::sync::atomic::Ordering::Acquire) {
            return Err("GTK 托盘线程已退出".into());
        }
        #[cfg(target_os = "linux")]
        if let Some(receiver) = &self.ready {
            match receiver.try_recv() {
                Ok(Ok(())) => {
                    self.ready = None;
                    return Ok(true);
                }
                Ok(Err(error)) => {
                    self.ready = None;
                    return Err(error);
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.ready = None;
                    return Err("GTK 托盘线程提前退出".into());
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return Ok(false),
            }
        }
        #[cfg(not(target_os = "windows"))]
        Ok(true)
    }

    /// 返回 true 表示已进入立即退出路径，调用方应结束托盘轮询并释放图标。
    pub fn request_exit(&self, cx: &mut App) -> bool {
        let needs_confirmation = {
            let workspace = self.workspace.read(cx);
            workspace.has_unsaved_edits_now(cx) || workspace.has_running_commands()
        };
        if needs_confirmation {
            // Both confirmation paths must be visible and focused, even when
            // the main window was hidden in the tray before Exit was chosen.
            restore(self.window, cx);
        }
        // Use the untyped handle: WindowHandle<Root>::update leases Root first,
        // then request_exit -> window.open_dialog would try to lease Root again.
        // That re-entrant Root update is why tray-triggered modals were not shown.
        let window: gpui::AnyWindowHandle = self.window.into();
        let stop_polling = match window.update(cx, |_, window, cx| {
            self.workspace
                .update(cx, |workspace, cx| workspace.request_exit(window, cx))
        }) {
            Ok(stop_polling) => stop_polling,
            Err(error) => {
                eprintln!("退出前无法访问主窗口：{error}");
                cx.quit();
                true
            }
        };
        if !stop_polling && !needs_confirmation {
            // The live-state check inside request_exit is authoritative. If it
            // discovered an edit after the preliminary check, reveal the dialog now.
            restore(self.window, cx);
        }
        stop_polling
    }

    #[cfg(target_os = "windows")]
    pub fn recover_if_hidden(&self, cx: &mut App) {
        if self
            .window
            .update(cx, |_, window, _| windows_window_hidden(window))
            .unwrap_or(false)
        {
            restore(self.window, cx); // Explorer 消失时不能把窗口困在不可见的托盘中。
        }
    }

    /// 返回 true 表示用户请求退出（交由统一的确认流程处理）。
    pub fn poll(&mut self, cx: &mut App, tick: usize) -> bool {
        #[cfg(target_os = "windows")]
        self.menu_style.sync(cx); // Also covers startup/config reload, without touching Root.
        let language = cx.try_global::<Language>().copied().unwrap_or_default();
        if language != self.language {
            #[cfg(not(target_os = "linux"))]
            self.labels.translate(language);
            #[cfg(target_os = "linux")]
            let _ = self.language_updates.send(language);
            self.language = language;
        }
        for event in MenuEvent::receiver().try_iter() {
            match event.id.0.as_str() {
                SHOW_ID => restore(self.window, cx),
                QUIT_ID => return true,
                _ => {}
            }
        }
        for event in TrayIconEvent::receiver().try_iter() {
            if let TrayIconEvent::DoubleClick { id, .. } = event {
                if id.0 == tray_id() {
                    restore(self.window, cx);
                }
            }
        }
        #[cfg(target_os = "windows")]
        if tick % 5 == 0 {
            // 图标仍匹配任务栏系统主题；右键菜单单独跟随应用内外观。
            let dark = system_dark();
            if dark != self.dark {
                match icon(dark)
                    .and_then(|icon| self._icon.set_icon(Some(icon)).map_err(|e| e.to_string()))
                {
                    Ok(()) => self.dark = dark,
                    Err(error) => eprintln!("托盘换色失败：{error}"),
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        let _ = tick;
        false
    }
}

#[cfg(target_os = "linux")]
impl Drop for TrayRuntime {
    fn drop(&mut self) {
        let _ = self.shutdown.send(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_id_is_unique_to_this_process() {
        assert_eq!(tray_id(), format!("{ICON_ID}-{}", std::process::id()));
    }

    #[test]
    fn icon_uses_svg_alpha_with_white_on_dark_and_black_on_light() {
        for (width, height) in [(16, 16), (32, 32), (64, 40), (64, 64)] {
            let white = rgba(width, height, true).unwrap();
            let black = rgba(width, height, false).unwrap();
            let mut opaque = 0;
            let mut transparent = 0;
            for (a, b) in white.chunks_exact(4).zip(black.chunks_exact(4)) {
                assert_eq!(a[3], b[3]);
                if a[3] > 240 {
                    opaque += 1;
                    assert_eq!(&a[..3], &[255, 255, 255]);
                    assert_eq!(&b[..3], &[0, 0, 0]);
                }
                if a[3] == 0 {
                    transparent += 1;
                }
            }
            assert!(opaque > 0 && transparent > 0, "图标必须有轮廓和透明边缘");
        }
    }
}
