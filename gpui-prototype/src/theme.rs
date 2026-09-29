use crate::tokens::*;
use gpui::component::{Theme as KitTheme, ThemeMode};
use gpui::{px, rgb, App, Global, SharedString};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    Dark,
    Light,
}
impl Global for Theme {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Accent {
    Blue,
    Teal,
    Violet,
    Emerald,
    Cyan,
    Rose,
    Pink,
    Orange,
    Amber,
    Slate,
}
impl Global for Accent {}
impl Accent {
    pub const ALL: [Self; 10] = [
        Self::Blue,
        Self::Teal,
        Self::Violet,
        Self::Emerald,
        Self::Cyan,
        Self::Rose,
        Self::Pink,
        Self::Orange,
        Self::Amber,
        Self::Slate,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Blue => "雾蓝",
            Self::Teal => "青绿",
            Self::Violet => "紫罗兰",
            Self::Emerald => "翡翠",
            Self::Cyan => "湖蓝",
            Self::Rose => "玫瑰",
            Self::Pink => "莓粉",
            Self::Orange => "暖橙",
            Self::Amber => "琥珀",
            Self::Slate => "石墨",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Blue => "blue",
            Self::Teal => "teal",
            Self::Violet => "violet",
            Self::Emerald => "emerald",
            Self::Cyan => "cyan",
            Self::Rose => "rose",
            Self::Pink => "pink",
            Self::Orange => "orange",
            Self::Amber => "amber",
            Self::Slate => "slate",
        }
    }
    pub fn from_key(key: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|accent| accent.key() == key)
            .unwrap_or(Self::Blue)
    }
}
pub const FONT_WEIGHTS: [u16; 6] = [300, 400, 500, 600, 700, 800];
pub fn weight_label(weight: u16) -> &'static str {
    match weight {
        300 => "细体",
        500 => "中等",
        600 => "半粗",
        700 => "粗体",
        800 => "特粗",
        _ => "常规",
    }
}
pub fn normalized_weight(weight: u16) -> u16 {
    if FONT_WEIGHTS.contains(&weight) {
        weight
    } else {
        400
    }
}

#[derive(Clone, Copy)]
pub struct UiFontSize(pub u8);
impl Global for UiFontSize {}
pub fn font_size(cx: &App) -> f32 {
    cx.global::<UiFontSize>().0 as f32
}
#[derive(Clone, Copy)]
pub struct UiFontWeight(pub u16);
impl Global for UiFontWeight {}
pub fn font_weight(cx: &App) -> gpui::FontWeight {
    gpui::FontWeight(cx.global::<UiFontWeight>().0 as f32)
}
pub fn set_font_weight(weight: u16, cx: &mut App) {
    cx.set_global(UiFontWeight(normalized_weight(weight)));
}
pub fn set_font_size(size: u8, cx: &mut App) {
    cx.set_global(UiFontSize(size.clamp(10, 24)));
}
pub fn set_reduced_motion(reduced: bool, cx: &mut App) {
    let t = KitTheme::global_mut(cx);
    t.motion.duration_fast = std::time::Duration::from_millis(if reduced { 0 } else { 100 });
    t.motion.duration_normal = std::time::Duration::from_millis(if reduced { 0 } else { 160 });
    t.motion.duration_slow = std::time::Duration::from_millis(if reduced { 0 } else { 220 });
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub app: u32,
    pub panel: u32,
    pub subtle: u32,
    pub control: u32,
    pub button: u32,
    pub hover: u32,
    pub pressed: u32,
    pub border: u32,
    pub divider: u32,
    pub focus: u32,
    pub text: u32,
    pub muted: u32,
    pub selection: u32,
    pub selected_tab: u32,
    pub primary: u32,
    pub primary_hover: u32,
    pub primary_border: u32,
    pub on_primary: u32,
    pub danger: u32,
    pub danger_hover: u32,
}
impl Palette {
    fn with_accent(mut self, mode: Theme, accent: Accent) -> Self {
        let (color, hover, active, selection) = match (mode, accent) {
            (Theme::Dark, Accent::Blue) => (0x667985, 0x7a8d99, 0x526675, 0x344454),
            (Theme::Dark, Accent::Teal) => (0x166b63, 0x218176, 0x105a53, 0x244640),
            (Theme::Dark, Accent::Violet) => (0x654f88, 0x79639c, 0x523e73, 0x423450),
            (Theme::Light, Accent::Blue) => (0x2563eb, 0x1d4ed8, 0x1e40af, 0xbfdbfe),
            (Theme::Light, Accent::Teal) => (0x0f766e, 0x0d655e, 0x115e59, 0xccfbf1),
            (Theme::Light, Accent::Violet) => (0x6d28d9, 0x5b21b6, 0x4c1d95, 0xede9fe),
            (Theme::Dark, Accent::Emerald) => (0x176b4b, 0x20805b, 0x10573d, 0x203e32),
            (Theme::Dark, Accent::Cyan) => (0x22677b, 0x2e7b91, 0x1a5365, 0x233c44),
            (Theme::Dark, Accent::Rose) => (0x9f3454, 0xb44765, 0x882c47, 0x4d2631),
            (Theme::Dark, Accent::Pink) => (0x943568, 0xaa477c, 0x7e2a58, 0x48273c),
            (Theme::Dark, Accent::Orange) => (0xa34b26, 0xba5a30, 0x8b3b1d, 0x482e22),
            (Theme::Dark, Accent::Amber) => (0x946219, 0xa77529, 0x7b5010, 0x433620),
            (Theme::Dark, Accent::Slate) => (0x526477, 0x65798b, 0x435365, 0x2d3743),
            (Theme::Light, Accent::Emerald) => (0x047857, 0x065f46, 0x064e3b, 0xd1fae5),
            (Theme::Light, Accent::Cyan) => (0x0e7490, 0x155e75, 0x164e63, 0xcffafe),
            (Theme::Light, Accent::Rose) => (0xbe123c, 0x9f1239, 0x881337, 0xffe4e6),
            (Theme::Light, Accent::Pink) => (0xbe185d, 0x9d174d, 0x831843, 0xfce7f3),
            (Theme::Light, Accent::Orange) => (0xc2410c, 0x9a3412, 0x7c2d12, 0xffedd5),
            (Theme::Light, Accent::Amber) => (0xb45309, 0x92400e, 0x78350f, 0xfef3c7),
            (Theme::Light, Accent::Slate) => (0x475569, 0x334155, 0x1e293b, 0xe2e8f0),
        };
        self.focus = color;
        self.primary = color;
        self.primary_hover = hover;
        self.primary_border = active;
        self.selected_tab = color;
        self.selection = selection;
        self.on_primary = 0xffffff;
        self
    }
}

impl Theme {
    pub fn palette(self) -> Palette {
        match self {
            Self::Dark => Palette {
                app: 0x121414,
                panel: 0x151717,
                subtle: 0x111313,
                control: 0x181818,
                button: 0x202323,
                hover: 0x2b2f2f,
                pressed: 0x161919,
                border: 0x303436,
                divider: 0x282c2e,
                focus: 0x667985,
                text: 0xe2e4e5,
                muted: 0x8b9498,
                selection: 0x344454,
                selected_tab: 0x343838,
                primary: 0x1a263b,
                primary_hover: 0x233350,
                primary_border: 0x344962,
                on_primary: 0xedf3fa,
                danger: 0xe58b8b,
                danger_hover: 0x382b2b,
            },
            Self::Light => Palette {
                app: 0xf3f4f6,
                panel: 0xffffff,
                subtle: 0xf9fafb,
                control: 0xffffff,
                button: 0xffffff,
                hover: 0xf3f4f6,
                pressed: 0xe5e7eb,
                border: 0xd1d5db,
                divider: 0xe5e7eb,
                focus: 0x2563eb,
                text: 0x111827,
                muted: 0x6b7280,
                selection: 0xbfdbfe,
                selected_tab: 0xffffff,
                primary: 0x10b981,
                primary_hover: 0x059669,
                primary_border: 0x059669,
                on_primary: 0xffffff,
                danger: 0xdc2626,
                danger_hover: 0xfee2e2,
            },
        }
    }
    pub fn toggle(self) -> Self {
        if self == Self::Dark {
            Self::Light
        } else {
            Self::Dark
        }
    }
}

pub fn palette(cx: &App) -> Palette {
    let mode = *cx.global::<Theme>();
    mode.palette().with_accent(mode, *cx.global::<Accent>())
}

pub struct Fonts {
    pub body: SharedString,
    pub code: SharedString,
}
impl Global for Fonts {}
impl Fonts {
    pub fn detect(cx: &App) -> Self {
        let available = cx.text_system().all_font_names();
        let pick = |names: &[&str]| -> SharedString {
            names
                .iter()
                .find(|n| available.iter().any(|a| a == **n))
                .unwrap_or(&names[names.len() - 1])
                .to_string()
                .into()
        };
        #[cfg(target_os = "windows")]
        let body = pick(&["Microsoft YaHei UI", "Microsoft YaHei", "Segoe UI"]);
        #[cfg(target_os = "macos")]
        let body = pick(&[".AppleSystemUIFont", "Helvetica Neue", "Helvetica"]);
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let body = pick(&["Noto Sans CJK SC", "Noto Sans", "DejaVu Sans"]);
        Self {
            body,
            code: pick(&[
                "Cascadia Mono",
                "Consolas",
                "Menlo",
                "DejaVu Sans Mono",
                "Courier New",
            ]),
        }
    }
}

/// Kit 的行为层保持不变，只映射应用的语义配色；尺寸不随主题切换。
pub fn apply(mode: Theme, cx: &mut App) {
    KitTheme::change(
        if mode == Theme::Dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        None,
        cx,
    );
    let fonts = Fonts::detect(cx);
    let accent = cx.try_global::<Accent>().copied().unwrap_or(Accent::Blue);
    let p = mode.palette().with_accent(mode, accent);
    let t = KitTheme::global_mut(cx);
    // Kit 用 font_size 作为 rem 基准，保持 16px；正文/代码在组件层明确为 14/13px。
    t.font_size = px(16.);
    t.font_family = fonts.body.clone();
    t.mono_font_family = fonts.code.clone();
    t.mono_font_size = px(CODE);
    t.radius = px(RADIUS);
    t.radius_lg = px(RADIUS);
    t.shadow = false;
    t.background = rgb(p.panel).into();
    t.foreground = rgb(p.text).into();
    t.muted = rgb(p.subtle).into();
    t.muted_foreground = rgb(p.muted).into();
    t.border = rgb(p.border).into();
    t.input = rgb(p.border).into();
    t.ring = rgb(p.focus).into();
    t.selection = rgb(p.selection).into();
    t.caret = rgb(p.text).into();
    t.button = rgb(p.button).into();
    t.button_foreground = rgb(p.text).into();
    t.button_hover = rgb(p.hover).into();
    t.button_active = rgb(p.pressed).into();
    t.primary = rgb(p.focus).into();
    t.primary_foreground = rgb(p.on_primary).into();
    t.button_primary = rgb(p.primary).into();
    t.button_primary_foreground = rgb(p.on_primary).into();
    t.button_primary_hover = rgb(p.primary_hover).into();
    t.button_primary_active = rgb(p.primary_border).into();
    t.danger = rgb(p.danger).into();
    t.danger_hover = rgb(p.danger_hover).into();
    t.secondary = rgb(p.button).into();
    t.secondary_hover = rgb(p.hover).into();
    t.secondary_active = rgb(p.pressed).into();
    t.secondary_foreground = rgb(p.text).into();
    t.accent = rgb(p.hover).into();
    t.accent_foreground = rgb(p.text).into();
    t.popover = rgb(p.panel).into();
    t.popover_foreground = rgb(p.text).into();
    t.colors.list = rgb(p.panel).into();
    t.list_hover = rgb(p.hover).into();
    t.list_active = rgb(p.selection).into();
    t.tab = rgb(p.app).into();
    t.tab_bar = rgb(p.app).into();
    t.tab_foreground = rgb(p.muted).into();
    t.tab_active = rgb(p.selected_tab).into();
    t.tab_active_foreground = rgb(p.text).into();
    t.title_bar = rgb(p.app).into();
    t.title_bar_border = rgb(p.divider).into();

    // GPUI Kit 组件使用 ThemeToken 快照，不只读取兼容字段。
    t.tokens.background = gpui::Hsla::from(rgb(p.app)).into();
    t.tokens.foreground = gpui::Hsla::from(rgb(p.text)).into();
    t.tokens.border = gpui::Hsla::from(rgb(p.border)).into();
    t.tokens.muted = gpui::Hsla::from(rgb(p.subtle)).into();
    t.tokens.muted_foreground = gpui::Hsla::from(rgb(p.muted)).into();
    t.tokens.input = gpui::Hsla::from(rgb(p.border)).into();
    t.tokens.accent = gpui::Hsla::from(rgb(p.hover)).into();
    t.tokens.accent_foreground = gpui::Hsla::from(rgb(p.text)).into();
    t.tokens.primary = gpui::Hsla::from(rgb(p.focus)).into();
    t.tokens.primary_foreground = gpui::Hsla::from(rgb(p.on_primary)).into();
    t.tokens.primary_hover = gpui::Hsla::from(rgb(p.primary_hover)).into();
    t.tokens.primary_active = gpui::Hsla::from(rgb(p.primary_border)).into();
    t.tokens.button = gpui::Hsla::from(rgb(p.button)).into();
    t.tokens.button_foreground = gpui::Hsla::from(rgb(p.text)).into();
    t.tokens.button_hover = gpui::Hsla::from(rgb(p.hover)).into();
    t.tokens.button_active = gpui::Hsla::from(rgb(p.pressed)).into();
    t.tokens.button_primary = gpui::Hsla::from(rgb(p.primary)).into();
    t.tokens.button_primary_foreground = gpui::Hsla::from(rgb(p.on_primary)).into();
    t.tokens.button_primary_hover = gpui::Hsla::from(rgb(p.primary_hover)).into();
    t.tokens.button_primary_active = gpui::Hsla::from(rgb(p.primary_border)).into();
    // Pill tabs read ThemeToken::secondary on hover, not the legacy secondary_hover field.
    t.tokens.secondary = gpui::Hsla::from(rgb(p.hover)).into();
    t.tokens.button_secondary = gpui::Hsla::from(rgb(p.button)).into();
    t.tokens.button_secondary_foreground = gpui::Hsla::from(rgb(p.text)).into();
    t.tokens.button_secondary_hover = gpui::Hsla::from(rgb(p.hover)).into();
    t.tokens.button_secondary_active = gpui::Hsla::from(rgb(p.pressed)).into();
    t.tokens.button_danger = gpui::Hsla::from(rgb(p.danger)).into();
    t.tokens.button_danger_foreground = gpui::Hsla::from(rgb(p.on_primary)).into();
    t.tokens.button_danger_hover = gpui::Hsla::from(rgb(p.danger_hover)).into();
    t.tokens.button_danger_active = gpui::Hsla::from(rgb(p.danger)).into();
    t.tokens.popover = gpui::Hsla::from(rgb(p.panel)).into();
    t.tokens.popover_foreground = gpui::Hsla::from(rgb(p.text)).into();
    t.tokens.list = gpui::Hsla::from(rgb(p.panel)).into();
    // Kit 的 Select 列表外层 ListItem 和内层 SearchableListItemElement 都绘制 hover；
    // 只保留内层的高光，避免鼠标移到文字时出现两层叠加的亮块。
    t.tokens.list_hover = gpui::Hsla::from(rgb(p.hover)).opacity(0.).into();
    t.tokens.list_active = gpui::Hsla::from(rgb(p.selection)).into();
    t.tokens.tab = gpui::Hsla::from(rgb(p.app)).into();
    t.tokens.tab_foreground = gpui::Hsla::from(rgb(p.muted)).into();
    t.tokens.tab_active = gpui::Hsla::from(rgb(p.selected_tab)).into();
    t.tokens.tab_active_foreground = gpui::Hsla::from(rgb(p.text)).into();
    t.tokens.tab_bar = gpui::Hsla::from(rgb(p.app)).into();
    t.motion.duration_fast = std::time::Duration::from_millis(100);
    t.motion.duration_normal = std::time::Duration::from_millis(160);
    t.motion.duration_slow = std::time::Duration::from_millis(220);
    KitTheme::sync_base(cx);
    cx.set_global(mode);
    cx.set_global(accent);
    cx.set_global(fonts);
    cx.set_global(UiFontSize(14));
    cx.set_global(UiFontWeight(400));
}
