//! Windows-only native popup drawing. Muda still owns HMENU, keyboard navigation and command dispatch.
use crate::{
    i18n::{self, Language},
    theme::{self, Accent, Fonts, Theme, UiFontSize, UiFontWeight},
};
use gpui::{App, Global, Subscription};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Debug, PartialEq)]
struct Appearance {
    background: u32,
    foreground: u32,
    hover: u32,
    muted: u32,
    font: String,
    font_size: f32,
    font_weight: i32,
    labels: [String; 2],
}

impl Appearance {
    fn from_app(cx: &App) -> Self {
        let p = theme::palette(cx);
        let language = cx.try_global::<Language>().copied().unwrap_or_default();
        Self {
            background: p.panel,
            foreground: p.text,
            hover: p.hover,
            muted: p.muted,
            font: cx.global::<Fonts>().body.to_string(),
            font_size: theme::font_size(cx),
            font_weight: theme::font_weight(cx).0 as i32,
            labels: [
                i18n::translate(language, "显示主窗口").into(),
                i18n::translate(language, "退出…").into(),
            ],
        }
    }

    fn row_height(&self, dpi: u32) -> u32 {
        ((self.font_size * 1.4 + 12.).max(32.) * scale(dpi)).ceil() as u32
    }
}

fn scale(dpi: u32) -> f32 {
    dpi.max(96) as f32 / 96.
}

fn app_style_allowed(high_contrast: bool) -> bool {
    !high_contrast
}

fn colorref(rgb: u32) -> u32 {
    ((rgb & 0xff) << 16) | (rgb & 0xff00) | ((rgb >> 16) & 0xff)
}

/// Shared snapshots only: native callbacks never borrow the GPUI workspace/Root.
#[derive(Clone)]
pub(crate) struct StyleHandle(Rc<RefCell<Appearance>>);

impl StyleHandle {
    pub fn new(cx: &App) -> Self {
        Self(Rc::new(RefCell::new(Appearance::from_app(cx))))
    }

    pub fn sync(&self, cx: &App) {
        let next = Appearance::from_app(cx);
        if *self.0.borrow() != next {
            *self.0.borrow_mut() = next;
        }
    }

    fn observe<G: Global>(&self, cx: &mut App) -> Subscription {
        let handle = self.clone();
        cx.observe_global::<G>(move |cx| handle.sync(cx))
    }

    pub fn observe_changes(&self, cx: &mut App) -> Vec<Subscription> {
        vec![
            self.observe::<Theme>(cx),
            self.observe::<Accent>(cx),
            self.observe::<Fonts>(cx),
            self.observe::<UiFontSize>(cx),
            self.observe::<UiFontWeight>(cx),
            self.observe::<Language>(cx),
        ]
    }
}

#[cfg(target_os = "windows")]
mod native {
    use super::*;
    use std::{cell::Cell, mem::size_of, ptr};
    use tray_icon::menu::{ContextMenu, Menu};
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM},
        Graphics::Gdi::{
            CreateFontW, CreateSolidBrush, DeleteObject, DrawFocusRect, DrawTextW, FillRect, GetDC,
            GetTextExtentPoint32W, MonitorFromPoint, ReleaseDC, RestoreDC, SaveDC, SelectObject,
            SetBkMode, SetTextColor, CLEARTYPE_QUALITY, DEFAULT_CHARSET, DT_LEFT, DT_NOPREFIX,
            DT_SINGLELINE, DT_VCENTER, HBRUSH, HDC, HFONT, MONITOR_DEFAULTTONEAREST, TRANSPARENT,
        },
        UI::{
            Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW},
            Controls::{
                DRAWITEMSTRUCT, MEASUREITEMSTRUCT, ODS_DISABLED, ODS_FOCUS, ODS_SELECTED, ODT_MENU,
            },
            HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI},
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::{
                GetCursorPos, GetMenuInfo, GetMenuItemInfoW, SetMenuInfo, SetMenuItemInfoW,
                SystemParametersInfoW, HMENU, MENUINFO, MENUITEMINFOW, MFT_OWNERDRAW, MIIM_FTYPE,
                MIIM_ID, MIM_BACKGROUND, SPI_GETHIGHCONTRAST, WM_DRAWITEM, WM_INITMENUPOPUP,
                WM_MEASUREITEM, WM_NCDESTROY,
            },
        },
    };

    const SUBCLASS: usize = 0x45435254; // ECRT, separate from muda/tray-icon subclass IDs.

    struct Item {
        id: u32,
        original_type: u32,
    }

    struct DrawState {
        menu: HMENU,
        style: StyleHandle,
        items: Vec<Item>,
        hwnd: Cell<HWND>,
        dpi: Cell<u32>,
        brush: Cell<HBRUSH>,
        brush_color: Cell<Option<u32>>,
        original_background: HBRUSH,
        custom: Cell<bool>,
    }

    impl DrawState {
        unsafe fn set_custom(&self, enabled: bool) -> Result<(), String> {
            for item in &self.items {
                let info = MENUITEMINFOW {
                    cbSize: size_of::<MENUITEMINFOW>() as u32,
                    fMask: MIIM_FTYPE,
                    fType: item.original_type | if enabled { MFT_OWNERDRAW } else { 0 },
                    ..Default::default()
                };
                if SetMenuItemInfoW(self.menu, item.id, 0, &info) == 0 {
                    return Err("无法更新 Windows 托盘菜单绘制模式".into());
                }
            }
            self.custom.set(enabled);
            Ok(())
        }

        unsafe fn apply(&self) -> Result<(), String> {
            // Accessibility always wins over the application's optional styling.
            let mut contrast = HIGHCONTRASTW {
                cbSize: size_of::<HIGHCONTRASTW>() as u32,
                ..Default::default()
            };
            if SystemParametersInfoW(
                SPI_GETHIGHCONTRAST,
                contrast.cbSize,
                &mut contrast as *mut _ as _,
                0,
            ) == 0
            {
                return Err("无法查询 Windows 高对比度设置".into());
            }
            if !app_style_allowed(contrast.dwFlags & HCF_HIGHCONTRASTON != 0) {
                self.set_custom(false)?;
                self.background(self.original_background)?;
                return Ok(());
            }
            let rgb = self.style.0.borrow().background;
            if self.brush_color.get() != Some(rgb) {
                let brush = CreateSolidBrush(colorref(rgb));
                if brush.is_null() {
                    return Err("无法创建托盘菜单背景画刷".into());
                }
                if let Err(error) = self.background(brush) {
                    DeleteObject(brush);
                    return Err(error);
                }
                let old = self.brush.replace(brush);
                if !old.is_null() {
                    DeleteObject(old);
                }
                self.brush_color.set(Some(rgb));
            } else {
                self.background(self.brush.get())?;
            }
            self.set_custom(true)
        }

        unsafe fn background(&self, brush: HBRUSH) -> Result<(), String> {
            let info = MENUINFO {
                cbSize: size_of::<MENUINFO>() as u32,
                fMask: MIM_BACKGROUND,
                hbrBack: brush,
                ..Default::default()
            };
            if SetMenuInfo(self.menu, &info) == 0 {
                Err("无法设置托盘菜单背景".into())
            } else {
                Ok(())
            }
        }

        unsafe fn restore(&self) {
            let _ = self.set_custom(false);
            let _ = self.background(self.original_background);
        }

        fn index(&self, id: u32) -> Option<usize> {
            self.items.iter().position(|item| item.id == id)
        }
    }

    struct Font(HFONT);
    impl Font {
        unsafe fn new(style: &Appearance, dpi: u32) -> Option<Self> {
            let family: Vec<u16> = style.font.encode_utf16().chain(Some(0)).collect();
            let font = CreateFontW(
                -(style.font_size * scale(dpi)).round() as i32,
                0,
                0,
                0,
                style.font_weight,
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                0,
                0,
                CLEARTYPE_QUALITY as u32,
                0,
                family.as_ptr(),
            );
            (!font.is_null()).then_some(Self(font))
        }
    }
    impl Drop for Font {
        fn drop(&mut self) {
            unsafe {
                DeleteObject(self.0);
            }
        }
    }

    /// Delegates all behavior to muda and only decorates this tray owner's native menu.
    pub(crate) struct ThemedMenu {
        inner: Menu,
        state: Box<DrawState>, // Stable address until the owner's subclass is detached.
    }

    impl ThemedMenu {
        pub fn new(inner: Menu, style: StyleHandle) -> Result<Self, String> {
            unsafe {
                let menu = inner.hpopupmenu() as HMENU;
                let mut original = MENUINFO {
                    cbSize: size_of::<MENUINFO>() as u32,
                    fMask: MIM_BACKGROUND,
                    ..Default::default()
                };
                if GetMenuInfo(menu, &mut original) == 0 {
                    return Err("无法读取托盘菜单背景".into());
                }
                let mut items = Vec::new();
                for position in 0..2 {
                    let mut info = MENUITEMINFOW {
                        cbSize: size_of::<MENUITEMINFOW>() as u32,
                        fMask: MIIM_ID | MIIM_FTYPE,
                        ..Default::default()
                    };
                    if GetMenuItemInfoW(menu, position, 1, &mut info) == 0 {
                        return Err("无法读取托盘菜单项".into());
                    }
                    items.push(Item {
                        id: info.wID,
                        original_type: info.fType,
                    });
                }
                Ok(Self {
                    inner,
                    state: Box::new(DrawState {
                        menu,
                        style,
                        items,
                        hwnd: Cell::new(ptr::null_mut()),
                        dpi: Cell::new(96),
                        brush: Cell::new(ptr::null_mut()),
                        brush_color: Cell::new(None),
                        original_background: original.hbrBack,
                        custom: Cell::new(false),
                    }),
                })
            }
        }
    }

    impl ContextMenu for ThemedMenu {
        fn hpopupmenu(&self) -> isize {
            self.inner.hpopupmenu()
        }
        unsafe fn show_context_menu_for_hwnd(
            &self,
            hwnd: isize,
            position: Option<tray_icon::dpi::Position>,
        ) -> bool {
            let _ = self.state.apply();
            self.inner.show_context_menu_for_hwnd(hwnd, position)
        }
        unsafe fn attach_menu_subclass_for_hwnd(&self, hwnd: isize) {
            self.inner.attach_menu_subclass_for_hwnd(hwnd);
            if SetWindowSubclass(
                hwnd as HWND,
                Some(draw_menu),
                SUBCLASS,
                &*self.state as *const DrawState as usize,
            ) == 0
            {
                eprintln!("Windows 托盘菜单主题不可用：无法安装绘制处理，保留原生菜单");
                return;
            }
            self.state.hwnd.set(hwnd as HWND);
            if let Err(error) = self.state.apply() {
                self.state.restore();
                eprintln!("Windows 托盘菜单主题不可用：{error}；保留原生菜单");
            }
        }
        unsafe fn detach_menu_subclass_from_hwnd(&self, hwnd: isize) {
            self.state.restore();
            RemoveWindowSubclass(hwnd as HWND, Some(draw_menu), SUBCLASS);
            self.state.hwnd.set(ptr::null_mut());
            self.inner.detach_menu_subclass_from_hwnd(hwnd);
        }
    }

    impl Drop for ThemedMenu {
        fn drop(&mut self) {
            unsafe {
                let hwnd = self.state.hwnd.get();
                if !hwnd.is_null() {
                    self.detach_menu_subclass_from_hwnd(hwnd as isize);
                }
                // Restore HMENU before its custom brush is deleted or muda destroys it.
                self.state.restore();
                let brush = self.state.brush.get();
                if !brush.is_null() {
                    DeleteObject(brush);
                }
            }
        }
    }

    unsafe extern "system" fn draw_menu(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _: usize,
        data: usize,
    ) -> LRESULT {
        // Never unwind over the Win32 callback boundary.
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dispatch(hwnd, message, wparam, lparam, &*(data as *const DrawState))
        }))
        .unwrap_or_else(|_| DefSubclassProc(hwnd, message, wparam, lparam))
    }

    unsafe fn dispatch(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        state: &DrawState,
    ) -> LRESULT {
        if message == WM_INITMENUPOPUP && wparam == state.menu as usize {
            // The tray can be on a different-DPI monitor from its hidden owner.
            let mut cursor = POINT::default();
            let mut x = 96;
            let mut y = 96;
            let dpi = if GetCursorPos(&mut cursor) != 0
                && GetDpiForMonitor(
                    MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST),
                    MDT_EFFECTIVE_DPI,
                    &mut x,
                    &mut y,
                ) == 0
            {
                x
            } else {
                GetDpiForWindow(hwnd)
            };
            state.dpi.set(dpi.max(96));
            if let Err(error) = state.apply() {
                state.restore();
                eprintln!("托盘菜单主题更新失败：{error}");
            }
        }
        if message == WM_NCDESTROY {
            RemoveWindowSubclass(hwnd, Some(draw_menu), SUBCLASS);
            state.hwnd.set(ptr::null_mut());
        }
        if !state.custom.get() {
            return DefSubclassProc(hwnd, message, wparam, lparam);
        }
        let Some(style) = state.style.0.try_borrow().ok().map(|style| style.clone()) else {
            return DefSubclassProc(hwnd, message, wparam, lparam);
        };
        let dpi = state.dpi.get();
        if message == WM_MEASUREITEM && lparam != 0 {
            let measure = &mut *(lparam as *mut MEASUREITEMSTRUCT);
            if measure.CtlType == ODT_MENU && state.index(measure.itemID).is_some() {
                let dc = GetDC(hwnd);
                if dc.is_null() {
                    return DefSubclassProc(hwnd, message, wparam, lparam);
                }
                let Some(font) = Font::new(&style, dpi) else {
                    ReleaseDC(hwnd, dc);
                    return DefSubclassProc(hwnd, message, wparam, lparam);
                };
                let saved = SaveDC(dc);
                if saved == 0 {
                    ReleaseDC(hwnd, dc);
                    return DefSubclassProc(hwnd, message, wparam, lparam);
                }
                SelectObject(dc, font.0);
                let mut width = 0;
                for text in &style.labels {
                    let text: Vec<u16> = text.encode_utf16().collect();
                    let mut size = SIZE::default();
                    if GetTextExtentPoint32W(dc, text.as_ptr(), text.len() as i32, &mut size) != 0 {
                        width = width.max(size.cx);
                    }
                }
                RestoreDC(dc, saved);
                ReleaseDC(hwnd, dc);
                measure.itemWidth = (width + (32. * scale(dpi)).ceil() as i32)
                    .max((160. * scale(dpi)).ceil() as i32)
                    as u32;
                measure.itemHeight = style.row_height(dpi);
                return 1;
            }
        }
        if message == WM_DRAWITEM && lparam != 0 {
            let item = &*(lparam as *const DRAWITEMSTRUCT);
            if item.CtlType == ODT_MENU && item.hwndItem as HMENU == state.menu {
                if let Some(index) = state.index(item.itemID) {
                    draw_item(item.hDC, item.rcItem, item.itemState, &style, index, dpi);
                    return 1;
                }
            }
        }
        DefSubclassProc(hwnd, message, wparam, lparam)
    }

    unsafe fn draw_item(
        dc: HDC,
        mut rect: RECT,
        flags: u32,
        style: &Appearance,
        index: usize,
        dpi: u32,
    ) {
        let selected = flags & ODS_SELECTED != 0;
        let brush = CreateSolidBrush(colorref(if selected {
            style.hover
        } else {
            style.background
        }));
        if !brush.is_null() {
            FillRect(dc, &rect, brush);
            DeleteObject(brush);
        }
        let Some(font) = Font::new(style, dpi) else {
            return;
        };
        let saved = SaveDC(dc);
        if saved == 0 {
            return;
        }
        SelectObject(dc, font.0);
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(
            dc,
            colorref(if flags & ODS_DISABLED != 0 {
                style.muted
            } else {
                style.foreground
            }),
        );
        let padding = (16. * scale(dpi)).round() as i32;
        rect.left += padding;
        rect.right -= padding;
        let text: Vec<u16> = style.labels[index].encode_utf16().collect();
        DrawTextW(
            dc,
            text.as_ptr(),
            text.len() as i32,
            &mut rect,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
        if flags & ODS_FOCUS != 0 {
            DrawFocusRect(dc, &rect);
        }
        RestoreDC(dc, saved);
    }
}

#[cfg(target_os = "windows")]
pub(crate) use native::ThemedMenu;

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    #[test]
    fn high_contrast_keeps_native_system_presentation() {
        assert!(app_style_allowed(false));
        assert!(!app_style_allowed(true));
    }

    #[test]
    fn rgb_conversion_and_dpi_row_metrics_are_stable() {
        assert_eq!(colorref(0x123456), 0x563412);
        assert_eq!(scale(0), 1.);
        assert_eq!(scale(144), 1.5);
        let style = Appearance {
            background: 0,
            foreground: 0,
            hover: 0,
            muted: 0,
            font: "Segoe UI".into(),
            font_size: 14.,
            font_weight: 400,
            labels: ["Show".into(), "Quit".into()],
        };
        assert_eq!(style.row_height(96), 32);
        assert_eq!(style.row_height(192), 64);
        assert!(
            Appearance {
                font_size: 24.,
                ..style
            }
            .row_height(96)
                >= 45
        );
    }

    #[gpui::test]
    fn snapshots_use_the_app_palette_not_the_system_theme(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
            let dark = Appearance::from_app(cx);
            let palette = theme::palette(cx);
            assert_eq!(dark.background, palette.panel);
            assert_eq!(dark.foreground, palette.text);
            assert_eq!(dark.hover, palette.hover);
            theme::apply(Theme::Light, cx);
            let light = Appearance::from_app(cx);
            assert_eq!(light.background, theme::palette(cx).panel);
            assert_eq!(light.foreground, theme::palette(cx).text);
            assert_ne!(dark.background, light.background);
        });
    }

    #[gpui::test]
    fn appearance_observers_follow_theme_typography_and_language(cx: &mut TestAppContext) {
        let (handle, observers) = cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
            i18n::apply(Language::Chinese, cx);
            let handle = StyleHandle::new(cx);
            let observers = handle.observe_changes(cx);
            (handle, observers)
        });
        cx.run_until_parked();
        let dark = handle.0.borrow().clone();
        cx.update(|cx| {
            theme::apply(Theme::Light, cx);
            theme::set_font_size(24, cx);
            theme::set_font_weight(700, cx);
            i18n::apply(Language::English, cx);
        });
        cx.run_until_parked();
        let updated = handle.0.borrow().clone();
        assert_ne!(dark.background, updated.background);
        assert_eq!(updated.font_size, 24.);
        assert_eq!(updated.font_weight, 700);
        assert_eq!(updated.labels[0], "Show main window");
        assert_eq!(updated.labels[1], "Quit…");
        drop(observers);
    }
}
