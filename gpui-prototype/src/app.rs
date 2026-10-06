use crate::{
    backend::{self, ConfigStore, OutputStream, RunEvent, RunHandle},
    components::*,
    core::parser::CommandParser,
    i18n::{self, tr, Language},
    log_output::LogAutoScroll,
    log_window::DetachedLogWindow,
    notifications,
    page_transition::{self, PageSnapshot, PageTransition, PageVisual},
    settings::{Preferences, SettingsPanel},
    state::{CommandTab, LogItem, LogItems, Parameter, RowData, TabData},
    tab_icons,
    theme::{self, palette, Accent, Fonts, Theme},
    tokens::*,
    tray,
    tray_actions::{TrayAction, TrayMenuState},
};
use futures::io::AsyncReadExt as _;
use gpui::assets::IconName;
use gpui::component::{
    button::{Button, ButtonCustomVariant, ButtonVariants},
    input::{InputState, Paste, TextareaState},
    menu::DropdownMenu,
    scroll::{Scrollbar, ScrollbarMode},
    select::{Select, SelectEvent, SelectState},
    tab::{Tab, TabBar, TabVariant},
    Disableable, Icon, IndexPath, Root, Selectable, Sizable, Size, StyledExt, TitleBar, WindowExt,
};
use gpui::{
    div, prelude::*, px, rgb, Animation, AnimationExt, App, AppContext, ClipboardEntry,
    ClipboardItem, Context, Entity, Focusable, IntoElement, MouseButton, Render, ScrollHandle,
    Subscription, Window, WindowBounds, WindowControlArea, WindowDecorations, WindowHandle,
    WindowOptions,
};
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct SidebarGroup {
    id: u64,
    name: String,
    collapsed: bool,
}

impl Default for SidebarGroup {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            collapsed: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SidebarEntry {
    Section(Option<u64>),
    Tab(usize),
}

#[derive(Clone, Copy)]
enum SidebarGroupEdit {
    Create,
    CreateAndMove(usize),
    Rename(u64),
}

struct SessionLog {
    id: usize,
    // Immutable command captured at launch, never the editor's subsequently changed preview.
    // Drop it on completion; completed logs keep only output and metadata.
    command: Option<String>,
    command_name: String,
    caption: String,
    output: String,
    finished: bool,
    exit_code: Option<i32>,
    stopped: bool,
}

pub(crate) struct LogSnapshot {
    pub items: Vec<LogItem>,
    pub selected_id: Option<usize>,
    pub output: String,
    pub can_delete: bool,
    pub can_stop: bool,
    pub selected_command: Option<String>,
    pub font_size: u8,
}

pub(crate) fn log_zoom_delta(event: &gpui::KeyDownEvent) -> Option<i16> {
    let m = event.keystroke.modifiers;
    let zoom_modifier =
        (m.control && !m.platform) || (cfg!(target_os = "macos") && m.platform && !m.control);
    if !zoom_modifier || m.alt {
        return None;
    }
    match event.keystroke.key.as_str() {
        "-" => Some(-1),
        "=" | "+" => Some(1),
        _ => None,
    }
}

pub struct CommandWorkspace {
    /// All configurations, including ones not currently open in the title bar.
    pub tabs: Vec<CommandTab>,
    /// Runtime IDs in top-bar order; the sidebar always enumerates all `tabs`.
    pub(crate) open_tab_ids: Vec<usize>,
    pub active: usize,
    /// Runtime ID -> last saved disk index (unchanged by unsaved sidebar sorting).
    saved_slots: std::collections::HashMap<usize, usize>,
    saved_digest: Option<u64>,
    saved_tab_data: std::collections::HashMap<usize, serde_json::Value>,
    sidebar_groups: Vec<SidebarGroup>,
    next_sidebar_group_id: u64,
    sidebar_ungrouped_collapsed: bool,
    tab_group_options_open: bool,
    pub(crate) sidebar_collapsed: bool,
    /// Transient hover preview; never saved to config or treated as a manual width change.
    sidebar_auto_expanded: bool,
    /// Clicking to collapse while still hovering must not reopen until the pointer leaves.
    sidebar_auto_suppressed: bool,
    /// Preferred expanded width; never overwrite it when the window temporarily shrinks.
    sidebar_expanded_width: Option<f32>,
    sidebar_display_width: f32,
    /// Last painted hover panel width; used to dismiss it on mouse movement even
    /// when GPUI replaces the hovered rail with the overlay in the same frame.
    sidebar_overlay_display_width: f32,
    sidebar_hovered: Option<usize>,
    next_tab_id: usize,
    store: Option<ConfigStore>,
    config_writable: bool,
    config_dirty: bool,
    sidebar_group_name_input: Option<Entity<InputState>>,
    pending_icon_sources: std::collections::HashMap<usize, String>,
    logs: Vec<SessionLog>,
    run_handles: std::collections::HashMap<usize, RunHandle>,
    pub(crate) selected_log_id: Option<usize>,
    next_run_id: usize,
    log_selector: Entity<SelectState<LogItems>>,
    pub(crate) log_output: Entity<TextareaState>,
    pub(crate) log_auto_scroll: LogAutoScroll,
    // Only the non-interactive sheets move. Live input entities and IME geometry stay stable.
    page_switch_epoch: usize,
    page_switch: Option<(usize, i8)>,
    page_transition: Option<PageTransition>,
    pub preview: Entity<TextareaState>,
    tab_scroll: ScrollHandle,
    sidebar_scroll: ScrollHandle,
    row_scroll: ScrollHandle,
    pub show_log: bool,
    pub(crate) settings_panel: Option<Entity<SettingsPanel>>,
    pub(crate) log_font_size: u8,
    pub(crate) detached_log: Option<WindowHandle<Root>>,
    detaching_log: bool,
    log_window_closed: Option<Subscription>,
    pub font_size: u8,
    pub font_weight: u16,
    pub reduced_motion: bool,
    pub enabled_first: bool,
    // 工作区级偏好：切换标签及隐藏日志不重置，布局钳制只影响本次显示。
    pub(crate) log_height_override: Option<f32>,
    description_width_override: Option<f32>,
    resizing: Option<ResizeDrag>,
    drag_target: Option<(usize, bool)>,
    row_drag_auto_scroll: Option<RowDragAutoScroll>,
    row_drag_auto_scroll_running: bool,
    row_drag_auto_scroll_generation: u64,
    tab_drag_target: Option<(TabArea, usize, bool)>,
    // Window-local X: draw above the Kit tab-bar indicator, not inside a label.
    tab_drop_x: Option<f32>,
    tab_drag_centers: std::collections::HashMap<(TabArea, usize), f32>,
    tab_menu: Option<(usize, f32, f32)>,
    pub(crate) status: String,
    close_to_tray: Option<Rc<Cell<bool>>>,
    exit_dialog_open: Rc<Cell<bool>>,
    _subscriptions: Vec<Subscription>,
}

impl CommandWorkspace {
    fn load_sidebar_groups(settings: Option<&serde_json::Value>) -> Vec<SidebarGroup> {
        let Some(items) = settings.and_then(|settings| settings["sidebar_groups"].as_array())
        else {
            return Vec::new();
        };
        let mut ids = std::collections::HashSet::new();
        let mut names = std::collections::HashSet::new();
        items
            .iter()
            .filter_map(|item| {
                let id = item["id"].as_u64()?;
                let name = item["name"].as_str()?.trim();
                if id == u64::MAX
                    || name.is_empty()
                    || name.eq_ignore_ascii_case("未分组")
                    || name.eq_ignore_ascii_case("Unassigned")
                {
                    return None;
                }
                if !ids.insert(id) || !names.insert(name.to_lowercase()) {
                    return None;
                }
                Some(SidebarGroup {
                    id,
                    name: name.to_owned(),
                    collapsed: item["collapsed"].as_bool().unwrap_or(false),
                })
            })
            .collect()
    }

    fn sidebar_entries(&self) -> Vec<SidebarEntry> {
        if self.sidebar_groups.is_empty() {
            return (0..self.tabs.len()).map(SidebarEntry::Tab).collect();
        }
        let ungrouped = self
            .tabs
            .iter()
            .enumerate()
            .filter_map(|(index, tab)| tab.sidebar_group_id.is_none().then_some(index))
            .collect::<Vec<_>>();
        let mut entries = Vec::new();
        if !ungrouped.is_empty() {
            entries.push(SidebarEntry::Section(None));
            if !self.sidebar_ungrouped_collapsed {
                entries.extend(ungrouped.into_iter().map(SidebarEntry::Tab));
            }
        }
        for group in &self.sidebar_groups {
            entries.push(SidebarEntry::Section(Some(group.id)));
            if !group.collapsed {
                entries.extend(self.tabs.iter().enumerate().filter_map(|(index, tab)| {
                    (tab.sidebar_group_id == Some(group.id)).then_some(SidebarEntry::Tab(index))
                }));
            }
        }
        entries
    }

    fn sidebar_entry_index_for_tab(&self, tab_index: usize) -> usize {
        let entries = self.sidebar_entries();
        if let Some(index) = entries
            .iter()
            .position(|entry| *entry == SidebarEntry::Tab(tab_index))
        {
            return index;
        }
        let group_id = self
            .tabs
            .get(tab_index)
            .and_then(|tab| tab.sidebar_group_id);
        entries
            .iter()
            .position(|entry| *entry == SidebarEntry::Section(group_id))
            .unwrap_or(tab_index)
    }

    fn scroll_sidebar_to_tab(&mut self, tab_index: usize) {
        let entry_index = self.sidebar_entry_index_for_tab(tab_index);
        self.sidebar_scroll.scroll_to_item(entry_index);
    }

    fn scroll_sidebar_to_top_of_tab(&mut self, tab_index: usize) {
        let entry_index = self.sidebar_entry_index_for_tab(tab_index);
        self.sidebar_scroll.scroll_to_top_of_item(entry_index);
    }

    fn reveal_sidebar_tab(&mut self, tab_index: usize, cx: &mut Context<Self>) {
        let Some(group_id) = self
            .tabs
            .get(tab_index)
            .and_then(|tab| tab.sidebar_group_id)
        else {
            if !self.sidebar_groups.is_empty() && self.sidebar_ungrouped_collapsed {
                self.sidebar_ungrouped_collapsed = false;
                self.config_dirty = true;
                cx.notify();
            }
            return;
        };
        if let Some(group) = self
            .sidebar_groups
            .iter_mut()
            .find(|group| group.id == group_id)
        {
            if group.collapsed {
                group.collapsed = false;
                self.config_dirty = true;
                cx.notify();
            }
        }
    }

    #[cfg(any(test, feature = "ui-test"))]
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new_with_backend(window, cx, None, None, None)
    }

    pub fn new_with_backend(
        window: &mut Window,
        cx: &mut Context<Self>,
        store: Option<ConfigStore>,
        config: Option<serde_json::Value>,
        load_error: Option<String>,
    ) -> Self {
        let settings = config.as_ref().and_then(|value| value.get("gpui"));
        let sidebar_groups = Self::load_sidebar_groups(settings);
        let sidebar_group_ids = sidebar_groups
            .iter()
            .map(|group| group.id)
            .collect::<std::collections::HashSet<_>>();
        let loaded_theme = config.as_ref().and_then(|value| value["theme"].as_str());
        let mode = match loaded_theme {
            Some("light") => Theme::Light,
            Some("dark") => Theme::Dark,
            _ => *cx.global::<Theme>(),
        };
        let accent = Accent::from_key(
            settings
                .and_then(|value| value["accent"].as_str())
                .unwrap_or("blue"),
        );
        cx.set_global(accent);
        theme::apply(mode, cx);
        let font_size = settings
            .and_then(|value| value["font_size"].as_u64())
            .unwrap_or(14)
            .clamp(10, 24) as u8;
        let font_weight = theme::normalized_weight(
            settings
                .and_then(|s| s["font_weight"].as_u64())
                .unwrap_or(400)
                .min(800) as u16,
        );
        let language = Language::from_code(
            settings
                .and_then(|s| s["language"].as_str())
                .unwrap_or("zh_CN"),
        );
        i18n::apply(language, cx);
        let reduced_motion = settings
            .and_then(|value| value["reduced_motion"].as_bool())
            .unwrap_or(false);
        theme::set_font_size(font_size, cx);
        theme::set_font_weight(font_weight, cx);
        theme::set_reduced_motion(reduced_motion, cx);

        let mut tabs = config
            .as_ref()
            .and_then(|value| value["tabs"].as_array())
            .into_iter()
            .flatten()
            .filter_map(|tab| serde_json::from_value::<TabData>(tab.clone()).ok())
            .enumerate()
            .map(|(id, data)| CommandTab::new(id, data, window, cx))
            .collect::<Vec<_>>();
        for tab in &mut tabs {
            if tab
                .sidebar_group_id
                .is_some_and(|group_id| !sidebar_group_ids.contains(&group_id))
            {
                tab.sidebar_group_id = None;
            }
        }
        if tabs.is_empty() {
            let data = if config.is_some() {
                TabData {
                    name: i18n::format(cx, "标签{}", &["1"]),
                    rows: vec![RowData::default()],
                    ..Default::default()
                }
            } else {
                TabData::example()
            };
            tabs.push(CommandTab::new(0, data, window, cx));
        }
        if let Some(items) = config.as_ref().and_then(|value| value["tabs"].as_array()) {
            for (index, tab) in tabs.iter_mut().enumerate() {
                tab.row_height_override = items
                    .get(index)
                    .and_then(|value| value["gpui_row_height"].as_u64())
                    .map(|rows| (rows as usize).clamp(1, 9));
            }
        }
        let enabled_first = settings
            .and_then(|value| value["enabled_first"].as_bool())
            .unwrap_or(false);
        if enabled_first {
            for tab in &mut tabs {
                tab.rows.sort_by_key(|row| !row.enabled);
            }
        }
        let remembered = store.as_ref().and_then(ConfigStore::load_tab_session);
        let session_stale =
            store.as_ref().is_some_and(ConfigStore::has_tab_session) && remembered.is_none();
        // An external edit may reorder Qt's tabs without updating GPUI-only indices.
        let indices = if session_stale {
            None
        } else {
            remembered
                .as_ref()
                .map(|(open, _)| open.clone())
                .or_else(|| {
                    settings
                        .and_then(|settings| settings["open_tabs"].as_array())
                        .map(|indices| {
                            indices
                                .iter()
                                .filter_map(|index| index.as_u64().map(|index| index as usize))
                                .collect()
                        })
                })
        };
        let mut open_tab_ids = Vec::new();
        for index in indices.unwrap_or_else(|| (0..tabs.len()).collect()) {
            if let Some(tab) = tabs.get(index) {
                if !open_tab_ids.contains(&tab.id) {
                    open_tab_ids.push(tab.id);
                }
            }
        }
        let requested_active = remembered
            .map(|(_, active)| active)
            .or_else(|| session_stale.then_some(0))
            .or_else(|| {
                config
                    .as_ref()
                    .and_then(|value| value["current_tab_index"].as_u64())
                    .map(|index| index as usize)
            })
            .unwrap_or(0)
            .min(tabs.len() - 1);
        let active = if open_tab_ids.contains(&tabs[requested_active].id) {
            requested_active
        } else {
            open_tab_ids
                .first()
                .and_then(|id| tabs.iter().position(|tab| tab.id == *id))
                .unwrap_or(0)
        };
        let saved_slots = if store
            .as_ref()
            .is_some_and(|store| store.config_path().exists())
        {
            tabs.iter()
                .enumerate()
                .map(|(index, tab)| (tab.id, index))
                .collect()
        } else {
            std::collections::HashMap::new()
        };
        let next_tab_id = tabs.iter().map(|tab| tab.id).max().unwrap_or(0) + 1;
        let preview = cx.new(|cx| {
            TextareaState::new(window, cx)
                .default_value(tabs[active].command(cx))
                .placeholder(tr(cx, "命令预览 · 修改参数后实时更新"))
        });
        let log_selector = cx.new(|cx| SelectState::new(LogItems(Vec::new()), None, window, cx));
        let log_output =
            cx.new(|cx| TextareaState::new(window, cx).placeholder(tr(cx, "暂无运行记录。")));
        let log_subscription =
            cx.subscribe_in(&log_selector, window, |this, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(id)) = event {
                    this.select_log(*id, window, cx);
                }
            });
        let saved_tab_data = Self::tab_data_snapshot(&tabs, cx);
        let mut workspace = Self {
            tabs,
            open_tab_ids,
            active,
            saved_slots,
            saved_digest: store.as_ref().and_then(ConfigStore::config_digest),
            saved_tab_data,
            next_sidebar_group_id: sidebar_groups
                .iter()
                .map(|group| group.id)
                .max()
                .map_or(0, |id| id.saturating_add(1)),
            sidebar_groups,
            sidebar_ungrouped_collapsed: settings
                .and_then(|settings| settings["sidebar_ungrouped_collapsed"].as_bool())
                .unwrap_or(false),
            tab_group_options_open: false,
            sidebar_collapsed: settings
                .and_then(|s| s["sidebar_collapsed"].as_bool())
                .unwrap_or(false),
            sidebar_auto_expanded: false,
            sidebar_auto_suppressed: false,
            sidebar_expanded_width: settings
                .and_then(|s| s["sidebar_width"].as_f64())
                .filter(|width| width.is_finite())
                .map(|width| (width as f32).clamp(160., 400.)),
            sidebar_display_width: 0.,
            sidebar_overlay_display_width: 56.,
            sidebar_hovered: None,
            next_tab_id,
            store: store.clone(),
            config_writable: load_error.is_none(),
            config_dirty: false,
            sidebar_group_name_input: None,
            pending_icon_sources: std::collections::HashMap::new(),
            logs: Vec::new(),
            run_handles: std::collections::HashMap::new(),
            selected_log_id: None,
            next_run_id: 0,
            log_selector,
            log_output,
            log_auto_scroll: LogAutoScroll::default(),
            page_switch_epoch: 0,
            page_switch: None,
            page_transition: None,
            preview,
            tab_scroll: ScrollHandle::new(),
            sidebar_scroll: ScrollHandle::new(),
            row_scroll: ScrollHandle::new(),
            show_log: settings
                .and_then(|value| value["show_log"].as_bool())
                .unwrap_or(false),
            settings_panel: None,
            log_font_size: 14,
            detached_log: None,
            detaching_log: false,
            log_window_closed: None,
            font_size,
            font_weight,
            reduced_motion,
            enabled_first,
            log_height_override: settings
                .and_then(|value| value["log_height"].as_f64())
                .map(|height| (height as f32).clamp(32., 600.)),
            description_width_override: settings
                .and_then(|value| value["description_width"].as_f64())
                .map(|width| width as f32),
            resizing: None,
            drag_target: None,
            row_drag_auto_scroll: None,
            row_drag_auto_scroll_running: false,
            row_drag_auto_scroll_generation: 0,
            tab_drag_target: None,
            tab_drop_x: None,
            tab_drag_centers: std::collections::HashMap::new(),
            tab_menu: None,
            status: load_error.unwrap_or_else(|| {
                tr(
                    cx,
                    if config.is_some() {
                        "已恢复上次保存的配置与标签页"
                    } else {
                        "欢迎使用 EasyCommandRunner GPUI 版"
                    },
                )
            }),
            close_to_tray: None,
            exit_dialog_open: Rc::new(Cell::new(false)),
            _subscriptions: vec![log_subscription],
        };
        workspace.scroll_sidebar_to_tab(active);
        workspace
    }

    pub fn set_close_to_tray(&mut self, active: Rc<Cell<bool>>) {
        self.close_to_tray = Some(active);
    }

    fn tab_data_snapshot(
        tabs: &[CommandTab],
        cx: &App,
    ) -> std::collections::HashMap<usize, serde_json::Value> {
        tabs.iter()
            .map(|tab| {
                (
                    tab.id,
                    serde_json::to_value(tab.data(cx)).expect("tab data serializes"),
                )
            })
            .collect()
    }

    pub fn has_unsaved_edits(&self) -> bool {
        self.config_dirty || self.tabs.iter().any(|tab| tab.dirty)
    }

    /// Include live input values as well as asynchronously delivered Change events.
    /// This keeps exit confirmation reliable if a tray command races the last edit.
    pub fn has_unsaved_edits_now(&self, cx: &App) -> bool {
        self.has_unsaved_edits()
            || self.tabs.len() != self.saved_tab_data.len()
            || self.tabs.iter().any(|tab| {
                serde_json::to_value(tab.data(cx))
                    .map(|current| self.saved_tab_data.get(&tab.id) != Some(&current))
                    .unwrap_or(true)
            })
    }

    pub(crate) fn tray_menu_state(&self, visible: bool, cx: &App) -> TrayMenuState {
        TrayMenuState {
            language: *cx.global::<Language>(),
            visible,
            can_hide: !self.exit_dialog_open.get(),
            can_run: !self.open_tab_ids.is_empty() && !self.current().command(cx).trim().is_empty(),
            can_stop: self
                .selected_log_id
                .is_some_and(|id| self.run_handles.contains_key(&id)),
            can_save: self.config_writable && self.store.is_some(),
            theme: *cx.global::<Theme>(),
        }
    }

    /// Tray commands call the same workspace paths as the main UI. No draft autosave.
    pub(crate) fn perform_tray_action(
        &mut self,
        action: TrayAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.tray_menu_state(true, cx).enabled(action) {
            return;
        }
        match action {
            TrayAction::Run => self.run_current_command(window, cx),
            TrayAction::Stop => self.stop_selected_run(cx),
            TrayAction::Logs => {
                if self.detached_log.is_some() {
                    self.toggle_log(cx);
                } else {
                    if !self.show_log {
                        self.config_dirty = true;
                    }
                    self.show_log = true;
                    self.log_auto_scroll.reopen();
                    cx.notify();
                }
            }
            TrayAction::Save => {
                if let Err(error) = self.save_configuration(cx) {
                    self.status = error;
                    cx.notify();
                }
            }
            TrayAction::Settings => self.open_settings(window, cx),
            TrayAction::ToggleTheme => {
                self.set_appearance(cx.global::<Theme>().toggle(), window, cx)
            }
            // Window visibility and exit have platform/confirmation handling in TrayRuntime.
            TrayAction::ToggleWindow | TrayAction::Quit => {}
        }
    }

    pub(crate) fn has_running_commands(&self) -> bool {
        !self.run_handles.is_empty()
    }

    fn persist_tab_session(&mut self, cx: &mut Context<Self>) {
        let Some(store) = &self.store else {
            return;
        };
        let indices = self
            .open_tab_ids
            .iter()
            .filter_map(|id| self.saved_slots.get(id).copied())
            .collect::<Vec<_>>();
        let active = self
            .saved_slots
            .get(&self.tabs[self.active].id)
            .copied()
            .or_else(|| indices.first().copied())
            .unwrap_or(0);
        if let Err(error) = store.save_tab_session(self.saved_digest, &indices, active) {
            self.status = error;
            cx.notify();
        }
    }

    fn open_position(&self) -> Option<usize> {
        self.open_tab_ids
            .iter()
            .position(|id| *id == self.tabs[self.active].id)
    }

    fn configuration(&self, cx: &App) -> serde_json::Value {
        let tabs = self
            .tabs
            .iter()
            .map(|tab| {
                let mut value = serde_json::to_value(tab.data(cx)).expect("tab data serializes");
                if let Some(rows) = tab.row_height_override {
                    value["gpui_row_height"] = serde_json::json!(rows);
                }
                value
            })
            .collect::<Vec<_>>();
        let accent = cx.global::<Accent>().key();
        serde_json::json!({
            "tabs": tabs,
            "current_tab_index": self.active,
            "theme": if *cx.global::<Theme>() == Theme::Dark { "dark" } else { "light" },
            "gpui": {
                "accent": accent,
                "font_size": self.font_size,
                "font_weight": self.font_weight,
                "language": cx.global::<Language>().code(),
                "reduced_motion": self.reduced_motion,
                "enabled_first": self.enabled_first,
                "show_log": self.show_log,
                "sidebar_collapsed": self.sidebar_collapsed,
                "sidebar_width": self.sidebar_expanded_width,
                "sidebar_ungrouped_collapsed": self.sidebar_ungrouped_collapsed,
                "sidebar_groups": serde_json::to_value(&self.sidebar_groups).expect("groups serialize"),
                "open_tabs": self.open_tab_ids.iter().filter_map(|id| self.tabs.iter().position(|tab| tab.id == *id)).collect::<Vec<_>>(),
                "log_height": self.log_height_override,
                "description_width": self.description_width_override,
            }
        })
    }

    fn save_configuration(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        if !self.config_writable {
            return Err("现有配置读取失败，已禁止覆盖。请先修复文件或恢复备份。".into());
        }
        let store = self
            .store
            .clone()
            .ok_or_else(|| "配置存储尚未初始化。".to_string())?;
        store.save(&self.configuration(cx))?;
        self.saved_tab_data = Self::tab_data_snapshot(&self.tabs, cx);
        self.saved_slots = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| (tab.id, index))
            .collect();
        self.saved_digest = store.config_digest();
        self.persist_tab_session(cx);
        for tab in &mut self.tabs {
            tab.dirty = false;
        }
        self.config_dirty = false;
        self.status = i18n::format(
            cx,
            "配置已保存：{}",
            &[&store.config_path().display().to_string()],
        );
        cx.notify();
        Ok(())
    }

    fn apply_saved_configuration(
        &mut self,
        value: Option<serde_json::Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut replacement = Self::new_with_backend(window, cx, self.store.clone(), value, None);
        replacement.close_to_tray = self.close_to_tray.clone();
        replacement.detached_log = self.detached_log.take();
        replacement.log_window_closed = self.log_window_closed.take();
        replacement.detaching_log = self.detaching_log;
        replacement.log_font_size = self.log_font_size;
        if replacement.detached_log.is_some() {
            replacement.show_log = true;
        }
        *self = replacement;
        cx.notify();
    }

    fn reload_configuration(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let store = self
            .store
            .as_ref()
            .ok_or_else(|| "配置存储尚未初始化。".to_string())?;
        let value = match store.load() {
            Ok(value) => value,
            Err(error) => {
                self.config_writable = false;
                return Err(error);
            }
        };
        self.apply_saved_configuration(value, window, cx);
        self.status = tr(cx, "已重新加载磁盘配置");
        cx.notify();
        Ok(())
    }

    fn request_reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let has_changes = self.has_unsaved_edits_now(cx);
        let has_running = self.has_running_commands();
        if !has_changes && !has_running {
            if let Err(error) = self.reload_configuration(window, cx) {
                self.status = error;
                cx.notify();
            }
            return;
        }
        let weak = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, cx| {
            let reload = weak.clone();
            dialog
                .title(tr(cx, "放弃修改并重新加载？"))
                .child(tr(
                    cx,
                    if has_running {
                        "重新加载会终止正在运行的命令，并放弃未保存更改；磁盘配置不会被修改。"
                    } else {
                        "尚未保存的更改会丢失；磁盘配置不会被修改。"
                    },
                ))
                .footer(
                    row()
                        .justify_end()
                        .py(px(GAP))
                        .child(
                            button("cancel-reload", "取消", None, cx)
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            button("confirm-reload", "放弃并重新加载", None, cx)
                                .primary()
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    let _ = reload.update(cx, |view, cx| {
                                        view.request_reload_after_confirm(window, cx)
                                    });
                                }),
                        ),
                )
        });
    }

    fn request_reload_after_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.reload_configuration(window, cx) {
            Ok(()) => self.status = "已放弃未保存修改并重新加载".into(),
            Err(error) => self.status = error,
        }
        cx.notify();
    }

    pub(crate) fn open_backups(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(store) = self.store.clone() else {
            return;
        };
        let backups = match store.backups() {
            Ok(backups) => backups,
            Err(error) => {
                self.status = error;
                cx.notify();
                return;
            }
        };
        let view = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, window, cx| {
            let import_view = view.clone();
            let choices = column()
                .gap(px(GAP))
                .children(backups.iter().cloned().enumerate().map(|(index, name)| {
                    let restore_view = view.clone();
                    let export_view = view.clone();
                    let backup = name.clone();
                    let export_name = name.clone();
                    row()
                        .child(gpui::div().flex_1().min_w_0().truncate().child(name))
                        .child(button(("backup-choice", index), "恢复", None, cx).on_click(
                            move |_, w, cx| {
                                let _ = restore_view.update(cx, |v, cx| {
                                    v.confirm_restore_backup(backup.clone(), w, cx)
                                });
                            },
                        ))
                        .child(
                            button(("backup-export", index), "导出…", None, cx).on_click(
                                move |_, w, cx| {
                                    let _ = export_view.update(cx, |v, cx| {
                                        v.prompt_export(Some(export_name.clone()), w, cx)
                                    });
                                },
                            ),
                        )
                }));
            dialog
                .title(tr(cx, "备份管理"))
                .w(px(540.))
                .child(
                    frame("backup-list")
                        .max_h((window.viewport_size().height - px(290.)).max(px(120.)))
                        .overflow_y_scroll()
                        .child(choices)
                        .when(backups.is_empty(), |d| {
                            d.child(tr(cx, "还没有配置备份；修改已保存配置后会自动创建备份。"))
                        }),
                )
                .footer(
                    row()
                        .justify_end()
                        .child(button("import-backup", "导入备份…", None, cx).on_click(
                            move |_, w, cx| {
                                w.close_dialog(cx);
                                let _ =
                                    import_view.update(cx, |v, cx| v.prompt_import(true, w, cx));
                            },
                        ))
                        .child(
                            button("backups-close", "完成", None, cx)
                                .on_click(|_, w, cx| w.close_dialog(cx)),
                        ),
                )
        });
    }

    fn confirm_restore_backup(&mut self, backup: String, w: &mut Window, cx: &mut Context<Self>) {
        let owner = cx.entity().downgrade();
        w.open_dialog(cx, move |dialog, _, cx| {
            let owner = owner.clone();
            let backup = backup.clone();
            dialog
                .title(tr(cx, "恢复此配置备份？"))
                .child(backup.clone())
                .child(tr(
                    cx,
                    "恢复前会备份当前配置；未保存修改会丢失，运行中的命令会停止。",
                ))
                .footer(
                    row()
                        .justify_end()
                        .child(
                            button("cancel-restore-backup", "取消", None, cx)
                                .on_click(|_, w, cx| w.close_dialog(cx)),
                        )
                        .child(button("confirm-restore-backup", "恢复", None, cx).on_click(
                            move |_, w, cx| {
                                w.close_all_dialogs(cx);
                                let _ = owner.update(cx, |v, cx| {
                                    v.restore_backup_selection(backup.clone(), w, cx)
                                });
                            },
                        )),
                )
        });
    }

    fn prompt_working_directory(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tab_id = self.current().id;
        let prompt = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(tr(cx, "选择工作目录").into()),
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = prompt.await;
            let _ = view.update_in(cx, |workspace, window, cx| {
                let path = match result {
                    Ok(Ok(Some(paths))) => match paths.into_iter().next() {
                        Some(path) => path,
                        None => return,
                    },
                    Ok(Ok(None)) => return,
                    _ => {
                        workspace.status = tr(cx, "无法打开目录选择器");
                        cx.notify();
                        return;
                    }
                };
                let Some(tab) = workspace.tabs.iter_mut().find(|tab| tab.id == tab_id) else {
                    return;
                };
                tab.directory.update(cx, |state, cx| {
                    state.set_value(path.to_string_lossy().into_owned(), window, cx)
                });
                workspace.changed(tab_id, window, cx);
                workspace.status = tr(cx, "已选择工作目录");
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn prompt_import(
        &mut self,
        backup_only: bool,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let prompt = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(
                tr(
                    cx,
                    if backup_only {
                        "导入备份…"
                    } else {
                        "导入配置…"
                    },
                )
                .into(),
            ),
        });
        cx.spawn_in(w, async move |view, cx| {
            let result = prompt.await;
            let _ = view.update_in(cx, |v, w, cx| {
                let path = match result {
                    Ok(Ok(Some(paths))) => match paths.into_iter().next() {
                        Some(path) => path,
                        None => return,
                    },
                    Ok(Ok(None)) => return,
                    _ => {
                        v.status = tr(cx, "无法打开文件选择器");
                        cx.notify();
                        return;
                    }
                };
                if backup_only {
                    let result = v
                        .store
                        .as_ref()
                        .ok_or_else(|| tr(cx, "配置存储尚未初始化。"))
                        .and_then(|store| store.import_backup(&path));
                    match result {
                        Ok(_) => {
                            v.status = tr(cx, "备份已导入；点击恢复后才会应用。");
                            v.open_backups(w, cx);
                        }
                        Err(e) => v.status = e,
                    }
                    cx.notify();
                    return;
                }
                match backend::read_configuration(&path) {
                    Ok(value) => v.confirm_import(value, w, cx),
                    Err(e) => {
                        v.status = e;
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    fn confirm_import(&mut self, value: serde_json::Value, w: &mut Window, cx: &mut Context<Self>) {
        let owner = cx.entity().downgrade();
        w.open_dialog(cx, move |dialog, _, cx| {
            let value = value.clone();
            let owner = owner.clone();
            dialog
                .title(tr(cx, "导入并替换当前配置？"))
                .child(tr(
                    cx,
                    "恢复前会备份当前配置；未保存修改会丢失，运行中的命令会停止。",
                ))
                .footer(
                    row()
                        .justify_end()
                        .child(
                            button("cancel-import", "取消", None, cx)
                                .on_click(|_, w, cx| w.close_dialog(cx)),
                        )
                        .child(button("confirm-import", "导入", None, cx).on_click(
                            move |_, w, cx| {
                                w.close_dialog(cx);
                                let _ = owner.update(cx, |v, cx| {
                                    let result = v
                                        .store
                                        .as_ref()
                                        .ok_or_else(|| tr(cx, "配置存储尚未初始化。"))
                                        .and_then(|store| store.replace_configuration(&value));
                                    match result {
                                        Ok(()) => {
                                            v.apply_saved_configuration(Some(value.clone()), w, cx);
                                            v.status = tr(cx, "配置已导入");
                                        }
                                        Err(e) => v.status = e,
                                    }
                                    cx.notify();
                                });
                            },
                        )),
                )
        });
    }

    pub(crate) fn prompt_export(
        &mut self,
        backup: Option<String>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(store) = self.store.clone() else {
            self.status = tr(cx, "配置存储尚未初始化。");
            cx.notify();
            return;
        };
        let value = if let Some(name) = &backup {
            match store.read_backup(name) {
                Ok(value) => value,
                Err(e) => {
                    self.status = e;
                    cx.notify();
                    return;
                }
            }
        } else {
            self.configuration(cx)
        };
        let directory = store
            .config_path()
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let prompt = cx.prompt_for_new_path(
            directory,
            Some(backup.as_deref().unwrap_or("easy-command-runner.json")),
        );
        cx.spawn_in(w, async move |view, cx| {
            let result = prompt.await;
            let _ = view.update_in(cx, |v, _, cx| {
                let mut path = match result {
                    Ok(Ok(Some(path))) => path,
                    Ok(Ok(None)) => return,
                    _ => {
                        v.status = tr(cx, "无法打开文件选择器");
                        cx.notify();
                        return;
                    }
                };
                if path.extension().is_none() {
                    path.set_extension("json");
                }
                v.status = match store.export_configuration(&path, &value) {
                    Ok(()) => tr(cx, "配置已导出"),
                    Err(e) => e,
                };
                cx.notify();
            });
        })
        .detach();
    }

    fn restore_backup_selection(
        &mut self,
        backup: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let result = self
            .store
            .as_ref()
            .ok_or_else(|| "配置存储尚未初始化。".to_string())
            .and_then(|store| store.restore_backup(&backup));
        match result {
            Ok(config) => {
                self.apply_saved_configuration(Some(config), window, cx);
                self.status = i18n::format(cx, "已恢复备份：{}", &[&backup]);
            }
            Err(error) => self.status = error,
        }
        cx.notify();
    }

    /// Returns true when exit is immediate; false while a confirmation is pending.
    pub fn request_exit(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.exit_dialog_open.get() {
            return false;
        }
        let has_changes = self.has_unsaved_edits_now(cx);
        let has_running = self.has_running_commands();
        if !has_changes && !has_running {
            // Let the main-window closed callback own GPUI shutdown. This also
            // closes the actual HWND before the Windows message loop quits.
            window.remove_window();
            return true;
        }
        self.exit_dialog_open.set(true);
        let closed = self.exit_dialog_open.clone();
        let can_save = has_changes && self.config_writable && self.store.is_some();
        let view = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, cx| {
            let on_close = closed.clone();
            let on_cancel = closed.clone();
            let on_confirm = closed.clone();
            let save_view = view.clone();
            dialog
                .on_close(move |_, _, _| on_close.set(false))
                .title(tr(
                    cx,
                    if has_changes {
                        "保存修改再退出？"
                    } else {
                        "命令仍在运行"
                    },
                ))
                .child(tr(
                    cx,
                    if has_running && has_changes {
                        "退出将停止正在运行的命令。可保存配置并退出，或放弃修改后退出。"
                    } else if has_running {
                        "退出将终止所有正在运行的命令；取消可让它们继续运行。"
                    } else if can_save {
                        "保存后下次启动会恢复标签、当前页面和工作区布局；也可放弃本次修改。"
                    } else {
                        "配置无法安全写入磁盘；可以取消，或放弃本次会话修改并退出。"
                    },
                ))
                .footer(
                    row()
                        .justify_end()
                        .py(px(GAP))
                        .child(
                            button("cancel-exit", "取消", None, cx).on_click(move |_, w, cx| {
                                on_cancel.set(false);
                                w.close_dialog(cx);
                            }),
                        )
                        .when(can_save, |row| {
                            row.child(
                                button("save-exit", "保存并退出", None, cx)
                                    .primary()
                                    .on_click(move |_, w, cx| {
                                        if matches!(
                                            save_view.update(cx, |v, cx| v.save_configuration(cx)),
                                            Ok(Ok(()))
                                        ) {
                                            w.close_dialog(cx);
                                            w.remove_window();
                                        }
                                    }),
                            )
                        })
                        .child(
                            button(
                                "confirm-exit",
                                if has_changes {
                                    "放弃并退出"
                                } else {
                                    "停止运行并退出"
                                },
                                None,
                                cx,
                            )
                            .on_click(move |_, w, cx| {
                                on_confirm.set(false);
                                w.close_dialog(cx);
                                w.remove_window();
                            }),
                        ),
                )
        });
        false
    }

    fn current(&self) -> &CommandTab {
        &self.tabs[self.active]
    }
    pub fn changed(&mut self, tab_id: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
            tab.dirty = true;
        }
        self.config_dirty = true;
        self.finish_page_switch(); // Never hide fresh text edits behind a frozen animation sheet.
        self.refresh(window, cx);
    }
    fn paste_external_file_path(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(path) = cx.read_from_clipboard().and_then(|clipboard| {
            clipboard.entries.into_iter().find_map(|entry| match entry {
                ClipboardEntry::ExternalPaths(paths) => paths
                    .paths()
                    .first()
                    .map(|path| path.to_string_lossy().into_owned()),
                _ => None,
            })
        }) else {
            return false;
        };

        let focused = self.tabs.iter().find_map(|tab| {
            for input in [
                &tab.name,
                &tab.directory,
                &tab.program,
                &tab.other,
                &tab.append,
            ] {
                if input.focus_handle(cx).is_focused(window) {
                    return Some((tab.id, input.clone()));
                }
            }
            tab.rows
                .iter()
                .flat_map(|row| [&row.option, &row.value, &row.note])
                .find(|input| input.focus_handle(cx).is_focused(window))
                .map(|input| (tab.id, input.clone()))
        });
        let Some((tab_id, input)) = focused else {
            return false;
        };

        // Match Qt PathLineEdit: a file clipboard replaces the whole single-line value.
        input.update(cx, |state, cx| state.set_value(path, window, cx));
        self.changed(tab_id, window, cx);
        true
    }

    fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let command = self.current().command(cx);
        if self.preview.read(cx).value().as_ref() != command {
            self.preview
                .update(cx, |state, cx| state.set_value(command, window, cx));
        }
        cx.notify();
    }
    fn page_snapshot(&self, window: &Window, cx: &App) -> Rc<PageSnapshot> {
        let tab = self.current();
        let layout = WorkspaceLayout::with_overrides(
            f32::from(window.viewport_size().height),
            tab.rows.len(),
            self.show_log,
            tab.row_height_override,
            self.log_height_override,
        );
        Rc::new(PageSnapshot {
            tab_id: tab.id,
            data: tab.data(cx),
            command: tab.command(cx),
            append: tab.append.read(cx).value().to_string(),
            rows_height: layout.rows_height,
            rows_offset: f32::from(self.row_scroll.offset().y),
            description_width: self.description_width_override,
            enabled_first: self.enabled_first,
        })
    }

    fn page_visual(&self, window: &Window, cx: &App) -> Rc<PageVisual> {
        if let (Some((_, direction)), Some(transition)) = (self.page_switch, &self.page_transition)
        {
            let progress = cx
                .background_executor()
                .now()
                .saturating_duration_since(transition.started_at)
                .as_secs_f32()
                / page_transition::DURATION.as_secs_f32();
            return transition.freeze(direction, progress);
        }
        if self.open_position().is_none() {
            Rc::new(PageVisual::Empty)
        } else {
            Rc::new(PageVisual::Page(self.page_snapshot(window, cx)))
        }
    }

    fn finish_page_switch(&mut self) {
        self.page_switch = None;
        self.page_transition = None;
    }

    fn start_page_switch_from(&mut self, direction: i8, outgoing: Rc<PageVisual>, cx: &App) {
        if self.reduced_motion || cx.reduce_motion() {
            self.finish_page_switch();
        } else {
            self.page_switch_epoch = self.page_switch_epoch.wrapping_add(1);
            self.page_switch = Some((self.page_switch_epoch, direction));
            self.page_transition = Some(PageTransition {
                started_at: cx.background_executor().now(),
                outgoing,
                incoming: None,
            });
        }
    }

    fn start_page_switch(&mut self, direction: i8, window: &Window, cx: &App) {
        if self.reduced_motion || cx.reduce_motion() {
            self.finish_page_switch();
            return;
        }
        let outgoing = self.page_visual(window, cx);
        self.start_page_switch_from(direction, outgoing, cx);
    }

    fn page_slide_layers(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<(Rc<PageVisual>, Rc<PageSnapshot>, f32, i8)> {
        if self.reduced_motion || cx.reduce_motion() || self.open_tab_ids.is_empty() {
            self.finish_page_switch();
            return None;
        }
        let (_, direction) = self.page_switch?;
        let progress = cx
            .background_executor()
            .now()
            .saturating_duration_since(self.page_transition.as_ref()?.started_at)
            .as_secs_f32()
            / page_transition::DURATION.as_secs_f32();
        if progress >= 1. {
            self.finish_page_switch();
            return None;
        }
        if self.page_transition.as_ref()?.incoming.is_none() {
            let incoming = self.page_snapshot(window, cx);
            self.page_transition.as_mut()?.incoming = Some(incoming);
        }
        let transition = self.page_transition.as_ref()?;
        let layers = (
            transition.outgoing.clone(),
            transition.incoming.as_ref()?.clone(),
            progress,
            direction,
        );
        cx.on_next_frame(window, |view, _, cx| {
            if view.page_switch.is_some() {
                cx.notify();
            }
        });
        Some(layers)
    }

    pub fn select_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        if self.active == index && self.open_position().is_some() {
            // A top tab can reveal its configuration from a collapsed group without replaying the page transition.
            self.reveal_sidebar_tab(index, cx);
            self.scroll_sidebar_to_tab(index);
            self.refresh(window, cx);
            return;
        }
        let previous = self.open_position();
        let was_open = self.open_tab_ids.contains(&self.tabs[index].id);
        if !was_open {
            self.open_tab_ids.push(self.tabs[index].id);
        }
        let next = self
            .open_tab_ids
            .iter()
            .position(|id| *id == self.tabs[index].id)
            .unwrap();
        self.start_page_switch(
            if next >= previous.unwrap_or(next) {
                1
            } else {
                -1
            },
            window,
            cx,
        );
        self.active = index;
        self.resizing = None; // 切页时终止拖动，不能把上一页的拖动继续写入新页。
        self.tab_menu = None;
        self.tab_group_options_open = false;
        self.drag_target = None;
        self.stop_row_drag_auto_scroll();
        self.tab_drag_target = None;
        self.tab_drop_x = None;
        self.row_scroll = ScrollHandle::new();
        self.tab_scroll.scroll_to_item(next);
        self.reveal_sidebar_tab(index, cx);
        self.scroll_sidebar_to_tab(index);
        self.persist_tab_session(cx);
        self.refresh(window, cx);
    }
    fn run_current_command(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let command = self.current().command(cx);
        let tab_name = self.current().label(cx);
        let working_dir = self.current().directory.read(cx).value().to_string();
        if command.trim().is_empty() {
            self.status = tr(cx, "请输入程序或命令后再运行。");
            cx.notify();
            return;
        }
        const MAX_RUN_HISTORY: usize = 100;
        if self.logs.len() >= MAX_RUN_HISTORY && !self.logs.iter().any(|log| log.finished) {
            self.status = tr(cx, "运行历史已满且所有命令仍在运行；停止部分命令后再试。");
            cx.notify();
            return;
        }
        match backend::start_shell(&command, &working_dir) {
            Err(error) => {
                self.status = error;
                cx.notify();
            }
            Ok((handle, receiver)) => {
                let id = self.next_run_id;
                self.next_run_id += 1;
                if self.logs.len() >= MAX_RUN_HISTORY {
                    if let Some(index) = self.logs.iter().position(|log| log.finished) {
                        self.logs.remove(index);
                    }
                }
                self.logs.push(SessionLog {
                    id,
                    command: Some(command),
                    command_name: tab_name.clone(),
                    caption: i18n::format(cx, "{} · 运行 #{}", &[&tab_name, &(id + 1).to_string()]),
                    output: String::new(),
                    finished: false,
                    exit_code: None,
                    stopped: false,
                });
                self.selected_log_id = Some(id);
                self.log_auto_scroll.request_tail();
                self.run_handles.insert(id, handle);
                self.show_log = true;
                self.config_dirty = true;
                self.sync_log_list(window, cx);
                self.status = i18n::format(cx, "正在运行：{}", &[&tab_name]);
                cx.notify();

                let task = cx.spawn_in(window, async move |view, window| loop {
                    let mut events = Vec::new();
                    let mut disconnected = false;
                    while events.len() < 128 {
                        match receiver.try_recv() {
                            Ok(event) => {
                                let finished = matches!(event, RunEvent::Finished { .. });
                                events.push(event);
                                if finished {
                                    break;
                                }
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => break,
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                disconnected = true;
                                break;
                            }
                        }
                    }
                    let finished = events
                        .iter()
                        .any(|event| matches!(event, RunEvent::Finished { .. }));
                    if !events.is_empty() {
                        let _ = view.update_in(window, |view, window, cx| {
                            view.receive_run_events(id, events, window, cx);
                        });
                    }
                    if finished || disconnected {
                        break;
                    }
                    window
                        .background_executor()
                        .timer(std::time::Duration::from_millis(24))
                        .await;
                });
                task.detach();
            }
        }
    }

    fn receive_run_events(
        &mut self,
        id: usize,
        events: Vec<RunEvent>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut completed = false;
        let mut output_changed = false;
        for event in events {
            match event {
                RunEvent::Output { stream, text } => {
                    if let Some(log) = self.logs.iter_mut().find(|log| log.id == id) {
                        if stream == OutputStream::Stderr {
                            log.output.push_str("[stderr] ");
                        }
                        log.output.push_str(&text);
                        const MAX_LOG_BYTES: usize = 1_000_000;
                        if log.output.len() > MAX_LOG_BYTES {
                            let mut start = log.output.len() - MAX_LOG_BYTES;
                            while !log.output.is_char_boundary(start) {
                                start += 1;
                            }
                            log.output.drain(..start);
                            log.output
                                .insert_str(0, &tr(cx, "[前序输出已截断，保留最近 1 MB]\n"));
                        }
                        output_changed = true;
                    }
                }
                RunEvent::Finished { exit_code, stopped } => {
                    let Some(log) = self.logs.iter_mut().find(|log| log.id == id) else {
                        continue;
                    };
                    if log.finished {
                        continue; // A repeated completion must not duplicate the caption or notification.
                    }
                    log.finished = true;
                    log.exit_code = Some(exit_code);
                    log.stopped = stopped;
                    let result = if stopped {
                        tr(cx, "已停止")
                    } else {
                        i18n::format(cx, "退出 {}", &[&exit_code.to_string()])
                    };
                    log.caption = format!("{} · {result}", log.caption);
                    if let Some(notification) =
                        notifications::run_completion(id, &log.command_name, exit_code, stopped, cx)
                    {
                        cx.show_system_notification(notification);
                    }
                    self.run_handles.remove(&id);
                    log.command = None;
                    self.status = if stopped {
                        tr(cx, "命令已停止")
                    } else {
                        i18n::format(cx, "命令结束，退出代码：{}", &[&exit_code.to_string()])
                    };
                    completed = true;
                }
            }
        }
        if completed {
            self.sync_log_list(window, cx);
        } else if output_changed && self.selected_log_id == Some(id) {
            if let Some(log) = self.logs.iter().find(|log| log.id == id) {
                self.log_auto_scroll
                    .set_value(&self.log_output, log.output.clone(), window, cx);
            }
        }
        cx.notify();
    }

    pub(crate) fn selected_running_command(&self) -> Option<&str> {
        let id = self.selected_log_id?;
        self.run_handles.contains_key(&id).then_some(())?;
        self.logs
            .iter()
            .find(|log| log.id == id)?
            .command
            .as_deref()
    }

    pub(crate) fn copy_selected_run_command(&mut self, cx: &mut Context<Self>) {
        let Some(command) = self.selected_running_command() else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(command.to_owned()));
        self.status = tr(cx, "当前运行命令已复制到剪贴板");
        cx.notify();
    }

    pub(crate) fn stop_selected_run(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected_log_id else {
            return;
        };
        if let Some(handle) = self.run_handles.get(&id) {
            if let Err(error) = handle.stop() {
                self.status = error;
            } else {
                self.status = tr(cx, "正在停止命令…");
            }
            cx.notify();
        }
    }

    pub(crate) fn select_log(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(log) = self.logs.iter().find(|log| log.id == id) else {
            return;
        };
        self.selected_log_id = Some(id);
        let output = log.output.clone();
        self.log_auto_scroll.request_tail();
        self.log_auto_scroll
            .set_value(&self.log_output, output, window, cx);
        cx.notify();
    }

    fn sync_log_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let items = self
            .logs
            .iter()
            .map(|log| LogItem {
                id: log.id,
                label: log.caption.clone().into(),
            })
            .collect();
        let selected = self
            .selected_log_id
            .and_then(|id| self.logs.iter().position(|log| log.id == id))
            .map(IndexPath::new);
        self.log_selector.update(cx, |state, cx| {
            state.set_items(LogItems(items), window, cx);
            state.set_selected_index(selected, window, cx);
        });
        let output = self
            .selected_log_id
            .and_then(|id| self.logs.iter().find(|log| log.id == id))
            .map(|log| log.output.clone())
            .unwrap_or_default();
        self.log_auto_scroll
            .set_value(&self.log_output, output, window, cx);
        cx.notify();
    }

    pub(crate) fn delete_selected_log(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self
            .selected_log_id
            .and_then(|id| self.logs.iter().position(|log| log.id == id))
        else {
            return;
        };
        if !self.logs[index].finished {
            return;
        }
        self.logs.remove(index);
        self.selected_log_id = self
            .logs
            .get(index.min(self.logs.len().saturating_sub(1)))
            .map(|log| log.id);
        self.log_auto_scroll.request_tail();
        self.sync_log_list(window, cx);
    }

    pub(crate) fn log_snapshot(&self) -> LogSnapshot {
        let selected = self
            .selected_log_id
            .and_then(|id| self.logs.iter().find(|log| log.id == id));
        LogSnapshot {
            items: self
                .logs
                .iter()
                .map(|log| LogItem {
                    id: log.id,
                    label: log.caption.clone().into(),
                })
                .collect(),
            selected_id: selected.map(|log| log.id),
            output: selected.map_or_else(String::new, |log| log.output.clone()),
            can_delete: selected.is_some_and(|log| log.finished),
            can_stop: self
                .selected_log_id
                .is_some_and(|id| self.run_handles.contains_key(&id)),
            selected_command: self.selected_running_command().map(str::to_owned),
            font_size: self.log_font_size,
        }
    }

    pub(crate) fn zoom_log(&mut self, delta: i16, cx: &mut Context<Self>) {
        self.log_font_size = (self.log_font_size as i16 + delta).clamp(8, 32) as u8;
        cx.notify();
    }

    fn toggle_log(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.detached_log {
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, _| window.activate_window());
            });
        } else {
            self.show_log = !self.show_log;
            if self.show_log {
                self.log_auto_scroll.reopen();
            }
            self.config_dirty = true;
            cx.notify();
        }
    }

    pub(crate) fn reattach_log(&mut self, cx: &mut Context<Self>) {
        self.detached_log = None;
        self.show_log = true;
        self.log_auto_scroll.reopen();
        cx.notify();
    }

    fn detach_log(&mut self, cx: &mut Context<Self>) {
        if self.detaching_log {
            return;
        }
        if self.detached_log.is_some() {
            self.toggle_log(cx);
            return;
        }
        self.detaching_log = true;
        cx.notify();
        let owner = cx.entity().downgrade();
        // Open after the current workspace update: the new window's first frame reads this
        // workspace, and reading it while it is mutably borrowed would panic.
        cx.defer(move |cx| {
            let Some(entity) = owner.upgrade() else {
                return;
            };
            let title = tr(cx, "运行日志 - EasyCommandRunner");
            let result = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(gpui::size(px(850.), px(360.)), cx)),
                    window_min_size: Some(gpui::size(px(480.), px(240.))),
                    titlebar: Some(gpui::TitlebarOptions {
                        title: Some(title.into()),
                        ..Default::default()
                    }),
                    window_decorations: Some(WindowDecorations::Server),
                    ..Default::default()
                },
                move |window, cx| {
                    let panel = cx.new(|cx| DetachedLogWindow::new(entity.clone(), window, cx));
                    cx.new(|cx| Root::new(panel, window, cx))
                },
            );
            match result {
                Ok(handle) => {
                    let id = handle.window_id();
                    let closed_owner = owner.clone();
                    let listener = cx.on_window_closed(move |cx, closed| {
                        if closed != id {
                            return;
                        }
                        let _ = closed_owner.update(cx, |view, cx| {
                            if view
                                .detached_log
                                .is_some_and(|window| window.window_id() == id)
                            {
                                view.reattach_log(cx);
                            }
                        });
                    });
                    let _ = owner.update(cx, |view, cx| {
                        view.detached_log = Some(handle);
                        view.log_window_closed = Some(listener);
                        view.detaching_log = false;
                        view.show_log = true;
                        cx.notify();
                    });
                }
                Err(error) => {
                    let _ = owner.update(cx, |view, cx| {
                        view.detaching_log = false;
                        view.status = format!("无法分离运行日志：{error}");
                        cx.notify();
                    });
                }
            }
        });
    }

    #[cfg(any(test, feature = "ui-test"))]
    pub(crate) fn add_finished_log_for_test(
        &mut self,
        caption: &str,
        output: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> usize {
        let id = self.next_run_id;
        self.next_run_id += 1;
        self.logs.push(SessionLog {
            id,
            command: None,
            command_name: caption.into(),
            caption: caption.into(),
            output: output.into(),
            finished: true,
            exit_code: Some(0),
            stopped: false,
        });
        self.selected_log_id = Some(id);
        self.sync_log_list(window, cx);
        id
    }

    fn new_tab(&mut self, copy: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.new_tab_from(if copy { Some(self.active) } else { None }, window, cx);
    }
    fn new_tab_from(&mut self, source: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        let data = if let Some(source) = source {
            let mut data = self.tabs[source].data(cx);
            data.name = i18n::format(cx, "{} 副本", &[&data.name]);
            data
        } else {
            TabData {
                name: i18n::format(cx, "标签{}", &[&(self.next_tab_id + 1).to_string()]),
                rows: vec![RowData::default()],
                ..Default::default()
            }
        };
        let mut tab = CommandTab::new(self.next_tab_id, data, window, cx);
        // 复制页继承当时的布局，此后独立记忆；空白新页使用默认高度。
        tab.row_height_override = source.and_then(|index| self.tabs[index].row_height_override);
        tab.dirty = true;
        self.next_tab_id += 1;
        self.tabs.push(tab);
        self.select_tab(self.tabs.len() - 1, window, cx);
    }
    /// Closing an editor is never deletion: unsaved input entities remain in the sidebar.
    fn request_close(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.tabs.get(index).map(|tab| tab.id) else {
            return;
        };
        let Some(position) = self.open_tab_ids.iter().position(|open| *open == id) else {
            return;
        };
        let outgoing = (self.active == index).then(|| self.page_visual(window, cx));
        self.open_tab_ids.remove(position);
        self.tab_menu = None;
        self.tab_group_options_open = false;
        if self.active == index {
            if let Some(next_id) = self
                .open_tab_ids
                .get(position.min(self.open_tab_ids.len().saturating_sub(1)))
                .copied()
            {
                self.start_page_switch_from(
                    if position < self.open_tab_ids.len() {
                        1
                    } else {
                        -1
                    },
                    outgoing.expect("the active editor has an outgoing visual"),
                    cx,
                );
                self.active = self.tabs.iter().position(|tab| tab.id == next_id).unwrap();
                self.row_scroll = ScrollHandle::new();
                self.reveal_sidebar_tab(self.active, cx);
                self.scroll_sidebar_to_tab(self.active);
                self.refresh(window, cx);
            } else {
                self.finish_page_switch();
                cx.notify();
            }
        } else {
            cx.notify();
        }
        self.persist_tab_session(cx);
    }

    /// Sidebar X removes a stored configuration, not merely its editor. Always confirm.
    fn request_delete(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(index) else {
            return;
        };
        if self.tabs.len() == 1 {
            return;
        }
        let id = tab.id;
        let title = tab.label(cx);
        let view = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, cx| {
            let view = view.clone();
            dialog
                .title(tr(cx, "从配置列表删除？"))
                .child(i18n::format(
                    cx,
                    "删除「{}」会移除配置和未保存修改；保存配置后才会从磁盘删除。",
                    &[&title],
                ))
                .footer(
                    row()
                        .justify_end()
                        .py(px(GAP))
                        .child(
                            button("cancel-delete-tab", "取消", None, cx)
                                .on_click(|_, w, cx| w.close_dialog(cx)),
                        )
                        .child(button("confirm-delete-tab", "删除配置", None, cx).on_click(
                            move |_, w, cx| {
                                w.close_dialog(cx);
                                let _ = view.update(cx, |view, cx| {
                                    if let Some(index) =
                                        view.tabs.iter().position(|tab| tab.id == id)
                                    {
                                        view.close_tab(index, w, cx);
                                    }
                                });
                            },
                        )),
                )
        });
    }
    fn close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let previous_id = self.current().id;
        let outgoing = self.page_visual(window, cx);
        self.config_dirty = true;
        self.resizing = None;
        self.tab_menu = None;
        self.tab_group_options_open = false;
        let removed_id = self.tabs[index].id;
        self.open_tab_ids.retain(|id| *id != removed_id);
        self.saved_slots.remove(&removed_id);
        self.tabs.remove(index);
        if self.tabs.is_empty() {
            self.tabs.push(CommandTab::new(
                self.next_tab_id,
                TabData {
                    name: tr(cx, "未命名"),
                    ..Default::default()
                },
                window,
                cx,
            ));
            self.open_tab_ids.push(self.next_tab_id);
            self.next_tab_id += 1;
        }
        self.active = if index < self.active {
            self.active - 1
        } else {
            self.active.min(self.tabs.len() - 1)
        };
        if self.current().id != previous_id {
            // 关闭背景标签时当前页面内容没有变化，无须播放。
            self.start_page_switch_from(if index < self.tabs.len() { 1 } else { -1 }, outgoing, cx);
            self.row_scroll = ScrollHandle::new();
        }
        if !self.open_tab_ids.contains(&self.current().id) {
            if let Some(id) = self.open_tab_ids.first().copied() {
                self.active = self.tabs.iter().position(|tab| tab.id == id).unwrap();
            }
        }
        self.reveal_sidebar_tab(self.active, cx);
        self.scroll_sidebar_to_tab(self.active);
        self.persist_tab_session(cx);
        if self.open_tab_ids.is_empty() {
            cx.notify();
        } else {
            self.refresh(window, cx);
        }
    }
    /// In the middle of a target, follow the drag direction: moving item 2 onto
    /// item 1 means "before", not "after" (which would leave adjacent items unchanged).
    /// Near either edge users can still explicitly pick a before/after insertion slot.
    fn tab_drop_after(&self, drag: &TabDrag, target: usize, pointer: f32, center: f32) -> bool {
        let (source, destination) = match drag.area {
            TabArea::Sidebar => (
                self.tabs.iter().position(|tab| tab.id == drag.tab_id),
                self.tabs.iter().position(|tab| tab.id == target),
            ),
            TabArea::Top => (
                self.open_tab_ids.iter().position(|id| *id == drag.tab_id),
                self.open_tab_ids.iter().position(|id| *id == target),
            ),
        };
        if (pointer - center).abs() <= 6. {
            source.zip(destination).is_some_and(|(from, to)| from < to)
        } else {
            pointer > center
        }
    }

    /// The sidebar sorts configurations; the title bar sorts only open editors.
    /// Runtime IDs keep focus, drafts, and the active editor attached to the same config.
    fn reorder_tabs(&mut self, drag: &TabDrag, target: usize, after: bool, cx: &mut Context<Self>) {
        self.tab_drag_target = None;
        self.tab_drop_x = None;
        self.tab_drag_centers.clear();
        if drag.tab_id == target {
            cx.notify(); // Remove the insertion marker even for a self-drop.
            return;
        }
        let active_id = self.tabs[self.active].id;
        let ids = match drag.area {
            TabArea::Sidebar => self.tabs.iter().map(|tab| tab.id).collect::<Vec<_>>(),
            TabArea::Top => self.open_tab_ids.clone(),
        };
        let (Some(source), Some(destination)) = (
            ids.iter().position(|id| *id == drag.tab_id),
            ids.iter().position(|id| *id == target),
        ) else {
            return;
        };
        if drag.area == TabArea::Sidebar
            && self.tabs[source].sidebar_group_id != self.tabs[destination].sidebar_group_id
        {
            self.tab_drag_target = None;
            self.tab_drag_centers.clear();
            cx.notify();
            return;
        }
        let mut insert = destination + usize::from(after);
        if source < insert {
            insert -= 1;
        }
        if insert == source {
            return;
        }
        match drag.area {
            TabArea::Sidebar => {
                let tab = self.tabs.remove(source);
                self.tabs.insert(insert, tab);
                self.active = self
                    .tabs
                    .iter()
                    .position(|tab| tab.id == active_id)
                    .unwrap();
                self.config_dirty = true;
                self.scroll_sidebar_to_tab(insert);
            }
            TabArea::Top => {
                let id = self.open_tab_ids.remove(source);
                self.open_tab_ids.insert(insert, id);
                self.tab_scroll.scroll_to_item(insert);
            }
        }
        self.persist_tab_session(cx);
        cx.notify();
    }

    fn add_row(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tab = self.current();
        let id = tab.rows.iter().map(|r| r.id).max().map_or(0, |id| id + 1);
        // Blank options stay disabled until the user explicitly includes them in the command.
        let row = Parameter::new(id, tab.id, RowData::default(), window, cx);
        let focus = row.option.focus_handle(cx);
        self.tabs[self.active].rows.push(row);
        self.row_scroll
            .scroll_to_item(self.current().rows.len() - 1);
        self.changed(self.current().id, window, cx);
        window.focus(&focus, cx);
    }
    fn stop_row_drag_auto_scroll(&mut self) {
        if self.row_drag_auto_scroll.take().is_some() || self.row_drag_auto_scroll_running {
            self.row_drag_auto_scroll_running = false;
            self.row_drag_auto_scroll_generation =
                self.row_drag_auto_scroll_generation.wrapping_add(1);
        }
    }

    fn advance_row_drag_auto_scroll(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(state) = self.row_drag_auto_scroll else {
            self.row_drag_auto_scroll_running = false;
            return false;
        };
        if self.current().id != state.drag.tab_id
            || !self
                .current()
                .rows
                .iter()
                .any(|row| row.id == state.drag.row_id)
        {
            self.stop_row_drag_auto_scroll();
            self.drag_target = None;
            cx.notify();
            return false;
        }
        let offset = self.row_scroll.offset();
        // max_offset is a positive extent; actual positions lie in [-max, 0].
        let limit = self.row_scroll.max_offset().y;
        let next_y = (offset.y + px(state.direction * 12.)).clamp(-limit, px(0.));
        if next_y != offset.y {
            self.row_scroll.set_offset(gpui::point(offset.x, next_y));
        }
        let bounds = self.row_scroll.bounds();
        let y = f32::from(state.pointer.y - bounds.top() - next_y);
        let next = row_drop_target(
            &self.current().rows,
            y,
            state.drag.row_id,
            self.enabled_first,
        );
        if self.drag_target != next {
            self.drag_target = next;
            cx.notify();
        }
        if next_y == offset.y {
            self.row_drag_auto_scroll_running = false;
            return false;
        }
        window.refresh();
        true
    }

    fn reorder_row(
        &mut self,
        drag: &RowDrag,
        target_id: usize,
        after: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.current().id != drag.tab_id || drag.row_id == target_id {
            self.drag_target = None;
            cx.notify();
            return;
        }
        let rows = &mut self.tabs[self.active].rows;
        let Some(source) = rows.iter().position(|row| row.id == drag.row_id) else {
            return;
        };
        let row = rows.remove(source);
        let Some(target) = rows.iter().position(|row| row.id == target_id) else {
            rows.insert(source, row);
            return;
        };
        let destination = target + usize::from(after);
        rows.insert(destination, row);
        self.drag_target = None;
        self.changed(drag.tab_id, window, cx);
    }
    fn set_all(&mut self, enabled: bool, window: &mut Window, cx: &mut Context<Self>) {
        for row in &mut self.tabs[self.active].rows {
            row.enabled = enabled;
        }
        self.changed(self.current().id, window, cx);
    }
    fn toggle_row(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(row) = self.tabs[self.active].rows.iter_mut().find(|r| r.id == id) {
            row.enabled = !row.enabled;
            if self.enabled_first {
                self.tabs[self.active].rows.sort_by_key(|r| !r.enabled);
            }
            self.changed(self.current().id, window, cx);
        }
    }
    fn toggle_enabled_first(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.enabled_first = !self.enabled_first;
        self.config_dirty = true;
        if self.enabled_first {
            for tab in &mut self.tabs {
                tab.rows.sort_by_key(|r| !r.enabled);
            }
        }
        self.drag_target = None;
        self.refresh(window, cx);
    }
    fn sort_parameters_by_option(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let before = self
            .current()
            .rows
            .iter()
            .map(|row| row.id)
            .collect::<Vec<_>>();
        let enabled_first = self.enabled_first;
        // Explicit click only: reordering options changes the actual command argument order.
        // A stable sort preserves the order of equally named flags and their input entities.
        self.tabs[self.active].rows.sort_by_cached_key(|row| {
            let option = row.option.read(cx).value();
            (
                enabled_first && !row.enabled,
                option.trim().is_empty(),
                option.trim().to_lowercase(),
            )
        });
        self.drag_target = None;
        if before
            != self
                .current()
                .rows
                .iter()
                .map(|row| row.id)
                .collect::<Vec<_>>()
        {
            self.changed(self.current().id, window, cx);
        } else {
            cx.notify();
        }
    }
    fn parse(&mut self, append: bool, window: &mut Window, cx: &mut Context<Self>) {
        let source = if append {
            self.current().append.read(cx).value()
        } else {
            self.current().program.read(cx).value()
        };
        if source.trim().is_empty() || CommandParser::tokenize(&source).is_err() {
            self.status = tr(cx, "无法解析：请输入命令并检查引号是否完整。");
            cx.notify();
            return;
        }
        if !append && !self.current().rows.is_empty() {
            let view = cx.entity().downgrade();
            let id = self.current().id;
            window.open_dialog(cx, move |dialog, _, cx| {
                let view = view.clone();
                let source = source.clone();
                dialog
                    .title(tr(cx, "替换现有参数？"))
                    .child(tr(cx, "解析将替换当前参数和行备注，不会执行命令。"))
                    .footer(
                        row()
                            .justify_end()
                            .py(px(GAP))
                            .child(
                                button("cancel-replace", "取消", None, cx)
                                    .on_click(|_, w, cx| w.close_dialog(cx)),
                            )
                            .child(
                                button("confirm-replace", "解析并替换", None, cx)
                                    .primary()
                                    .on_click(move |_, w, cx| {
                                        w.close_dialog(cx);
                                        let _ = view.update(cx, |v, cx| {
                                            if v.current().id == id {
                                                v.apply_parse(&source, false, w, cx);
                                            }
                                        });
                                    }),
                            ),
                    )
            });
        } else {
            self.apply_parse(&source, append, window, cx);
        }
    }
    fn apply_parse(
        &mut self,
        source: &str,
        append: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let tokens = CommandParser::parse(source, append);
        if tokens.is_empty() {
            return;
        }
        let tab_id = self.current().id;
        if !append {
            self.tabs[self.active].rows.clear();
            self.current()
                .program
                .update(cx, |s, cx| s.set_value(tokens[0].clone(), window, cx));
            self.current()
                .other
                .update(cx, |s, cx| s.set_value("", window, cx));
        }
        for pair in tokens[if append { 0 } else { 1 }..].chunks(2) {
            let id = self
                .current()
                .rows
                .iter()
                .map(|r| r.id)
                .max()
                .map_or(0, |id| id + 1);
            let row = Parameter::new(
                id,
                tab_id,
                RowData {
                    enabled: true,
                    option: pair[0].clone(),
                    value: pair.get(1).cloned().unwrap_or_default(),
                    note: String::new(),
                },
                window,
                cx,
            );
            self.tabs[self.active].rows.push(row);
        }
        if append {
            self.current()
                .append
                .update(cx, |s, cx| s.set_value("", window, cx));
        }
        self.changed(tab_id, window, cx);
    }

    fn set_appearance(&mut self, mode: Theme, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_page_switch();
        theme::apply(mode, cx);
        self.config_dirty = true;
        theme::set_font_size(self.font_size, cx);
        theme::set_font_weight(self.font_weight, cx);
        theme::set_reduced_motion(self.reduced_motion, cx);
        window.refresh();
        cx.notify();
    }

    pub(crate) fn copy_configuration(&mut self, cx: &mut Context<Self>) {
        let config = self.configuration(cx);
        cx.write_to_clipboard(ClipboardItem::new_string(
            serde_json::to_string_pretty(&config).expect("JSON serialization"),
        ));
        self.status = tr(cx, "已复制兼容 Qt 格式的配置 JSON；尚未写入磁盘");
        cx.notify();
    }

    pub(crate) fn apply_preferences(
        &mut self,
        p: Preferences,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let mut config = self.configuration(cx);
        config["theme"] = serde_json::json!(if p.theme == Theme::Dark {
            "dark"
        } else {
            "light"
        });
        config["gpui"]["accent"] = serde_json::json!(p.accent.key());
        config["gpui"]["font_size"] = p.font_size.into();
        config["gpui"]["font_weight"] = p.font_weight.into();
        config["gpui"]["language"] = p.language.code().into();
        config["gpui"]["reduced_motion"] = p.reduced_motion.into();
        if let Some(store) = &self.store {
            store.save_preferences(&config)?;
            self.saved_digest = store.config_digest();
            self.persist_tab_session(cx);
        }
        self.finish_page_switch();
        self.font_size = p.font_size;
        self.font_weight = p.font_weight;
        self.reduced_motion = p.reduced_motion;
        i18n::apply(p.language, cx);
        cx.set_global(p.accent);
        theme::apply(p.theme, cx);
        theme::set_font_size(p.font_size, cx);
        theme::set_font_weight(p.font_weight, cx);
        theme::set_reduced_motion(p.reduced_motion, cx);
        for tab in &self.tabs {
            tab.translate_placeholders(w, cx);
        }
        self.preview.update(cx, |s, cx| {
            s.set_placeholder(tr(cx, "命令预览 · 修改参数后实时更新"), w, cx)
        });
        self.log_output.update(cx, |s, cx| {
            s.set_placeholder(tr(cx, "暂无运行记录。"), w, cx)
        });
        self.status = tr(cx, "设置已应用");
        self.refresh(w, cx);
        w.refresh();
        cx.notify();
        Ok(())
    }

    fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let p = Preferences {
            theme: *cx.global::<Theme>(),
            accent: *cx.global::<Accent>(),
            font_size: self.font_size,
            font_weight: self.font_weight,
            language: *cx.global::<Language>(),
            reduced_motion: self.reduced_motion,
        };
        let owner = cx.entity().downgrade();
        let panel = cx.new(|cx| SettingsPanel::new(owner, p, window, cx));
        self.settings_panel = Some(panel.clone());
        window.open_dialog(cx, move |dialog, _, cx| {
            dialog
                .title(tr(cx, "工作区设置"))
                .w(px(540.))
                .child(panel.clone())
        });
    }

    fn menu_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let weak = cx.entity().downgrade();
        let can_save = self.config_writable && self.store.is_some();
        let has_store = self.store.is_some();
        let has_open = !self.open_tab_ids.is_empty();
        let detached = self.detached_log.is_some();
        icon_button("application-menu", "主菜单", IconName::Ellipsis, cx).dropdown_menu(
            move |mut menu, _, _| {
                let view = weak.clone();
                let new_tab = menu_item("新建标签")
                    .icon(Icon::new(IconName::Plus))
                    .on_click(move |_, window, cx| {
                        let _ = view.update(cx, |view, cx| view.new_tab(false, window, cx));
                    });
                let view = weak.clone();
                let copy_tab = menu_item("复制当前标签")
                    .icon(Icon::new(IconName::Copy))
                    .disabled(!has_open)
                    .on_click(move |_, window, cx| {
                        let _ = view.update(cx, |view, cx| view.new_tab(true, window, cx));
                    });
                let view = weak.clone();
                let all = menu_item("启用全部参数").on_click(move |_, window, cx| {
                    let _ = view.update(cx, |view, cx| view.set_all(true, window, cx));
                });
                let view = weak.clone();
                let none = menu_item("禁用全部参数").on_click(move |_, window, cx| {
                    let _ = view.update(cx, |view, cx| view.set_all(false, window, cx));
                });
                let view = weak.clone();
                let logs = menu_item(if detached {
                    "显示独立日志窗口"
                } else {
                    "切换运行日志"
                })
                .icon(Icon::new(IconName::Terminal))
                .on_click(move |_, _, cx| {
                    let _ = view.update(cx, |view, cx| view.toggle_log(cx));
                });
                let view = weak.clone();
                let settings = menu_item("工作区设置")
                    .icon(Icon::new(IconName::Settings2))
                    .on_click(move |_, window, cx| {
                        let _ = view.update(cx, |v, cx| v.open_settings(window, cx));
                    });
                let view = weak.clone();
                let export = menu_item("复制 Qt 兼容配置 JSON")
                    .icon(Icon::new(IconName::Copy))
                    .on_click(move |_, _, cx| {
                        let _ = view.update(cx, |v, cx| v.copy_configuration(cx));
                    });
                let view = weak.clone();
                let save = menu_item("保存配置")
                    .disabled(!can_save)
                    .on_click(move |_, _, cx| {
                        let _ = view.update(cx, |v, cx| {
                            if let Err(error) = v.save_configuration(cx) {
                                v.status = error;
                                cx.notify();
                            }
                        });
                    });
                let view = weak.clone();
                let reload = menu_item("重新加载配置").disabled(!has_store).on_click(
                    move |_, window, cx| {
                        let _ = view.update(cx, |v, cx| v.request_reload(window, cx));
                    },
                );
                let view = weak.clone();
                let quit = menu_item("退出…").on_click(move |_, window, cx| {
                    let _ = view.update(cx, |v, cx| v.request_exit(window, cx));
                });
                menu = menu
                    .item(new_tab)
                    .item(copy_tab)
                    .separator()
                    .item(all)
                    .item(none)
                    .separator()
                    .item(logs)
                    .separator()
                    .item(save)
                    .item(export)
                    .item(menu_item("导入配置…").disabled(!has_store).on_click({
                        let view = weak.clone();
                        move |_, w, cx| {
                            let _ = view.update(cx, |v, cx| v.prompt_import(false, w, cx));
                        }
                    }))
                    .item(menu_item("导出配置…").disabled(!has_store).on_click({
                        let view = weak.clone();
                        move |_, w, cx| {
                            let _ = view.update(cx, |v, cx| v.prompt_export(None, w, cx));
                        }
                    }))
                    .item(reload)
                    .item(settings)
                    .item(menu_item("恢复配置备份").disabled(!has_store).on_click({
                        let view = weak.clone();
                        move |_, window, cx| {
                            let _ = view.update(cx, |v, cx| v.open_backups(window, cx));
                        }
                    }))
                    .separator()
                    .item(quit);
                menu
            },
        )
    }

    fn set_tab_icon(
        &mut self,
        id: usize,
        source: Option<String>,
        svg: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) else {
            return;
        };
        self.pending_icon_sources.remove(&id);
        tab.icon_source = source;
        tab.icon_svg = svg;
        tab.dirty = true;
        self.config_dirty = true;
        self.status = tr(cx, "标签图标已更改；保存配置后持久化");
        cx.notify();
    }

    pub(crate) fn set_tab_icon_from_source(
        &mut self,
        id: usize,
        source: String,
        cx: &mut Context<Self>,
    ) {
        let source = source.trim().to_owned();
        if source.is_empty() {
            self.set_tab_icon(id, None, None, cx);
        } else if let Some(name) = source.strip_prefix("builtin:") {
            if tab_icons::builtin(name).is_some() {
                self.set_tab_icon(id, Some(format!("builtin:{name}")), None, cx);
            } else {
                self.status = tr(cx, "找不到该内置图标");
                cx.notify();
            }
        } else if source.starts_with("https://") || source.starts_with("http://") {
            if let Err(error) = tab_icons::validate_url(&source) {
                self.status = error;
                cx.notify();
                return;
            }
            self.pending_icon_sources.insert(id, source.clone());
            let owner = cx.entity().downgrade();
            let client = cx.http_client();
            let timeout = cx
                .background_executor()
                .timer(std::time::Duration::from_secs(15));
            self.status = tr(cx, "正在下载 SVG 图标…");
            cx.notify();
            cx.spawn(async move |_, cx| {
                let fetch = async {
                    // No redirects: an HTTPS URL must never redirect to a local/http resource.
                    let mut response = client
                        .get(&source, ().into(), false)
                        .await
                        .map_err(|e| format!("SVG 下载失败：{e}"))?;
                    if !response.status().is_success() {
                        return Err(format!("SVG 服务器返回 {}", response.status()));
                    }
                    let mut bytes = Vec::new();
                    response
                        .body_mut()
                        .take((tab_icons::MAX_SVG_BYTES + 1) as u64)
                        .read_to_end(&mut bytes)
                        .await
                        .map_err(|e| format!("SVG 下载中断：{e}"))?;
                    tab_icons::validate_svg(&bytes)
                };
                let result: Result<String, String> =
                    match futures::future::select(Box::pin(fetch), Box::pin(timeout)).await {
                        futures::future::Either::Left((result, _)) => result,
                        futures::future::Either::Right(_) => Err("SVG 下载超时（15 秒）。".into()),
                    };
                let _ = owner.update(cx, |view, cx| {
                    if view.pending_icon_sources.get(&id) != Some(&source) {
                        return;
                    }
                    view.pending_icon_sources.remove(&id);
                    match result {
                        Ok(data) => view.set_tab_icon(id, Some(source), Some(data), cx),
                        Err(error) => {
                            view.status = error;
                            cx.notify();
                        }
                    }
                });
            })
            .detach();
        } else {
            let local = if source.starts_with("file://") {
                url::Url::parse(&source)
                    .ok()
                    .and_then(|url| url.to_file_path().ok())
                    .and_then(|path| path.to_str().map(str::to_owned))
            } else {
                Some(source)
            };
            let Some(local) = local else {
                self.status = tr(cx, "SVG 文件地址无效。");
                cx.notify();
                return;
            };
            match tab_icons::read_local(&local) {
                Ok(data) => self.set_tab_icon(id, Some(local), Some(data), cx),
                Err(error) => {
                    self.status = error;
                    cx.notify();
                }
            }
        }
    }

    fn open_icon_picker(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.iter().find(|tab| tab.id == id) else {
            return;
        };
        let current = tab.icon_source.clone().unwrap_or_default();
        let owner = cx.entity().downgrade();
        let picker =
            cx.new(|cx| crate::icon_picker::IconPicker::new(owner, id, current, window, cx));
        window.open_dialog(cx, move |dialog, _, cx| {
            let selected = picker.clone();
            dialog
                .title(tr(cx, "选择标签图标"))
                .w(px(510.))
                .on_ok(move |_, w, cx| {
                    selected.update(cx, |picker, cx| picker.confirm_selection(w, cx))
                })
                .child(picker.clone())
        });
    }

    fn validate_sidebar_group_name(
        &self,
        name: &str,
        except: Option<u64>,
        cx: &App,
    ) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err(tr(cx, "分组名称不能为空").to_string());
        }
        if name.chars().count() > 48 {
            return Err(tr(cx, "分组名称不能超过 48 个字符").to_string());
        }
        let normalized = name.to_lowercase();
        if ["未分组", "Unassigned"]
            .iter()
            .any(|reserved| normalized == reserved.to_lowercase())
        {
            return Err(tr(cx, "这个名称保留给未分组区域").to_string());
        }
        if self
            .sidebar_groups
            .iter()
            .any(|group| Some(group.id) != except && group.name.to_lowercase() == normalized)
        {
            return Err(tr(cx, "已有同名分组").to_string());
        }
        Ok(name.to_owned())
    }

    fn create_sidebar_group_named(
        &mut self,
        name: &str,
        cx: &mut Context<Self>,
    ) -> Result<u64, String> {
        let name = self.validate_sidebar_group_name(name, None, cx)?;
        let mut id = self.next_sidebar_group_id;
        while id == u64::MAX || self.sidebar_groups.iter().any(|group| group.id == id) {
            id = id.wrapping_add(1);
        }
        self.next_sidebar_group_id = id.wrapping_add(1);
        self.sidebar_groups.push(SidebarGroup {
            id,
            name,
            collapsed: false,
        });
        self.sidebar_ungrouped_collapsed = false;
        self.config_dirty = true;
        self.status = tr(cx, "已创建分组；保存配置后生效");
        cx.notify();
        Ok(id)
    }

    fn rename_sidebar_group_named(
        &mut self,
        group_id: u64,
        name: &str,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let name = self.validate_sidebar_group_name(name, Some(group_id), cx)?;
        let Some(group) = self
            .sidebar_groups
            .iter_mut()
            .find(|group| group.id == group_id)
        else {
            return Err(tr(cx, "分组已不存在").to_string());
        };
        if group.name != name {
            group.name = name;
            self.config_dirty = true;
            self.status = tr(cx, "分组名称已修改；保存配置后生效");
            cx.notify();
        }
        Ok(())
    }

    fn assign_tab_to_group(
        &mut self,
        tab_id: usize,
        group_id: Option<u64>,
        cx: &mut Context<Self>,
    ) {
        if group_id.is_some_and(|id| !self.sidebar_groups.iter().any(|group| group.id == id)) {
            return;
        }
        let Some(index) = self.tabs.iter().position(|tab| tab.id == tab_id) else {
            return;
        };
        if self.tabs[index].sidebar_group_id == group_id {
            self.tab_menu = None;
            self.tab_group_options_open = false;
            return;
        }
        self.tabs[index].sidebar_group_id = group_id;
        if let Some(id) = group_id {
            if let Some(group) = self.sidebar_groups.iter_mut().find(|group| group.id == id) {
                group.collapsed = false;
            }
        } else {
            self.sidebar_ungrouped_collapsed = false;
        }
        self.config_dirty = true;
        self.sidebar_hovered = None;
        self.tab_menu = None;
        self.tab_group_options_open = false;
        self.scroll_sidebar_to_tab(index);
        self.status = tr(cx, "配置已移动；保存配置后生效");
        cx.notify();
    }

    fn commit_sidebar_group_edit(
        &mut self,
        edit: SidebarGroupEdit,
        name: &str,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        match edit {
            SidebarGroupEdit::Create => self.create_sidebar_group_named(name, cx).map(|_| ()),
            SidebarGroupEdit::CreateAndMove(tab_id) => {
                let group_id = self.create_sidebar_group_named(name, cx)?;
                self.assign_tab_to_group(tab_id, Some(group_id), cx);
                Ok(())
            }
            SidebarGroupEdit::Rename(group_id) => {
                self.rename_sidebar_group_named(group_id, name, cx)
            }
        }
    }

    fn open_sidebar_group_editor(
        &mut self,
        edit: SidebarGroupEdit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let initial_name = match edit {
            SidebarGroupEdit::Create => String::new(),
            SidebarGroupEdit::CreateAndMove(tab_id) => self
                .tabs
                .iter()
                .find(|tab| tab.id == tab_id)
                .map(|tab| {
                    let tab_name = tab.label(cx).chars().take(40).collect::<String>();
                    i18n::format(cx, "{} 分组", &[&tab_name])
                })
                .unwrap_or_default(),
            SidebarGroupEdit::Rename(group_id) => self
                .sidebar_groups
                .iter()
                .find(|group| group.id == group_id)
                .map(|group| group.name.clone())
                .unwrap_or_default(),
        };
        let group_name = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(initial_name)
                .placeholder(tr(cx, "分组名称"))
        });
        let group_name_focus = group_name.focus_handle(cx);
        self.sidebar_group_name_input = Some(group_name.clone());
        let group_name_for_dialog = group_name.clone();
        let owner = cx.entity().downgrade();
        let title = tr(
            cx,
            match edit {
                SidebarGroupEdit::Rename(_) => "重命名分组",
                _ => "新建分组",
            },
        );
        window.open_dialog(cx, move |dialog, _, cx| {
            let save_name = group_name_for_dialog.clone();
            let save_owner = owner.clone();
            let cancel_owner = owner.clone();
            let close_owner = owner.clone();
            dialog
                .on_close(move |_, _, cx| {
                    let _ = close_owner.update(cx, |view, _| {
                        view.sidebar_group_name_input = None;
                    });
                })
                .title(title.clone())
                .w(px(400.))
                .child(input(
                    "sidebar-group-name-input",
                    &group_name_for_dialog,
                    "分组名称",
                    false,
                    cx,
                ))
                .footer(
                    row()
                        .justify_end()
                        .py(px(GAP))
                        .child(button("cancel-sidebar-group", "取消", None, cx).on_click(
                            move |_, window, cx| {
                                let _ = cancel_owner.update(cx, |view, _| {
                                    view.sidebar_group_name_input = None;
                                });
                                window.close_dialog(cx);
                            },
                        ))
                        .child(
                            button("save-sidebar-group", "确定", None, cx)
                                .primary()
                                .on_click(move |_, window, cx| {
                                    let name = save_name.read(cx).value().trim().to_owned();
                                    match save_owner.update(cx, |view, cx| {
                                        let result =
                                            view.commit_sidebar_group_edit(edit, &name, cx);
                                        view.sidebar_group_name_input = None;
                                        result
                                    }) {
                                        Ok(Ok(())) => window.close_dialog(cx),
                                        Ok(Err(error)) => {
                                            let _ = save_owner.update(cx, |view, cx| {
                                                view.status = error;
                                                cx.notify();
                                            });
                                            window.close_dialog(cx);
                                        }
                                        Err(_) => window.close_dialog(cx),
                                    }
                                }),
                        ),
                )
        });
        window.focus(&group_name_focus, cx);
    }

    fn request_delete_sidebar_group(
        &mut self,
        group_id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(group) = self
            .sidebar_groups
            .iter()
            .find(|group| group.id == group_id)
        else {
            return;
        };
        let name = group.name.clone();
        let count = self
            .tabs
            .iter()
            .filter(|tab| tab.sidebar_group_id == Some(group_id))
            .count();
        let owner = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, cx| {
            let owner = owner.clone();
            let name = name.clone();
            dialog
                .title(tr(cx, "删除分组？"))
                .child(i18n::format(
                    cx,
                    "删除「{}」分组会将其中 {} 个配置移回“未分组”，不会删除配置。",
                    &[&name, &count.to_string()],
                ))
                .footer(
                    row()
                        .justify_end()
                        .py(px(GAP))
                        .child(
                            button("cancel-delete-sidebar-group", "取消", None, cx)
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            button("confirm-delete-sidebar-group", "删除分组", None, cx).on_click(
                                move |_, window, cx| {
                                    window.close_dialog(cx);
                                    let _ = owner.update(cx, |view, cx| {
                                        view.delete_sidebar_group(group_id, cx);
                                    });
                                },
                            ),
                        ),
                )
        });
    }

    fn delete_sidebar_group(&mut self, group_id: u64, cx: &mut Context<Self>) {
        let Some(index) = self
            .sidebar_groups
            .iter()
            .position(|group| group.id == group_id)
        else {
            return;
        };
        let name = self.sidebar_groups.remove(index).name;
        for tab in &mut self.tabs {
            if tab.sidebar_group_id == Some(group_id) {
                tab.sidebar_group_id = None;
            }
        }
        self.sidebar_ungrouped_collapsed = false;
        self.config_dirty = true;
        self.tab_menu = None;
        self.tab_group_options_open = false;
        self.status = i18n::format(
            cx,
            "已删除分组「{}」；配置已移至未分组，保存后生效",
            &[&name],
        );
        self.scroll_sidebar_to_tab(self.active);
        cx.notify();
    }

    fn toggle_sidebar_group(&mut self, group_id: Option<u64>, cx: &mut Context<Self>) {
        let changed = if let Some(group_id) = group_id {
            if let Some(group) = self
                .sidebar_groups
                .iter_mut()
                .find(|group| group.id == group_id)
            {
                group.collapsed = !group.collapsed;
                true
            } else {
                false
            }
        } else if !self.sidebar_groups.is_empty() {
            self.sidebar_ungrouped_collapsed = !self.sidebar_ungrouped_collapsed;
            true
        } else {
            false
        };
        if changed {
            self.config_dirty = true;
            self.scroll_sidebar_to_tab(self.active);
            cx.notify();
        }
    }

    fn default_sidebar_width(viewport: f32) -> f32 {
        (viewport * 0.25).clamp(160., 208.)
    }

    fn max_sidebar_width(viewport: f32) -> f32 {
        // Leave enough space for the compact form, both preview panes, and the parse input.
        (viewport - 448.).min(viewport * 0.45).min(400.).max(160.)
    }

    fn expanded_sidebar_width(&self, viewport: f32) -> f32 {
        self.sidebar_expanded_width
            .unwrap_or_else(|| Self::default_sidebar_width(viewport))
            .clamp(160., Self::max_sidebar_width(viewport))
    }

    fn sidebar_folded(&self) -> bool {
        self.sidebar_collapsed && !self.sidebar_auto_expanded
    }

    fn sidebar_width(&self, viewport: f32) -> f32 {
        // Only a pinned expansion participates in workspace layout. Hover uses
        // a separate overlay spring over the editor, leaving editor bounds fixed.
        if self.sidebar_collapsed {
            56.
        } else {
            self.expanded_sidebar_width(viewport)
        }
    }

    fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        // An open hover preview is pinned by clicking its toggle; an explicitly
        // collapsed panel stays closed until the pointer genuinely leaves it.
        self.sidebar_collapsed = !self.sidebar_collapsed;
        self.sidebar_auto_expanded = false;
        self.sidebar_auto_suppressed = self.sidebar_collapsed;
        self.sidebar_hovered = None;
        self.scroll_sidebar_to_top_of_tab(self.active);
        self.config_dirty = true;
        cx.notify();
    }

    fn pointer_in_sidebar(position: gpui::Point<gpui::Pixels>, width: f32, height: f32) -> bool {
        let x = f32::from(position.x);
        let y = f32::from(position.y);
        x >= 0. && x < width && y >= TITLE_HEIGHT && y < height
    }

    fn hover_sidebar(&mut self, hovered: bool, window: &Window, cx: &mut Context<Self>) {
        if !hovered {
            let height = f32::from(window.viewport_size().height);
            if self.sidebar_auto_expanded
                && Self::pointer_in_sidebar(
                    window.mouse_position(),
                    self.expanded_sidebar_width(f32::from(window.viewport_size().width))
                        .max(self.sidebar_overlay_display_width),
                    height,
                )
            {
                // The pointer may outrun the opening spring. Keep the preview
                // while it is inside the eventual overlay, not merely its current width.
                return;
            }
            if !Self::pointer_in_sidebar(window.mouse_position(), 56., height) {
                self.sidebar_auto_suppressed = false;
            }
            let changed = self.sidebar_auto_expanded || self.sidebar_hovered.is_some();
            self.sidebar_auto_expanded = false;
            self.sidebar_hovered = None;
            if changed {
                cx.notify();
            }
        } else if self.sidebar_collapsed
            && !self.sidebar_auto_expanded
            && !self.sidebar_auto_suppressed
            && self.resizing.is_none()
        {
            self.sidebar_auto_expanded = true;
            cx.notify();
        }
    }

    fn sidebar_group_options(
        &self,
        tab_id: usize,
        menu_top: f32,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let current_group = self
            .tabs
            .iter()
            .find(|tab| tab.id == tab_id)
            .and_then(|tab| tab.sidebar_group_id);
        let mut options = column().gap(px(2.));
        if self.sidebar_groups.is_empty() {
            options = options.child(
                button(
                    "context-create-group",
                    "新建分组并移动此配置",
                    Some(IconName::FolderPlus),
                    cx,
                )
                .ghost()
                .w_full()
                .on_click(cx.listener(move |view, _, window, cx| {
                    view.tab_menu = None;
                    view.tab_group_options_open = false;
                    view.open_sidebar_group_editor(
                        SidebarGroupEdit::CreateAndMove(tab_id),
                        window,
                        cx,
                    );
                })),
            );
        } else {
            options = options.child(
                button(
                    "context-group-ungrouped",
                    "未分组",
                    current_group.is_none().then_some(IconName::Check),
                    cx,
                )
                .ghost()
                .w_full()
                .disabled(current_group.is_none())
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.assign_tab_to_group(tab_id, None, cx);
                })),
            );
            for group in &self.sidebar_groups {
                let group_id = group.id;
                let full_name = group.name.clone();
                let visible_name = if full_name.chars().count() > 18 {
                    full_name
                        .chars()
                        .take(17)
                        .chain(std::iter::once('…'))
                        .collect()
                } else {
                    full_name.clone()
                };
                let selected = current_group == Some(group_id);
                options = options.child(
                    button(
                        ("context-group-option", group_id),
                        &visible_name,
                        selected.then_some(IconName::Check),
                        cx,
                    )
                    .accessibility_label(full_name.clone())
                    .tooltip(full_name)
                    .ghost()
                    .w_full()
                    .disabled(selected)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.assign_tab_to_group(tab_id, Some(group_id), cx);
                    })),
                );
            }
            options = options.child(
                button(
                    "context-create-group",
                    "新建分组并移动此配置…",
                    Some(IconName::FolderPlus),
                    cx,
                )
                .ghost()
                .w_full()
                .on_click(cx.listener(move |view, _, window, cx| {
                    view.tab_menu = None;
                    view.tab_group_options_open = false;
                    view.open_sidebar_group_editor(
                        SidebarGroupEdit::CreateAndMove(tab_id),
                        window,
                        cx,
                    );
                })),
            );
        }
        frame("context-group-options")
            .max_h(px((f32::from(window.viewport_size().height)
                - menu_top
                - 64.)
                .clamp(120., 300.)))
            .overflow_y_scroll()
            .child(options)
    }

    fn sidebar_section_header(
        &self,
        group_id: Option<u64>,
        show_details: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let key = group_id.unwrap_or(u64::MAX);
        let group = group_id.and_then(|id| self.sidebar_groups.iter().find(|group| group.id == id));
        let name = group
            .map(|group| group.name.clone())
            .unwrap_or_else(|| tr(cx, "未分组").to_string());
        let collapsed = group
            .map(|group| group.collapsed)
            .unwrap_or(self.sidebar_ungrouped_collapsed);
        let active_in_section = collapsed && self.tabs[self.active].sidebar_group_id == group_id;
        let toggle_label = format!(
            "{}：{}",
            tr(
                cx,
                if collapsed {
                    "展开分组"
                } else {
                    "折叠分组"
                }
            ),
            name
        );
        let toggle_icon = if collapsed {
            IconName::ChevronRight
        } else {
            IconName::ChevronDown
        };
        let toggle = Button::new(("sidebar-group-toggle", key))
            .icon(Icon::new(toggle_icon).size(px(ICON)))
            .accessibility_label(toggle_label.clone())
            .ghost()
            .with_size(Size::Medium)
            .h(px(CONTROL))
            .flex_1()
            .min_w_0()
            .px(px(4.))
            .gap(px(GAP))
            .justify_start()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(theme::font_size(cx)))
                    .font_weight(theme::font_weight(cx))
                    .text_color(rgb(if active_in_section {
                        palette(cx).focus
                    } else {
                        palette(cx).muted
                    }))
                    .child(name.clone()),
            )
            .on_click(cx.listener(move |view, _, _, cx| {
                view.toggle_sidebar_group(group_id, cx);
            }));
        let mut header = row()
            .w_full()
            .h(px(CONTROL))
            .flex_shrink_0()
            .items_center()
            .gap(px(2.))
            .when(!show_details, |header| header.justify_center())
            .child(if show_details {
                toggle.into_any_element()
            } else {
                icon_button(
                    ("sidebar-group-toggle", key),
                    &toggle_label,
                    IconName::FolderOpen,
                    cx,
                )
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.toggle_sidebar_group(group_id, cx);
                }))
                .into_any_element()
            });
        if show_details {
            if let Some(group_id) = group_id {
                let owner = cx.entity().downgrade();
                let group_name = name.clone();
                let menu_label = i18n::format(cx, "分组操作：{}", &[&group_name]);
                header = header.child(
                    icon_button(
                        ("sidebar-group-menu", group_id),
                        &menu_label,
                        IconName::Ellipsis,
                        cx,
                    )
                    .dropdown_menu(move |mut menu, _, _| {
                        let rename_owner = owner.clone();
                        let rename = menu_item("重命名分组")
                            .icon(Icon::new(IconName::Settings2))
                            .on_click(move |_, window, cx| {
                                let _ = rename_owner.update(cx, |view, cx| {
                                    view.open_sidebar_group_editor(
                                        SidebarGroupEdit::Rename(group_id),
                                        window,
                                        cx,
                                    );
                                });
                            });
                        let delete_owner = owner.clone();
                        let delete = menu_item("删除分组…")
                            .icon(Icon::new(IconName::Trash))
                            .on_click(move |_, window, cx| {
                                let _ = delete_owner.update(cx, |view, cx| {
                                    view.request_delete_sidebar_group(group_id, window, cx);
                                });
                            });
                        menu = menu.item(rename).separator().item(delete);
                        menu
                    }),
                );
            }
        }
        frame(("sidebar-group-header", key))
            .w_full()
            .h(px(CONTROL))
            .flex_shrink_0()
            .flex()
            .items_center()
            .child(header)
    }

    fn sidebar(
        &self,
        width: f32,
        expanded_width: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = palette(cx);
        let folded = self.sidebar_folded();
        let progress = ((width - 56.) / (expanded_width - 56.)).clamp(0., 1.);
        let show_details = progress > 0.015 || !folded;
        let entries = self.sidebar_entries();
        let mut row_tops = vec![None; self.tabs.len()];
        let mut section_tops = std::collections::HashMap::new();
        let mut content_height = 0.;
        let mut first_tab_index = None;
        for entry in &entries {
            match entry {
                SidebarEntry::Section(group_id) => {
                    section_tops.insert(*group_id, content_height);
                    content_height += CONTROL + 4.;
                }
                SidebarEntry::Tab(index) => {
                    first_tab_index.get_or_insert(*index);
                    row_tops[*index] = Some(content_height);
                    content_height += 40. + 4.;
                }
            }
        }
        let first_tab_top = first_tab_index
            .and_then(|index| row_tops[index])
            .unwrap_or(0.);
        // Long lists scroll independently of the first-row decoration anchor.
        // A spring travelling through offscreen rows leaves a misleading hover
        // highlight far from the pointer, so snap while the list overflows.
        let list_height = f32::from(window.viewport_size().height)
            - TITLE_HEIGHT
            - STATUS_HEIGHT
            - CONTROL
            - GAP * 3.
            - 16.;
        let overflowing = content_height + 16. > list_height;
        let motion = gpui::base::Spring::new(std::time::Duration::from_millis(240))
            .with_epsilon(0.2)
            .with_travel(!self.reduced_motion && !overflowing);
        let active_visible = row_tops[self.active].is_some();
        let selected_target = row_tops[self.active]
            .or_else(|| {
                section_tops
                    .get(&self.tabs[self.active].sidebar_group_id)
                    .copied()
            })
            .unwrap_or(first_tab_top);
        let selected_y =
            gpui::base::spring("sidebar-selection-y", selected_target, motion, window, cx);
        let hovered_index = self
            .sidebar_hovered
            .and_then(|id| self.tabs.iter().position(|t| t.id == id))
            .filter(|index| row_tops[*index].is_some());
        let hover_target = hovered_index
            .and_then(|index| row_tops[index])
            .unwrap_or(selected_target);
        let hover_y = gpui::base::spring("sidebar-hover-y", hover_target, motion, window, cx);
        let hover_opacity = gpui::base::transition(
            "sidebar-hover-opacity",
            if hovered_index.is_some() && !overflowing {
                1_f32
            } else {
                0.
            },
            gpui::base::Transition::new(std::time::Duration::from_millis(
                if self.reduced_motion || overflowing {
                    0
                } else {
                    130
                },
            )),
            window,
            cx,
        );
        let mut items = frame("sidebar-items")
            .flex()
            .flex_col()
            .gap(px(4.))
            .w_full()
            .flex_1()
            .min_h_0()
            // Keep the overlay scrollbar's track clear of row hover/selection
            // fills. The collapsed rail hides the scrollbar and stays centered.
            .when(!folded, |items| items.pr(px(SIDEBAR_LIST_RIGHT_PADDING)))
            .on_scroll_wheel(cx.listener(|v, _, _, cx| {
                // A wheel event scrolls rows beneath a stationary pointer without a MouseMove.
                // Clear the old row before the next frame can paint it at a stale screen position.
                if v.sidebar_hovered.take().is_some() {
                    cx.notify();
                }
            }))
            .overflow_y_scroll()
            .track_scroll(&self.sidebar_scroll);
        for entry in &entries {
            let index = match *entry {
                SidebarEntry::Section(group_id) => {
                    items = items.child(self.sidebar_section_header(group_id, show_details, cx));
                    continue;
                }
                SidebarEntry::Tab(index) => index,
            };
            let tab = &self.tabs[index];
            let id = tab.id;
            let label = tab.label(cx);
            let hover_group = format!("sidebar-row-{id}");
            // White only on the selected pill; all other glyphs follow the surface text.
            // During travel keep the destination legible until the pill reaches it.
            let row_top = row_tops[index].unwrap_or(first_tab_top);
            let selected_visible = !self.open_tab_ids.is_empty()
                && active_visible
                && index == self.active
                && (overflowing || (selected_y - row_top).abs() < 20.);
            let foreground = if selected_visible {
                p.on_primary
            } else {
                p.text
            };
            let icon = if let Some(bytes) = tab
                .icon_source
                .as_deref()
                .and_then(|s| s.strip_prefix("builtin:"))
                .and_then(tab_icons::builtin)
            {
                gpui::svg()
                    .data(bytes)
                    .size(px(20.))
                    .text_color(rgb(foreground))
                    .into_any_element()
            } else if let Some(source) = tab.icon_svg.as_ref() {
                gpui::svg()
                    .data(source.as_bytes())
                    .size(px(20.))
                    .text_color(rgb(foreground))
                    .into_any_element()
            } else {
                Icon::new(IconName::Terminal)
                    .size(px(20.))
                    .text_color(rgb(foreground))
                    .into_any_element()
            };
            items = items.child(
                frame(("sidebar-tab", id))
                    .group(hover_group.clone())
                    .w_full()
                    .h(px(40.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(GAP))
                    .px(px(GAP))
                    .when(!show_details, |d| d.px_0().justify_center())
                    .rounded(px(RADIUS))
                    .relative()
                    .text_color(rgb(foreground))
                    .on_hover(cx.listener(move |v, hovered, _, cx| {
                        if *hovered {
                            v.sidebar_hovered = Some(id);
                        } else if v.sidebar_hovered == Some(id) {
                            v.sidebar_hovered = None;
                        }
                        cx.notify();
                    }))
                    // Animate short lists with one indicator anchored inside row zero.
                    // In a scrollable list, paint each highlight inside its own row instead:
                    // an absolute index-based marker can drift as rows move under a stationary pointer.
                    .when(Some(index) == first_tab_index && !overflowing, |d| {
                        d.child(
                            frame("sidebar-hover-indicator")
                                .absolute()
                                .left_0()
                                .top(px((hover_y - first_tab_top).max(0.)))
                                .w_full()
                                .h(px(40.))
                                .rounded(px(RADIUS))
                                .bg(rgb(p.hover))
                                .opacity(hover_opacity),
                        )
                        .child(
                            frame("sidebar-selection-indicator")
                                .absolute()
                                .left_0()
                                .top(px((selected_y - first_tab_top).max(0.)))
                                .w_full()
                                .h(px(40.))
                                .rounded(px(RADIUS))
                                .bg(rgb(p.selected_tab))
                                .opacity(if self.open_tab_ids.is_empty() || !active_visible {
                                    0.
                                } else {
                                    1.
                                }),
                        )
                    })
                    .when(overflowing && hovered_index == Some(index), |d| {
                        d.child(
                            frame(("sidebar-hover-row-indicator", id))
                                .absolute()
                                .left_0()
                                .top_0()
                                .w_full()
                                .h_full()
                                .rounded(px(RADIUS))
                                .bg(rgb(p.hover)),
                        )
                    })
                    .when(overflowing && selected_visible, |d| {
                        d.child(
                            frame(("sidebar-selection-row-indicator", id))
                                .absolute()
                                .left_0()
                                .top_0()
                                .w_full()
                                .h_full()
                                .rounded(px(RADIUS))
                                .bg(rgb(p.selected_tab)),
                        )
                    })
                    .tooltip({
                        let label = label.clone();
                        move |w, cx| {
                            gpui::component::tooltip::Tooltip::new(label.clone()).build(w, cx)
                        }
                    })
                    .cursor(gpui::CursorStyle::PointingHand)
                    .on_click(cx.listener(move |v, _, w, cx| v.select_tab(index, w, cx)))
                    .on_drag(
                        TabDrag {
                            tab_id: id,
                            area: TabArea::Sidebar,
                        },
                        {
                            let label = label.clone();
                            move |_, _, _, cx| {
                                cx.new(|_| RowDragPreview {
                                    label: label.clone(),
                                })
                            }
                        },
                    )
                    .on_drag_move::<TabDrag>({
                        let owner = cx.entity().downgrade();
                        move |event, _, cx| {
                            let drag = event.drag(cx).clone();
                            let center = f32::from(event.bounds.center().y);
                            let pointer = f32::from(event.event.position.y);
                            let inside = event.bounds.contains(&event.event.position);
                            let _ = owner.update(cx, |v, cx| {
                                if !inside {
                                    if v.tab_drag_target.is_some_and(|(area, target, _)| {
                                        area == TabArea::Sidebar && target == id
                                    }) {
                                        v.tab_drag_target = None;
                                        cx.notify();
                                    }
                                    return;
                                }
                                let same_group = v
                                    .tabs
                                    .iter()
                                    .find(|tab| tab.id == drag.tab_id)
                                    .zip(v.tabs.iter().find(|tab| tab.id == id))
                                    .is_some_and(|(source, target)| {
                                        source.sidebar_group_id == target.sidebar_group_id
                                    });
                                let target = (drag.area == TabArea::Sidebar
                                    && drag.tab_id != id
                                    && same_group)
                                    .then(|| {
                                        (
                                            TabArea::Sidebar,
                                            id,
                                            v.tab_drop_after(&drag, id, pointer, center),
                                        )
                                    });
                                if target.is_some() {
                                    v.tab_drag_centers.insert((TabArea::Sidebar, id), center);
                                }
                                if v.tab_drag_target != target {
                                    v.tab_drag_target = target;
                                    cx.notify();
                                }
                            });
                        }
                    })
                    .on_drop::<TabDrag>({
                        let owner = cx.entity().downgrade();
                        move |drag, w, cx| {
                            let _ = owner.update(cx, |v, cx| {
                                if drag.area == TabArea::Sidebar {
                                    if let Some(center) =
                                        v.tab_drag_centers.get(&(TabArea::Sidebar, id))
                                    {
                                        let after = v.tab_drop_after(
                                            drag,
                                            id,
                                            f32::from(w.mouse_position().y),
                                            *center,
                                        );
                                        v.reorder_tabs(drag, id, after, cx);
                                    }
                                }
                            });
                        }
                    })
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |v, event: &gpui::MouseDownEvent, w, cx| {
                            cx.stop_propagation();
                            let x = f32::from(event.position.x)
                                .min(f32::from(w.viewport_size().width) - 208.)
                                .max(8.);
                            let y = f32::from(event.position.y)
                                .min(f32::from(w.viewport_size().height) - 148.)
                                .max(8.);
                            v.tab_group_options_open = false;
                            v.tab_menu = Some((id, x, y));
                            cx.notify();
                        }),
                    )
                    .when(show_details && !folded, |d| {
                        d.child(
                            frame(("sidebar-open-lane", id))
                                .w(px(12.))
                                .h(px(16.))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .when(self.open_tab_ids.contains(&id), |lane| {
                                    lane.child(
                                        frame(("sidebar-open-marker", id))
                                            .w(px(2.))
                                            .h(px(16.))
                                            .rounded_full()
                                            .bg(rgb(if selected_visible {
                                                p.on_primary
                                            } else {
                                                p.focus
                                            })),
                                    )
                                }),
                        )
                    })
                    .child(
                        frame(("sidebar-icon", id))
                            .size(px(24.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .relative()
                            .rounded_full()
                            // In hover preview the icon still selects the config, as it
                            // did in the collapsed rail. Only pinned expansion edits it.
                            .when(!folded && !self.sidebar_auto_expanded, |slot| {
                                slot.cursor(gpui::CursorStyle::PointingHand)
                                    .hover(|slot| {
                                        slot.bg(gpui::Hsla::from(rgb(if selected_visible {
                                            p.on_primary
                                        } else {
                                            p.focus
                                        }))
                                        .opacity(if selected_visible { 0.24 } else { 0.32 }))
                                    })
                                    .tooltip(|w, cx| {
                                        gpui::component::tooltip::Tooltip::new(tr(
                                            cx,
                                            "选择标签图标",
                                        ))
                                        .build(w, cx)
                                    })
                                    .on_click(cx.listener(move |v, _, w, cx| {
                                        cx.stop_propagation();
                                        v.open_icon_picker(id, w, cx);
                                    }))
                            })
                            .child(icon),
                    )
                    .when(folded && self.open_tab_ids.contains(&id), |d| {
                        // Keep the SVG centered in the collapsed rail. The open
                        // status mark rides the leading edge as an overlay so it
                        // does not turn the narrow rail into a two-column layout.
                        d.child(
                            frame(("sidebar-open-marker", id))
                                .absolute()
                                .left(px(2.))
                                .top(px(12.))
                                .w(px(2.))
                                .h(px(16.))
                                .rounded_full()
                                .bg(rgb(if selected_visible {
                                    p.on_primary
                                } else {
                                    p.focus
                                })),
                        )
                    })
                    .when(show_details, |d| {
                        let details = row()
                            .relative()
                            .flex_1()
                            .min_w_0()
                            // Protect the trailing delete action from the overlay
                            // scrollbar without narrowing the highlight or icon rail.
                            .pr(px(12.))
                            .child(div().flex_1().min_w_0().truncate().child(label))
                            .child(
                                icon_button(("sidebar-close-tab", id), "删除配置", IconName::X, cx)
                                    .with_size(Size::Size(px(20.)))
                                    .size(px(20.))
                                    // Reserve its space, but do not paint or hit-test an invisible X.
                                    .invisible()
                                    .group_hover(hover_group, |style| style.visible())
                                    .custom(
                                        ButtonCustomVariant::new(cx)
                                            .foreground(rgb(foreground).into())
                                            .hover(
                                                rgb(if selected_visible {
                                                    p.primary_hover
                                                } else {
                                                    p.hover
                                                })
                                                .into(),
                                            ),
                                    )
                                    .disabled(folded || self.tabs.len() == 1)
                                    .on_click(cx.listener(move |v, _, w, cx| {
                                        cx.stop_propagation();
                                        v.request_delete(index, w, cx);
                                    })),
                            );
                        d.child(details.opacity(progress))
                    })
                    .when_some(
                        self.tab_drag_target
                            .filter(|(area, target, _)| *area == TabArea::Sidebar && *target == id),
                        |d, (_, _, after)| {
                            d.child(
                                frame(("sidebar-tab-drop", id))
                                    .absolute()
                                    .left_0()
                                    .right_0()
                                    .h(px(2.))
                                    .when(after, |line| line.bottom_0())
                                    .when(!after, |line| line.top_0())
                                    .bg(rgb(p.focus)),
                            )
                        },
                    )
                    .when(folded && tab.dirty, |d| {
                        // An overlay dot preserves the rail geometry. A right border left a
                        // stray accent strip and narrowed both the icon and sliding backgrounds.
                        d.child(
                            frame(("sidebar-dirty-dot", id))
                                .absolute()
                                .top(px(3.))
                                .right(px(3.))
                                .size(px(4.))
                                .rounded_full()
                                .bg(rgb(if selected_visible {
                                    foreground
                                } else {
                                    p.focus
                                })),
                        )
                    }),
            );
        }
        frame("tab-sidebar")
            .relative()
            .overflow_hidden()
            .w(px(width))
            .flex_shrink_0()
            .h_full()
            .min_h_0()
            .bg(rgb(p.sidebar))
            .on_hover(cx.listener(|v, hovered, w, cx| v.hover_sidebar(*hovered, w, cx)))
            .hover_listener_mode(gpui::HoverListenerMode::InputModalityIndependent)
            .border_r_1()
            .border_color(rgb(p.divider))
            .flex()
            .flex_col()
            .gap(px(GAP))
            .p(px(GAP))
            .child(
                row()
                    .h(px(CONTROL))
                    .flex_shrink_0()
                    .when(folded, |header| header.justify_center())
                    .child(
                        icon_button(
                            "collapse-sidebar",
                            if folded {
                                "展开标签侧栏"
                            } else {
                                "折叠标签侧栏"
                            },
                            if folded {
                                IconName::ListIndentIncrease
                            } else {
                                IconName::ListIndentDecrease
                            },
                            cx,
                        )
                        .on_click(cx.listener(|v, _, _, cx| v.toggle_sidebar(cx))),
                    )
                    .when(show_details, |r| {
                        let details = row()
                            .relative()
                            .flex_1()
                            .min_w_0()
                            .child(div().flex_1().truncate().child(tr(cx, "配置列表")))
                            .child(
                                icon_button(
                                    "sidebar-add-group",
                                    "新建分组",
                                    IconName::FolderPlus,
                                    cx,
                                )
                                .disabled(folded)
                                .on_click(cx.listener(
                                    |v, _, w, cx| {
                                        v.open_sidebar_group_editor(SidebarGroupEdit::Create, w, cx)
                                    },
                                )),
                            )
                            .child(
                                icon_button("sidebar-add-tab", "新建标签", IconName::Plus, cx)
                                    .disabled(folded)
                                    .on_click(cx.listener(|v, _, w, cx| v.new_tab(false, w, cx))),
                            );
                        r.child(details.opacity(progress))
                    }),
            )
            .child(
                frame("sidebar-list-viewport")
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .child(items.child(div().h(px(16.)).flex_shrink_0()))
                    .when(!folded, |viewport| {
                        viewport.child(
                            frame("sidebar-scrollbar-lane")
                                .absolute()
                                // Keep the full thumb outside the overlapping resize hit target.
                                .right(px(SIDEBAR_SCROLL_LANE_RIGHT))
                                .top_0()
                                .bottom_0()
                                .w(px(10.))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|v, _, _, cx| {
                                        // Dragging the thumb also scrolls beneath a stationary pointer.
                                        if v.sidebar_hovered.take().is_some() {
                                            cx.notify();
                                        }
                                    }),
                                )
                                .child(
                                    Scrollbar::vertical(&self.sidebar_scroll)
                                        .id("sidebar-scrollbar")
                                        .mode(ScrollbarMode::Hover)
                                        // Shift only the painted/clickable thumb 3px right while
                                        // keeping its lane clear of the resize hit target.
                                        .styles(|styles| {
                                            styles
                                                .thumb(|thumb| {
                                                    thumb
                                                        .width(px(3.))
                                                        .inset(px(SIDEBAR_SCROLL_THUMB_INSET))
                                                })
                                                .thumb_hover(|thumb| {
                                                    thumb
                                                        .width(px(SIDEBAR_SCROLL_THUMB_MAX_WIDTH))
                                                        .inset(px(SIDEBAR_SCROLL_THUMB_INSET))
                                                })
                                                .thumb_active(|thumb| {
                                                    thumb
                                                        .width(px(SIDEBAR_SCROLL_THUMB_MAX_WIDTH))
                                                        .inset(px(SIDEBAR_SCROLL_THUMB_INSET))
                                                })
                                        })
                                        .viewport_from_layout(),
                                ),
                        )
                    }),
            )
            .when(folded && progress <= 0.015, |d| {
                d.child(
                    row().justify_center().child(
                        icon_button("sidebar-add-tab", "新建标签", IconName::Plus, cx)
                            .on_click(cx.listener(|v, _, w, cx| v.new_tab(false, w, cx))),
                    ),
                )
            })
            .child(
                frame("sidebar-resize-handle")
                    .absolute()
                    .right_0()
                    .top_0()
                    .bottom_0()
                    .w(px(10.))
                    .cursor(gpui::CursorStyle::ResizeLeftRight)
                    .aria_label(tr(cx, "拖动调整配置列表宽度"))
                    .tooltip(|w, cx| {
                        gpui::component::tooltip::Tooltip::new(tr(cx, "拖动调整配置列表宽度"))
                            .build(w, cx)
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |v, event: &gpui::MouseDownEvent, _, cx| {
                            v.resizing = Some(ResizeDrag {
                                kind: ResizeKind::SidebarWidth,
                                start_x: f32::from(event.position.x),
                                start_y: f32::from(event.position.y),
                                start_value: width,
                            });
                            cx.stop_propagation();
                        }),
                    ),
            )
    }

    fn header(&self, width: f32, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let platform_controls = if cfg!(target_os = "macos") { 80. } else { 114. };
        let tabs_width =
            (width - platform_controls - CONTROL * 7. - GAP * 11. - 40. - 56.).max(104.);
        let mut tabs = TabBar::new("command-tabs")
            .with_variant(TabVariant::Pill)
            .with_size(px(CONTROL))
            .selected_index(self.open_position().unwrap_or(0))
            .max_width(px(tabs_width))
            .track_scroll(&self.tab_scroll)
            .menu(false)
            .on_click(cx.listener(|view, position, window, cx| {
                if let Some(id) = view.open_tab_ids.get(*position).copied() {
                    if let Some(index) = view.tabs.iter().position(|tab| tab.id == id) {
                        view.select_tab(index, window, cx);
                    }
                }
            }));
        for tab_id in &self.open_tab_ids {
            let Some((index, tab)) = self
                .tabs
                .iter()
                .enumerate()
                .find(|(_, tab)| tab.id == *tab_id)
            else {
                continue;
            };
            let tab_id = *tab_id;
            tabs = tabs.child(
                Tab::new()
                    .with_variant(TabVariant::Pill)
                    .aria_label(tab.label(cx))
                    // Drop on the entire pill, including the blank padding and close-button side.
                    // Only the label starts a drag, so pressing X still closes the tab.
                    .on_drag_move::<TabDrag>({
                        let owner = cx.entity().downgrade();
                        move |event, _, cx| {
                            let drag = event.drag(cx).clone();
                            // The close suffix adds 20px + 4px margin + 4px Kit gap on
                            // the right. Use the label-area midpoint so dropping at the
                            // text center behaves symmetrically in both directions.
                            let center = f32::from(event.bounds.center().x) - 14.;
                            let pointer = f32::from(event.event.position.x);
                            let inside = event.bounds.contains(&event.event.position);
                            let _ = owner.update(cx, |v, cx| {
                                if !inside {
                                    if v.tab_drag_target.is_some_and(|(area, target, _)| {
                                        area == TabArea::Top && target == tab_id
                                    }) {
                                        v.tab_drag_target = None;
                                        v.tab_drop_x = None;
                                        cx.notify();
                                    }
                                    return;
                                }
                                let target = (drag.area == TabArea::Top && drag.tab_id != tab_id)
                                    .then(|| {
                                        (
                                            TabArea::Top,
                                            tab_id,
                                            v.tab_drop_after(&drag, tab_id, pointer, center),
                                        )
                                    });
                                if let Some((_, _, after)) = target {
                                    v.tab_drag_centers.insert((TabArea::Top, tab_id), center);
                                    let x = f32::from(if after {
                                        event.bounds.right()
                                    } else {
                                        event.bounds.left()
                                    });
                                    if v.tab_drop_x != Some(x) {
                                        v.tab_drop_x = Some(x);
                                        cx.notify();
                                    }
                                } else {
                                    v.tab_drop_x = None;
                                }
                                if v.tab_drag_target != target {
                                    v.tab_drag_target = target;
                                    cx.notify();
                                }
                            });
                        }
                    })
                    .on_drop::<TabDrag>({
                        let owner = cx.entity().downgrade();
                        move |drag, w, cx| {
                            let _ = owner.update(cx, |v, cx| {
                                if drag.area == TabArea::Top {
                                    if let Some(center) =
                                        v.tab_drag_centers.get(&(TabArea::Top, tab_id))
                                    {
                                        let after = v.tab_drop_after(
                                            drag,
                                            tab_id,
                                            f32::from(w.mouse_position().x),
                                            *center,
                                        );
                                        v.reorder_tabs(drag, tab_id, after, cx);
                                    }
                                }
                            });
                        }
                    })
                    .child(
                        frame(("tab-context", tab_id))
                            .flex()
                            .items_center()
                            .gap(px(GAP))
                            .pl(px(GAP))
                            .on_drag(
                                TabDrag {
                                    tab_id,
                                    area: TabArea::Top,
                                },
                                {
                                    let label = tab.label(cx);
                                    move |_, _, _, cx| {
                                        cx.new(|_| TopTabDragPreview {
                                            label: label.clone(),
                                        })
                                    }
                                },
                            )
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(move |v, event: &gpui::MouseDownEvent, w, cx| {
                                    cx.stop_propagation();
                                    let x = (f32::from(event.position.x))
                                        .min(f32::from(w.viewport_size().width) - 208.)
                                        .max(8.);
                                    let y = (f32::from(event.position.y) + 8.)
                                        .min(f32::from(w.viewport_size().height) - 88.)
                                        .max(8.);
                                    v.tab_group_options_open = false;
                                    v.tab_menu = Some((tab_id, x, y));
                                    cx.notify();
                                }),
                            )
                            .when(tab.dirty, |d| {
                                // Pill 选中态以 primary 填充：状态点必须用其前景色，
                                // 不能复用 focus（与选中背景相同，会完全消失）。
                                d.child(
                                    frame(("dirty-marker", tab_id))
                                        .w(px(18.))
                                        .h(px(20.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(
                                            frame(("dirty-dot", tab_id))
                                                .size(px(6.))
                                                .rounded_full()
                                                .bg(rgb(if index == self.active {
                                                    p.on_primary
                                                } else {
                                                    p.focus
                                                })),
                                        ),
                                )
                            })
                            .child(tab.label(cx)),
                    )
                    .suffix(
                        icon_button(("close-tab", tab_id), "关闭标签", IconName::X, cx)
                            .with_size(Size::Size(px(20.)))
                            .size(px(20.)) // 覆盖 icon_button 的 32px 实例样式，真实点击区域才是 20px
                            .rounded(px(10.)) // 20px 点击区的半径，悬停高光为正圆
                            .mr(px(4.))
                            .custom(
                                ButtonCustomVariant::new(cx)
                                    .foreground(
                                        rgb(if index == self.active {
                                            p.on_primary
                                        } else {
                                            p.muted
                                        })
                                        .into(),
                                    )
                                    .hover(if index == self.active {
                                        gpui::Hsla::from(rgb(p.on_primary)).opacity(0.18)
                                    } else {
                                        rgb(p.hover).into()
                                    })
                                    .active(if index == self.active {
                                        gpui::Hsla::from(rgb(p.on_primary)).opacity(0.28)
                                    } else {
                                        rgb(p.pressed).into()
                                    }),
                            )
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.request_close(index, window, cx)
                            })),
                    ),
            );
        }
        let bar = row()
            .when(!cfg!(target_os = "windows"), |bar| bar.w_full())
            .when(cfg!(target_os = "windows"), |bar| bar.flex_1().min_w_0())
            .h_full()
            .gap(px(GAP))
            .pr(px(GAP))
            .child(
                frame("app-brand")
                    .w(px(56.))
                    .h(px(CONTROL))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(GAP))
                    .child(
                        Icon::new(IconName::Terminal)
                            .size(px(ICON))
                            .text_color(rgb(p.focus)),
                    )
                    .child(div().text_size(px(SMALL)).font_semibold().child("ECR")),
            )
            .child(self.menu_button(cx))
            .child(
                icon_button("previous-tab", "上一个标签", IconName::ChevronLeft, cx)
                    .rounded(px(RADIUS))
                    .disabled(self.open_position().is_none_or(|position| position == 0))
                    .on_click(cx.listener(|v, _, w, cx| {
                        if let Some(position) = v
                            .open_position()
                            .and_then(|position| position.checked_sub(1))
                        {
                            let id = v.open_tab_ids[position];
                            if let Some(index) = v.tabs.iter().position(|tab| tab.id == id) {
                                v.select_tab(index, w, cx);
                            }
                        }
                    })),
            )
            .child(
                frame("tab-strip")
                    .min_w_0()
                    .max_w(px(tabs_width))
                    .rounded_full()
                    .bg(rgb(p.button))
                    .child(tabs),
            )
            .child(
                icon_button("add-tab-title", "新建标签", IconName::Plus, cx)
                    .on_click(cx.listener(|v, _, w, cx| v.new_tab(false, w, cx))),
            )
            .child(
                icon_button("next-tab", "下一个标签", IconName::ChevronRight, cx)
                    .rounded(px(RADIUS))
                    .disabled(
                        self.open_position()
                            .is_none_or(|position| position + 1 >= self.open_tab_ids.len()),
                    )
                    .on_click(cx.listener(|v, _, w, cx| {
                        if let Some(position) = v
                            .open_position()
                            .filter(|position| position + 1 < v.open_tab_ids.len())
                        {
                            let id = v.open_tab_ids[position + 1];
                            if let Some(index) = v.tabs.iter().position(|tab| tab.id == id) {
                                v.select_tab(index, w, cx);
                            }
                        }
                    })),
            )
            .child(
                frame("title-drag-space")
                    .flex_1()
                    .min_w(px(40.))
                    .h_full()
                    .window_control_area(WindowControlArea::Drag),
            )
            .child(
                icon_button(
                    "theme-toggle",
                    "切换深色 / 浅色主题",
                    if *cx.global::<Theme>() == Theme::Dark {
                        IconName::Sun
                    } else {
                        IconName::Moon
                    },
                    cx,
                )
                .on_click(cx.listener(|v, _, window, cx| {
                    v.set_appearance(cx.global::<Theme>().toggle(), window, cx);
                })),
            )
            .child(
                icon_button("save", "保存配置", IconName::Save, cx)
                    .disabled(!self.config_writable || self.store.is_none())
                    .on_click(cx.listener(|v, _, _, cx| {
                        if let Err(error) = v.save_configuration(cx) {
                            v.status = error;
                            cx.notify();
                        }
                    })),
            )
            .child(
                icon_button("settings", "工作区设置", IconName::Settings2, cx)
                    .on_click(cx.listener(|v, _, w, cx| v.open_settings(w, cx))),
            );
        if cfg!(target_os = "windows") {
            // Kit TitleBar marks its entire bar as HTCAPTION on Windows, including
            // children. Native hit testing then swallows theme/save/settings clicks.
            // Only our deliberately empty gap may drag the window.
            let control = |id, icon, area, color| {
                frame(id)
                    .w(px(34.))
                    .h_full()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .window_control_area(area)
                    .hover(|d| d.bg(rgb(color)))
                    .child(Icon::new(icon).size(px(ICON)).text_color(rgb(p.text)))
            };
            let supported = window.window_controls();
            frame("windows-title-bar")
                .flex()
                .items_center()
                .h(px(TITLE_HEIGHT))
                .pl(px(12.))
                .bg(rgb(p.app))
                .border_b_1()
                .border_color(rgb(p.divider))
                .child(bar)
                .child(
                    frame("windows-controls")
                        .flex()
                        .h_full()
                        .flex_shrink_0()
                        .when(supported.minimize, |d| {
                            d.child(control(
                                "windows-minimize",
                                IconName::WindowMinimize,
                                WindowControlArea::Min,
                                p.hover,
                            ))
                        })
                        .when(supported.maximize, |d| {
                            d.child(control(
                                "windows-maximize",
                                if window.is_maximized() {
                                    IconName::WindowRestore
                                } else {
                                    IconName::WindowMaximize
                                },
                                WindowControlArea::Max,
                                p.hover,
                            ))
                        })
                        .child(control(
                            "windows-close",
                            IconName::WindowClose,
                            WindowControlArea::Close,
                            p.danger_hover,
                        )),
                )
                .into_any_element()
        } else {
            let close_to_tray = self.close_to_tray.clone();
            TitleBar::new()
                // Linux 客户端装饰的 X 按钮直接调用 remove_window，绕过窗口管理器的
                // should_close 回调；显式接到同一条关闭路径，防止留下无窗口的托盘进程。
                .on_close_window(move |_, window, cx| {
                    if close_to_tray.as_ref().is_some_and(|active| active.get()) {
                        tray::hide_to_tray(window, cx);
                    } else if tray::can_close_without_tray(window, cx) {
                        window.remove_window();
                    }
                })
                .h(px(TITLE_HEIGHT))
                .bg(rgb(p.app))
                .child(bar)
                .into_any_element()
        }
    }

    fn resize_grip(
        &self,
        id: &'static str,
        kind: ResizeKind,
        start_value: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        frame(id)
            .absolute()
            .left_0()
            .right_0()
            // Rows: center the handle in the 8px gap below the list, off the last row.
            // Log: inset from its top divider; don't crowd the header controls.
            .when(matches!(kind, ResizeKind::Rows), |d| d.bottom(px(-4.)))
            .when(!matches!(kind, ResizeKind::Rows), |d| {
                d.top(px(if start_value >= 48. { 3. } else { 0. }))
            })
            .h(px(8.))
            .cursor(gpui::CursorStyle::ResizeUpDown)
            .flex()
            .justify_center()
            .items_center()
            .child(
                div()
                    .w(px(40.))
                    .h(px(2.))
                    .rounded_full()
                    .bg(rgb(palette(cx).muted)),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |v, event: &gpui::MouseDownEvent, _, cx| {
                    v.resizing = Some(ResizeDrag {
                        kind,
                        start_x: f32::from(event.position.x),
                        start_y: f32::from(event.position.y),
                        start_value,
                    });
                    cx.stop_propagation();
                }),
            )
    }

    fn parameter_table(
        &self,
        width: f32,
        height: f32,
        compact: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // 表头与参数行共用 8px 内网格，背景分层不再叠加边框和魔数补偿。
        let compact_actions = width < 850.
            && (theme::font_size(cx) > 16. || *cx.global::<Language>() == Language::English);
        let grid = ParameterGrid::new((width - TABLE_INSET * 2.).max(0.), compact);
        let p = palette(cx);
        let header_cell = |name: &'static str, text: &str, width| {
            frame(name)
                .w(px(width))
                .flex_shrink_0()
                .px(px(FIELD_PADDING))
                .child(tr(cx, text))
        };
        let header = frame("parameter-header")
            .flex()
            .items_center()
            .gap(px(INPUT_GAP))
            .px(px(TABLE_INSET))
            .h(px(HEADER_HEIGHT))
            .flex_shrink_0()
            .text_size(px(SMALL))
            .text_color(rgb(p.muted))
            .child(div().w(px(GRIP)).flex_shrink_0())
            .child(div().w(px(CONTROL)).flex_shrink_0())
            .child(
                frame("option-heading")
                    .w(px(grid.option))
                    .flex_shrink_0()
                    .px(px(FIELD_PADDING))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .cursor(gpui::CursorStyle::PointingHand)
                    .aria_label(tr(cx, "按选项 / 功能字母排序"))
                    .tooltip(|w, cx| {
                        gpui::component::tooltip::Tooltip::new(tr(
                            cx,
                            "点击按字母顺序排序；勾选参数置顶时已勾选参数先排序",
                        ))
                        .build(w, cx)
                    })
                    .on_click(cx.listener(|v, _, w, cx| v.sort_parameters_by_option(w, cx)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(tr(cx, "选项 / 功能")),
                    )
                    .child(
                        Icon::new(IconName::ArrowDownAZ)
                            .size(px(14.))
                            .text_color(rgb(p.muted)),
                    ),
            )
            .child(header_cell("value-heading", "参数值", grid.value))
            .child(header_cell(
                "note-heading",
                if compact { "" } else { "备注" },
                grid.note.unwrap_or(CONTROL),
            ))
            .child(div().w(px(CONTROL)).flex_shrink_0());
        let mut rows = frame("parameter-items")
            .flex()
            .flex_col()
            .gap(px(GAP))
            .h(px(height - TABLE_INSET * 2.))
            .min_h(px(CONTROL))
            .flex_shrink_0()
            .overflow_y_scroll()
            .track_scroll(&self.row_scroll)
            // 整个滚动视口接收拖放，行间的 8px 空隙、上下边界也可放置。
            .on_drag_move::<RowDrag>({
                let view = cx.entity().downgrade();
                move |ev, w, cx| {
                    let drag = *ev.drag(cx);
                    let bounds = ev.bounds;
                    let pointer = ev.event.position;
                    let _ = view.update(cx, |v, cx| {
                        if v.current().id != drag.tab_id {
                            v.stop_row_drag_auto_scroll();
                            return;
                        }
                        let edge = f32::from(pointer.y - bounds.top());
                        let inside_x = pointer.x >= bounds.left() && pointer.x < bounds.right();
                        let direction = if inside_x && edge < 14. {
                            Some(1.)
                        } else if inside_x && edge > f32::from(bounds.size.height) - 14. {
                            Some(-1.)
                        } else {
                            None
                        };
                        if let Some(direction) = direction {
                            v.row_drag_auto_scroll = Some(RowDragAutoScroll {
                                drag,
                                pointer,
                                direction,
                            });
                            let advanced = v.advance_row_drag_auto_scroll(w, cx);
                            if advanced && !v.row_drag_auto_scroll_running {
                                v.row_drag_auto_scroll_running = true;
                                let generation = v.row_drag_auto_scroll_generation.wrapping_add(1);
                                v.row_drag_auto_scroll_generation = generation;
                                cx.spawn_in(w, async move |owner, window| loop {
                                    window
                                        .background_executor()
                                        .timer(std::time::Duration::from_millis(30))
                                        .await;
                                    let keep = owner
                                        .update_in(window, |v, w, cx| {
                                            if v.row_drag_auto_scroll_generation != generation {
                                                return false;
                                            }
                                            v.advance_row_drag_auto_scroll(w, cx)
                                        })
                                        .unwrap_or(false);
                                    if !keep {
                                        break;
                                    }
                                })
                                .detach();
                            }
                        } else {
                            v.stop_row_drag_auto_scroll();
                            let y = f32::from(pointer.y - bounds.top() - v.row_scroll.offset().y);
                            let next = if inside_x {
                                row_drop_target(&v.current().rows, y, drag.row_id, v.enabled_first)
                            } else {
                                None
                            };
                            if v.drag_target != next {
                                v.drag_target = next;
                                cx.notify();
                            }
                        }
                    });
                }
            })
            .on_drop::<RowDrag>({
                let view = cx.entity().downgrade();
                move |drag, w, cx| {
                    let _ = view.update(cx, |v, cx| {
                        v.stop_row_drag_auto_scroll();
                        if let Some((target, after)) = v.drag_target {
                            v.reorder_row(drag, target, after, w, cx);
                        } else {
                            v.drag_target = None;
                            cx.notify();
                        }
                    });
                }
            })
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|v, _, _, cx| {
                    v.stop_row_drag_auto_scroll();
                    if v.drag_target.take().is_some() {
                        cx.notify();
                    }
                }),
            );
        for (index, parameter) in self.current().rows.iter().enumerate() {
            let id = parameter.id;
            let tab_id = self.current().id;
            let drag = RowDrag { tab_id, row_id: id };
            let indicator = self
                .drag_target
                .filter(|(target, _)| *target == id)
                .map(|(_, after)| after);
            let mut r = frame(("parameter-row", id))
                .relative()
                .flex()
                .items_center()
                .gap(px(INPUT_GAP))
                .h(px(CONTROL))
                .flex_shrink_0()
                .when_some(indicator, |row, after| {
                    row.child(
                        frame(("insert-indicator", id))
                            .absolute()
                            .left_0()
                            .right_0()
                            .h(px(2.))
                            .when(after, |line| line.bottom_0())
                            .when(!after, |line| line.top_0())
                            .bg(rgb(p.focus)),
                    )
                })
                .child(
                    frame(("move-row", id))
                        .size(px(GRIP))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_grab()
                        .on_drag(drag, {
                            let view = cx.entity().downgrade();
                            let option = parameter.option.clone();
                            move |_, _, _, cx| {
                                let _ = view.update(cx, |v, cx| {
                                    v.drag_target = None;
                                    cx.notify();
                                });
                                let label = option.read(cx).value().to_string();
                                cx.new(|_| RowDragPreview { label })
                            }
                        })
                        .child(
                            Icon::new(IconName::GripVertical)
                                .size(px(ICON))
                                .text_color(rgb(p.muted)),
                        ),
                )
                .child(
                    icon_button(
                        ("toggle-row", id),
                        "启用或禁用参数",
                        if parameter.enabled {
                            IconName::Check
                        } else {
                            IconName::Minus
                        },
                        cx,
                    )
                    .toggled(parameter.enabled)
                    .on_click(cx.listener(move |v, _, w, cx| v.toggle_row(id, w, cx))),
                )
                .child(
                    frame(("option-cell", id))
                        .w(px(grid.option))
                        .flex_shrink_0()
                        .when(!parameter.enabled, |d| d.opacity(0.55))
                        .child(input(
                            ("option", id),
                            &parameter.option,
                            "选项 / 功能",
                            true,
                            cx,
                        )),
                )
                .child(
                    frame(("value-cell", id))
                        .w(px(grid.value))
                        .flex_shrink_0()
                        .when(!parameter.enabled, |d| d.opacity(0.55))
                        .child(input(("value", id), &parameter.value, "参数值", true, cx)),
                );
            r = if let Some(width) = grid.note {
                r.child(
                    frame(("note-cell", id))
                        .w(px(width))
                        .flex_shrink_0()
                        .child(input(("note", id), &parameter.note, "备注", false, cx)),
                )
            } else {
                let note = parameter.note.clone();
                let contents = note.read(cx).value().to_string();
                let hover_text = if contents.trim().is_empty() {
                    tr(cx, "编辑此参数的备注")
                } else {
                    contents
                };
                r.child(
                    icon_button(
                        ("note-details", id),
                        "编辑此参数的备注",
                        IconName::MessageSquare,
                        cx,
                    )
                    // Hover reveals the note itself; clicking still opens the editor.
                    // Keep the same text in the accessible label for keyboard/screen readers.
                    .tooltip(hover_text.clone())
                    .accessibility_label(hover_text)
                    .on_click(move |_, window, cx| {
                        let note = note.clone();
                        window.open_dialog(cx, move |dialog, _, cx| {
                            dialog.title(tr(cx, "参数备注")).child(input(
                                "note-detail-input",
                                &note,
                                "备注",
                                false,
                                cx,
                            ))
                        });
                    }),
                )
            };
            rows = rows.child(r.child(delete_button(("remove-row", id), cx).on_click(
                cx.listener(move |v, _, w, cx| {
                    v.tabs[v.active].rows.remove(index);
                    v.changed(v.current().id, w, cx);
                }),
            )));
        }
        if self.current().rows.is_empty() {
            rows = rows.child(
                div()
                    .flex()
                    .items_center()
                    .h(px(CONTROL))
                    .px(px(GAP))
                    .text_color(rgb(p.muted))
                    .child(tr(cx, "暂无参数，点击下方添加。")),
            );
        }
        column()
            .gap(px(GAP))
            .flex_shrink_0()
            .child(
                column().gap(px(HEADER_GAP)).child(header).child(
                    frame("parameter-list")
                        .relative()
                        .flex()
                        .flex_col()
                        .h(px(height))
                        .min_h(px(CONTROL))
                        .flex_shrink_0()
                        .p(px(TABLE_INSET))
                        .rounded(px(RADIUS))
                        .bg(rgb(p.subtle))
                        .child(rows)
                        .when(self.current().rows.len() > 3, |d| {
                            d.child(self.resize_grip(
                                "parameter-resize",
                                ResizeKind::Rows,
                                height,
                                cx,
                            ))
                        }),
                ),
            )
            .child(
                frame("parameter-actions")
                    .flex()
                    .items_center()
                    .gap(px(GAP))
                    .h(px(CONTROL))
                    .child(
                        action_button(
                            "add-parameter",
                            "添加参数",
                            IconName::Plus,
                            compact_actions,
                            cx,
                        )
                        .on_click(cx.listener(|v, _, w, cx| v.add_row(w, cx))),
                    )
                    .child(
                        button("enabled-first", "勾选参数置顶", None, cx)
                            // 六个汉字按当前界面字号计宽，额外空间留给边框和左右内边距。
                            .w(px(theme::font_size(cx) * 6. + BUTTON_PADDING * 2. + 4.))
                            .custom(
                                ButtonCustomVariant::new(cx)
                                    .color(
                                        rgb(if self.enabled_first {
                                            p.primary
                                        } else {
                                            p.button
                                        })
                                        .into(),
                                    )
                                    .foreground(
                                        rgb(if self.enabled_first {
                                            p.on_primary
                                        } else {
                                            p.text
                                        })
                                        .into(),
                                    )
                                    .hover(
                                        rgb(if self.enabled_first {
                                            p.primary_hover
                                        } else {
                                            p.hover
                                        })
                                        .into(),
                                    )
                                    .active(
                                        rgb(if self.enabled_first {
                                            p.primary
                                        } else {
                                            p.pressed
                                        })
                                        .into(),
                                    ),
                            )
                            .selected(self.enabled_first)
                            .toggled(self.enabled_first)
                            .tooltip(tr(
                                cx,
                                "开启后，此命令中已勾选的参数排在未勾选参数前；只能在同组内拖动",
                            ))
                            .on_click(cx.listener(|v, _, w, cx| v.toggle_enabled_first(w, cx))),
                    )
                    .child(div().flex_1())
                    .child(
                        action_button(
                            "select-all",
                            "全选",
                            IconName::CheckCheck,
                            compact_actions,
                            cx,
                        )
                        .ghost()
                        .on_click(cx.listener(|v, _, w, cx| v.set_all(true, w, cx))),
                    )
                    .child(
                        action_button(
                            "select-none",
                            "全不选",
                            IconName::Square,
                            compact_actions,
                            cx,
                        )
                        .ghost()
                        .on_click(cx.listener(|v, _, w, cx| v.set_all(false, w, cx))),
                    )
                    .child(
                        div()
                            .text_size(px(SMALL))
                            .text_color(rgb(p.muted))
                            .child(i18n::format(
                                cx,
                                "{} / {} 已启用",
                                &[
                                    &self
                                        .current()
                                        .rows
                                        .iter()
                                        .filter(|r| r.enabled)
                                        .count()
                                        .to_string(),
                                    &self.current().rows.len().to_string(),
                                ],
                            )),
                    ),
            )
    }
}

#[derive(Clone, Copy)]
enum ResizeKind {
    Rows,
    Log,
    DescriptionWidth,
    SidebarWidth,
}
#[derive(Clone, Copy)]
struct ResizeDrag {
    kind: ResizeKind,
    start_x: f32,
    start_y: f32,
    start_value: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum TabArea {
    Sidebar,
    Top,
}

#[derive(Clone)]
struct TabDrag {
    tab_id: usize,
    area: TabArea,
}

#[derive(Clone, Copy)]
struct RowDrag {
    tab_id: usize,
    row_id: usize,
}

#[derive(Clone, Copy)]
struct RowDragAutoScroll {
    drag: RowDrag,
    pointer: gpui::Point<gpui::Pixels>,
    direction: f32,
}

/// 使用 32px 行高和 8px 行距计算落点；源行及启用优先排序组外不显示错误插入线。
fn row_drop_target(
    rows: &[Parameter],
    y: f32,
    source_id: usize,
    enabled_first: bool,
) -> Option<(usize, bool)> {
    if rows.is_empty() {
        return None;
    }
    let index = (y.max(0.) / (CONTROL + GAP)).floor() as usize;
    let index = index.min(rows.len() - 1);
    let after = y >= (index as f32 * (CONTROL + GAP) + CONTROL / 2.);
    let source = rows.iter().position(|r| r.id == source_id)?;
    if enabled_first && rows[source].enabled != rows[index].enabled {
        return None;
    }
    let destination = index + usize::from(after);
    if destination == source || destination == source + 1 {
        return None;
    }
    Some((rows[index].id, after))
}

pub(crate) struct TopTabDragPreview {
    pub(crate) label: String,
}
impl Render for TopTabDragPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        frame("top-tab-drag-preview")
            .flex()
            .items_center()
            .h(px(CONTROL))
            .max_w(px(240.))
            .px(px(12.))
            .rounded_full()
            .bg(rgb(p.selected_tab))
            .text_color(rgb(p.on_primary))
            .shadow_sm()
            .child(div().min_w_0().truncate().child(self.label.clone()))
    }
}

pub(crate) struct RowDragPreview {
    pub(crate) label: String,
}
impl Render for RowDragPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        frame("parameter-drag-preview")
            .flex()
            .items_center()
            .gap(px(GAP))
            .p(px(GAP))
            .rounded(px(RADIUS))
            .bg(rgb(p.button))
            .text_color(rgb(p.text))
            .child(
                Icon::new(IconName::GripVertical)
                    .size(px(ICON))
                    .text_color(rgb(p.text)),
            )
            .child(if self.label.is_empty() {
                tr(cx, "未命名参数")
            } else {
                self.label.clone()
            })
    }
}

impl Render for CommandWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Win32 WM_MOUSELEAVE updates the platform hover flag but does not emit
        // GPUI MouseExitEvent. A platform refresh is enough to dismiss the preview.
        // The headless Windows test window does not simulate this flag.
        #[cfg(all(windows, not(test), not(feature = "ui-test")))]
        if !window.is_window_hovered() {
            self.sidebar_auto_expanded = false;
            self.sidebar_auto_suppressed = false;
            self.sidebar_hovered = None;
        }
        let p = palette(cx);
        let exit_owner = cx.entity().downgrade();
        let width = f32::from(window.viewport_size().width);
        let expanded_width = self.expanded_sidebar_width(width);
        let target_sidebar_width = self.sidebar_width(width);
        // GPUI carries sidebar spring velocity across rapid reversals; the live page
        // keeps its input entities throughout both sidebar resizing and page transitions.
        let sidebar_width = gpui::base::spring(
            "sidebar-boundary-spring",
            target_sidebar_width,
            gpui::base::Spring::new(std::time::Duration::from_millis(210))
                .with_damping(0.74)
                .with_epsilon(0.35)
                .with_travel(
                    !self.reduced_motion
                        && !matches!(
                            self.resizing,
                            Some(ResizeDrag {
                                kind: ResizeKind::SidebarWidth,
                                ..
                            })
                        ),
                ),
            window,
            cx,
        )
        .clamp(52., expanded_width + 10.);
        let overlay_width = gpui::base::spring(
            "sidebar-hover-overlay-spring",
            if self.sidebar_auto_expanded {
                expanded_width
            } else {
                56.
            },
            gpui::base::Spring::new(std::time::Duration::from_millis(210))
                .with_damping(0.74)
                .with_epsilon(0.35)
                .with_travel(!self.reduced_motion),
            window,
            cx,
        )
        .clamp(52., expanded_width + 10.);
        let overlay_visible =
            self.sidebar_collapsed && (self.sidebar_auto_expanded || overlay_width > 56.35);
        self.sidebar_display_width = sidebar_width;
        self.sidebar_overlay_display_width = overlay_width;
        let content_width = (width - sidebar_width).max(0.);
        let height = f32::from(window.viewport_size().height);
        // Keep the input grid stable while the sidebar is dragged or its spring moves.
        // Only a window resize may switch between note-column and note-button modes.
        let compact = width - Self::default_sidebar_width(width) < 760.;
        let narrow_content = width - expanded_width;
        let compact_actions = narrow_content < 700.
            || (narrow_content < 950.
                && (theme::font_size(cx) > 16. || *cx.global::<Language>() == Language::English));
        let layout = WorkspaceLayout::with_overrides(
            height,
            self.current().rows.len(),
            self.show_log,
            self.current().row_height_override,
            self.log_height_override,
        );
        let log_height = layout.log_height;
        let preview_header_total = PREVIEW_HEADER_HEIGHT + GAP * 2. + CONTROL;
        let slide_layers = self.page_slide_layers(window, cx);
        let tab = self.current();
        let form = frame("command-form")
            .relative()
            .flex()
            .flex_col()
            .gap(px(GAP))
            .flex_shrink_0()
            .child(heading_input(&tab.name, cx))
            .child(
                column()
                    .gap(px(GAP))
                    .child(field(
                        "工作目录",
                        row()
                            .child(div().flex_1().min_w_0().child(input(
                                "working-directory",
                                &tab.directory,
                                "工作目录",
                                true,
                                cx,
                            )))
                            .child(
                                icon_button(
                                    "browse-directory",
                                    "选择工作目录",
                                    IconName::FolderOpen,
                                    cx,
                                )
                                .on_click(cx.listener(
                                    |v, _, window, cx| v.prompt_working_directory(window, cx),
                                )),
                            ),
                        cx,
                    ))
                    .child(field(
                        "程序",
                        row()
                            .child(div().flex_1().min_w_0().child(input(
                                "program",
                                &tab.program,
                                "程序",
                                true,
                                cx,
                            )))
                            .child(
                                button("parse-command", "解析", Some(IconName::ScanText), cx)
                                    .on_click(cx.listener(|v, _, w, cx| v.parse(false, w, cx))),
                            ),
                        cx,
                    )),
            )
            // Reuse the existing section gap so the hierarchy doesn't squeeze the log/preview
            // out of a small window. The divider is decoration, not another layout row.
            .child(
                frame("command-form-divider")
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom(px(-GAP - 1.))
                    .h(px(1.))
                    .bg(rgb(p.divider)),
            );
        let extras_width = (content_width - PAGE_INSET * 2.).max(0.);
        // Equal left half for executed arguments; the action stays fixed as the paste field grows.
        const PARSE_BUTTON_WIDTH: f32 = 104.;
        let extras = frame("command-extras")
            .flex()
            .items_center()
            .gap(px(GAP))
            .h(px(EXTRAS_HEIGHT))
            .flex_shrink_0()
            .child(
                frame("other-args-cell")
                    .w(px(extras_width * 0.5))
                    .flex_shrink_0()
                    .child(input(
                        "other-args",
                        &tab.other,
                        "其他参数 / 重定向 / 管道（按原文追加）",
                        true,
                        cx,
                    )),
            )
            .child(
                frame("append-fragment")
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
                    .bg(gpui::Hsla::from(rgb(p.focus)).opacity(0.10))
                    .rounded(px(RADIUS))
                    .child(
                        frame("append-input-cell")
                            .flex_1()
                            .min_w_0()
                            // Scope paste guidance to the input; the button has its own tooltip.
                            .tooltip(|w, cx| {
                                gpui::component::tooltip::Tooltip::new(tr(
                                    cx,
                                    "粘贴内容只用于追加解析；原文不会直接参与运行",
                                ))
                                .build(w, cx)
                            })
                            .child(input(
                                "append-command",
                                &tab.append,
                                "仅解析 · 粘贴命令",
                                true,
                                cx,
                            )),
                    )
                    .child(
                        button("append-parse", "追加解析", None, cx)
                            .w(px(PARSE_BUTTON_WIDTH))
                            .px(px(4.))
                            .justify_center()
                            .tooltip(tr(cx, "粘贴内容只用于追加解析；原文不会直接参与运行"))
                            .on_click(cx.listener(|v, _, w, cx| v.parse(true, w, cx))),
                    )
                    .child(
                        frame("append-only-hint")
                            .absolute()
                            .right_0()
                            .bottom(px(-15.))
                            .h(px(13.))
                            .text_size(px(10.))
                            .text_color(rgb(p.focus))
                            .child(tr(cx, "仅用于追加解析 · 不直接运行")),
                    ),
            );
        let split_available = (content_width - PAGE_INSET * 2. - GAP).max(0.);
        let description_width = self
            .description_width_override
            .unwrap_or(split_available * 0.40)
            .clamp(120., (split_available - PREVIEW_MIN).max(120.));
        let preview_section = frame("preview-section")
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(preview_header_total + PREVIEW_MIN))
            .gap(px(GAP))
            .child(
                row()
                    .h(px(PREVIEW_HEADER_HEIGHT))
                    .flex_shrink_0()
                    .child(
                        frame("preview-header")
                            .relative()
                            .flex()
                            .items_center()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .h(px(PREVIEW_HEADER_HEIGHT))
                            .text_size(px(SMALL))
                            .text_color(rgb(p.muted))
                            .aria_label(tr(cx, "命令预览·实时更新"))
                            .child(tr(cx, "命令预览·实时更新")),
                    )
                    .child(
                        frame("description-header")
                            .w(px(description_width))
                            .flex_shrink_0()
                            .truncate()
                            .text_size(px(SMALL))
                            .text_color(rgb(p.muted))
                            .aria_label(tr(cx, "描述 / 使用说明"))
                            .child(tr(cx, "描述 / 使用说明")),
                    ),
            )
            .child(
                row()
                    .gap(px(0.))
                    .flex_1()
                    .min_h(px(PREVIEW_MIN))
                    .child(
                        frame("preview-frame").flex_1().min_w_0().h_full().child(
                            textarea(&self.preview, "最终命令预览", true, cx)
                                .readonly(true)
                                .bg(rgb(p.subtle)),
                        ),
                    )
                    .child(
                        frame("preview-splitter")
                            .w(px(GAP))
                            .h_full()
                            .flex_shrink_0()
                            .flex()
                            .justify_center()
                            .cursor(gpui::CursorStyle::ResizeLeftRight)
                            .child(div().w(px(2.)).h_full().bg(rgb(p.divider)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |v, event: &gpui::MouseDownEvent, _, cx| {
                                    v.resizing = Some(ResizeDrag {
                                        kind: ResizeKind::DescriptionWidth,
                                        start_x: f32::from(event.position.x),
                                        start_y: f32::from(event.position.y),
                                        start_value: description_width,
                                    });
                                    cx.stop_propagation();
                                }),
                            ),
                    )
                    .child(
                        frame("description-frame")
                            .w(px(description_width))
                            .flex_shrink_0()
                            .h_full()
                            .child(textarea(
                                &self.current().description,
                                "描述 / 使用说明",
                                false,
                                cx,
                            )),
                    ),
            )
            .child(
                frame("run-actions")
                    .flex()
                    .items_center()
                    .gap(px(GAP))
                    .h(px(CONTROL))
                    .flex_shrink_0()
                    .child(
                        action_button(
                            "toggle-log",
                            "运行日志",
                            IconName::Terminal,
                            compact_actions,
                            cx,
                        )
                        .ghost()
                        .on_click(cx.listener(|v, _, _, cx| v.toggle_log(cx))),
                    )
                    .child(div().flex_1())
                    .child(
                        action_button(
                            "reload",
                            "重新加载配置",
                            IconName::RefreshCw,
                            compact_actions,
                            cx,
                        )
                        .ghost()
                        .disabled(self.store.is_none())
                        .tooltip(tr(cx, "从磁盘重新加载配置"))
                        .on_click(cx.listener(|v, _, w, cx| v.request_reload(w, cx))),
                    )
                    .child(
                        action_button(
                            "copy-command",
                            "复制命令",
                            IconName::Copy,
                            compact_actions,
                            cx,
                        )
                        .on_click(cx.listener(|v, _, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(
                                v.current().command(cx),
                            ));
                            v.status = tr(cx, "命令已复制到剪贴板");
                            cx.notify();
                        })),
                    )
                    .child(
                        button("run-command", "运行", Some(IconName::Play), cx)
                            .primary()
                            .disabled(self.current().command(cx).trim().is_empty())
                            .tooltip(tr(cx, "通过系统 Shell 执行预览中的命令"))
                            .on_click(cx.listener(|v, _, w, cx| v.run_current_command(w, cx))),
                    ),
            );
        let page = frame("page-switch-content")
            .relative()
            .flex()
            .flex_col()
            .w_full()
            .min_w_0()
            .flex_1()
            .min_h_0()
            .px(px(PAGE_INSET))
            .py(px(SECTION))
            .gap(px(SECTION))
            .child(form)
            .child(
                column()
                    .flex_shrink_0()
                    .gap(px(GAP))
                    .child(self.parameter_table(
                        content_width - PAGE_INSET * 2.,
                        layout.rows_height,
                        compact,
                        cx,
                    ))
                    .child(extras),
            )
            .child(
                frame("preview-region")
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(preview_section),
            );
        // Render the live editor throughout the transition so focus, selection, input
        // handlers and IME remain bound to its original entities. Only the plain-text
        // visual sheets slide; interacting with the editor reveals it immediately.
        let editor = frame("command-editor")
            .relative()
            .flex()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .capture_any_mouse_down(cx.listener(|view, _, _, cx| {
                if view.page_switch.is_some() {
                    view.finish_page_switch();
                    cx.notify();
                }
            }))
            .child(page.opacity(if slide_layers.is_some() { 0. } else { 1. }));
        // Sheets fill the editor viewport; titlebar, sidebar, status and log do not slide.
        let editor = if self.open_tab_ids.is_empty() {
            frame("empty-editor")
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(p.muted))
                .child(tr(cx, "从左侧配置列表打开一个标签，或新建配置"))
                .into_any_element()
        } else if let Some((outgoing, incoming, progress, direction)) = slide_layers {
            let (old_x, new_x) = page_transition::offsets(content_width, direction, progress);
            let (old_alpha, new_alpha) = page_transition::opacities(progress);
            editor
                .child(
                    frame("page-slide-outgoing")
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(px(old_x))
                        .w(px(content_width))
                        .opacity(old_alpha)
                        .overflow_hidden()
                        .bg(rgb(p.panel))
                        .child(outgoing.render(content_width, compact, compact_actions, cx)),
                )
                .child(
                    frame("page-slide-incoming")
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(px(new_x))
                        .w(px(content_width))
                        .opacity(new_alpha)
                        .overflow_hidden()
                        .bg(rgb(p.panel))
                        .child(incoming.render(content_width, compact, compact_actions, cx)),
                )
                .into_any_element()
        } else {
            editor.into_any_element()
        };
        let log = frame("log-dock")
            .relative()
            .flex()
            .flex_col()
            .h(px(log_height))
            .flex_shrink_0()
            .bg(rgb(p.app))
            .px(px(SECTION))
            .when(log_height >= 48., |d| d.py(px(GAP)).pt(px(12.)))
            .when(log_height > 32. && log_height < 48., |d| d.py(px(4.)))
            .gap(px(GAP))
            // 装饰线不占布局高度：32px 的紧凑日志标题栏也能完整容纳 32px 控件。
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(px(1.))
                    .bg(rgb(p.divider)),
            )
            .child(self.resize_grip("log-resize", ResizeKind::Log, log_height, cx))
            .child(
                row()
                    .h(px(CONTROL))
                    .flex_shrink_0()
                    .child(div().text_size(px(SMALL)).child(tr(
                        cx,
                        if content_width < 660. {
                            "日志"
                        } else {
                            "运行日志 · Ctrl+- / Ctrl+="
                        },
                    )))
                    .child(
                        frame("log-select-frame")
                            .w(px(if content_width < 510. { 156. } else { 200. }))
                            .h(px(CONTROL))
                            .flex_shrink_0()
                            .child(
                                Select::new(&self.log_selector)
                                    .id("log-selector")
                                    .accessibility_label(tr(cx, "选择运行记录"))
                                    .placeholder(tr(cx, "尚无运行记录"))
                                    .with_size(Size::Medium)
                                    .disabled(self.logs.is_empty())
                                    .w_full()
                                    .h(px(CONTROL))
                                    .rounded(px(CONTROL / 2.))
                                    .text_size(px(theme::font_size(cx))),
                            ),
                    )
                    .child(
                        action_button(
                            "detach-log",
                            "分离窗口",
                            IconName::ExternalLink,
                            width < 760. || theme::font_size(cx) > 16.,
                            cx,
                        )
                        .disabled(self.detaching_log)
                        .on_click(cx.listener(|v, _, _, cx| v.detach_log(cx))),
                    )
                    .child(
                        icon_button("delete-log", "删除当前运行记录", IconName::Trash, cx)
                            .disabled(
                                self.selected_log_id
                                    .and_then(|id| self.logs.iter().find(|log| log.id == id))
                                    .is_none_or(|log| !log.finished),
                            )
                            .on_click(cx.listener(|v, _, w, cx| v.delete_selected_log(w, cx))),
                    )
                    .when(
                        self.selected_log_id
                            .is_some_and(|id| self.run_handles.contains_key(&id)),
                        |row| {
                            row.child(
                                icon_button("stop-log", "停止当前运行", IconName::Square, cx)
                                    .on_click(cx.listener(|v, _, _, cx| v.stop_selected_run(cx))),
                            )
                            .when_some(
                                self.selected_running_command().map(str::to_owned),
                                |row, command| {
                                    row.child(
                                        icon_button(
                                            "copy-running-command",
                                            "复制当前运行命令",
                                            IconName::Copy,
                                            cx,
                                        )
                                        .tooltip(command)
                                        .on_click(
                                            cx.listener(|v, _, _, cx| {
                                                v.copy_selected_run_command(cx)
                                            }),
                                        ),
                                    )
                                },
                            )
                        },
                    )
                    .child(div().flex_1())
                    .child(
                        icon_button(
                            "log-scroll-bottom",
                            "回到底部并恢复自动跟随",
                            IconName::ArrowDownToLine,
                            cx,
                        )
                        .disabled(
                            self.log_auto_scroll.is_following() || self.selected_log_id.is_none(),
                        )
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.log_auto_scroll.request_tail();
                            cx.notify();
                        })),
                    )
                    .child(
                        icon_button("close-log", "收起日志", IconName::X, cx).on_click(
                            cx.listener(|v, _, _, cx| {
                                v.show_log = false;
                                v.config_dirty = true;
                                cx.notify();
                            }),
                        ),
                    ),
            )
            .when(log_height >= 76., |d| {
                d.child(
                    frame("log-output")
                        .relative()
                        .flex_1()
                        .min_h_0()
                        .capture_key_down(cx.listener(|v, event: &gpui::KeyDownEvent, w, cx| {
                            if !v.log_output.focus_handle(cx).is_focused(w) {
                                return;
                            }
                            if let Some(delta) = log_zoom_delta(event) {
                                v.zoom_log(delta, cx);
                                cx.stop_propagation();
                            }
                        }))
                        .child(
                            textarea(&self.log_output, "运行输出", true, cx)
                                .text_size(px(self.log_font_size as f32))
                                .line_height(px((self.log_font_size as f32 * 1.4).max(LINE)))
                                .readonly(true)
                                .bg(rgb(p.subtle)),
                        )
                        .child(
                            self.log_auto_scroll
                                .after_layout(&self.log_output, cx.entity_id()),
                        ),
                )
            });
        frame("workspace")
            .relative()
            .size_full()
            .capture_action(cx.listener(|v, _: &Paste, window, cx| {
                if v.paste_external_file_path(window, cx) {
                    cx.stop_propagation();
                }
            }))
            .capture_key_down(cx.listener(|v, event: &gpui::KeyDownEvent, _, cx| {
                if v.page_switch.is_some() {
                    v.finish_page_switch();
                    cx.notify();
                }
                let modifiers = event.keystroke.modifiers;
                let save_modifier = if cfg!(target_os = "macos") {
                    modifiers.platform && !modifiers.control
                } else {
                    modifiers.control && !modifiers.platform
                };
                if save_modifier
                    && !modifiers.alt
                    && !modifiers.shift
                    && event.keystroke.key.eq_ignore_ascii_case("s")
                {
                    if let Err(error) = v.save_configuration(cx) {
                        v.status = error;
                        cx.notify();
                    }
                    cx.stop_propagation();
                }
            }))
            .on_mouse_move(cx.listener(|v, event: &gpui::MouseMoveEvent, w, cx| {
                if v.sidebar_auto_expanded
                    && !Self::pointer_in_sidebar(
                        event.position,
                        v.expanded_sidebar_width(f32::from(w.viewport_size().width))
                            .max(v.sidebar_overlay_display_width),
                        f32::from(w.viewport_size().height),
                    )
                {
                    v.hover_sidebar(false, w, cx);
                }
                let Some(drag) = v.resizing else {
                    return;
                };
                if event.pressed_button != Some(MouseButton::Left) {
                    v.resizing = None;
                    return;
                }
                let delta = f32::from(event.position.y) - drag.start_y;
                match drag.kind {
                    ResizeKind::Rows => {
                        let count = v.current().rows.len();
                        if count > 3 {
                            let rows = ((drag.start_value - TABLE_INSET * 2. + GAP)
                                / (CONTROL + GAP)
                                + delta / (CONTROL + GAP))
                                .round() as usize;
                            v.tabs[v.active].row_height_override =
                                Some(rows.clamp(3, count.min(9)));
                            v.config_dirty = true;
                        }
                    }
                    ResizeKind::Log => {
                        v.log_height_override = Some((drag.start_value - delta).max(32.));
                        v.config_dirty = true;
                    }
                    ResizeKind::SidebarWidth => {
                        let movement = f32::from(event.position.x) - drag.start_x;
                        if movement.abs() > 0.5 {
                            v.sidebar_collapsed = false;
                            v.sidebar_auto_expanded = false;
                            v.sidebar_auto_suppressed = false;
                            v.sidebar_expanded_width = Some((drag.start_value + movement).clamp(
                                160.,
                                Self::max_sidebar_width(f32::from(w.viewport_size().width)),
                            ));
                            v.config_dirty = true;
                        }
                    }
                    ResizeKind::DescriptionWidth => {
                        let available = (f32::from(w.viewport_size().width)
                            - v.sidebar_display_width
                            - PAGE_INSET * 2.
                            - GAP)
                            .max(0.);
                        let movement = f32::from(event.position.x) - drag.start_x;
                        v.description_width_override = Some(
                            (drag.start_value - movement)
                                .clamp(120., (available - PREVIEW_MIN).max(120.)),
                        );
                        v.config_dirty = true;
                    }
                }
                w.refresh();
                cx.notify();
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|v, _, _, cx| {
                    v.stop_row_drag_auto_scroll();
                    v.resizing = None;
                    // on_drop runs after mouse_up; defer cleanup so its insertion side survives.
                    if v.tab_drag_target.is_none() && v.tab_drag_centers.is_empty() {
                        return;
                    }
                    let owner = cx.entity().downgrade();
                    cx.defer(move |cx| {
                        let _ = owner.update(cx, |v, cx| {
                            let had_target = v.tab_drag_target.take().is_some();
                            let had_centers = !v.tab_drag_centers.is_empty();
                            v.tab_drag_centers.clear();
                            v.tab_drop_x = None;
                            if had_target || had_centers {
                                cx.notify();
                            }
                        });
                    });
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|v, _, _, cx| {
                    v.stop_row_drag_auto_scroll();
                    v.resizing = None;
                    if v.tab_drag_target.is_none() && v.tab_drag_centers.is_empty() {
                        return;
                    }
                    let owner = cx.entity().downgrade();
                    cx.defer(move |cx| {
                        let _ = owner.update(cx, |v, cx| {
                            let had_target = v.tab_drag_target.take().is_some();
                            let had_centers = !v.tab_drag_centers.is_empty();
                            v.tab_drag_centers.clear();
                            v.tab_drop_x = None;
                            if had_target || had_centers {
                                cx.notify();
                            }
                        });
                    });
                }),
            )
            .flex()
            .flex_col()
            .bg(rgb(p.panel))
            .text_color(rgb(p.text))
            .font_family(cx.global::<Fonts>().body.clone())
            .text_size(px(theme::font_size(cx)))
            .font_weight(theme::font_weight(cx))
            .line_height(px(LINE.max(theme::font_size(cx) * 1.15)))
            .child(self.header(width, window, cx))
            .child(
                frame("workspace-body")
                    .relative()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(if overlay_visible {
                        // Keep a real 56px rail in the flex layout. The temporary
                        // expanded panel is painted last above the editor, not as
                        // an extra flex column that shifts inputs/IME geometry.
                        div().w(px(56.)).h_full().flex_shrink_0().into_any_element()
                    } else {
                        self.sidebar(sidebar_width, expanded_width, window, cx)
                            .into_any_element()
                    })
                    .child(
                        column()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .child(editor)
                            .when(self.show_log && self.detached_log.is_none(), |d| {
                                d.child(log)
                            })
                            .child(
                                frame("status-bar")
                                    .flex()
                                    .items_center()
                                    .bg(rgb(p.app))
                                    .border_t_1()
                                    .border_color(rgb(p.divider))
                                    .h(px(STATUS_HEIGHT))
                                    .flex_shrink_0()
                                    .px(px(PAGE_INSET))
                                    .text_size(px(SMALL))
                                    .text_color(rgb(p.muted))
                                    .overflow_hidden()
                                    .child(i18n::message(cx, &self.status)),
                            ),
                    )
                    .when(overlay_visible, |body| {
                        body.child(
                            frame("sidebar-hover-overlay")
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .left_0()
                                .w(px(overlay_width))
                                .flex()
                                .bg(rgb(p.sidebar))
                                .shadow_md()
                                .occlude()
                                .child(self.sidebar(overlay_width, expanded_width, window, cx)),
                        )
                    }),
            )
            // Kit's animated selected pill can paint over a marker inside tab-context.
            // Paint this non-interactive guide last, above the whole title bar.
            .when_some(
                self.tab_drag_target.and_then(|(area, id, _)| {
                    if area == TabArea::Top {
                        self.tab_drop_x.map(|x| (id, x))
                    } else {
                        None
                    }
                }),
                |d, (id, x)| {
                    d.child(
                        frame(("top-tab-drop", id))
                            .absolute()
                            .left(px(x - 2.))
                            .top(px((TITLE_HEIGHT - CONTROL) / 2.))
                            .w(px(4.))
                            .h(px(CONTROL))
                            .rounded_full()
                            .bg(rgb(if self.current().id == id {
                                // A white cue is legible but clashes with the accented pill.
                                // Text-selection tint preserves contrast and the accent family.
                                p.selection
                            } else {
                                p.focus
                            })),
                    )
                },
            )
            .when_some(self.tab_menu, |d, (tab_id, x, y)| {
                let group_options = self.sidebar_group_options(tab_id, y, window, cx);
                let menu = frame("tab-context-menu")
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(200.))
                    .p(px(GAP))
                    .rounded(px(RADIUS))
                    .bg(rgb(p.panel))
                    .border_1()
                    .border_color(rgb(p.border))
                    .shadow_md()
                    .occlude()
                    .on_mouse_down_out(cx.listener(|v, _, _, cx| {
                        v.tab_menu = None;
                        v.tab_group_options_open = false;
                        cx.notify();
                    }))
                    .child(
                        column()
                            .gap(px(4.))
                            .child(
                                button("context-copy", "复制此标签", Some(IconName::Copy), cx)
                                    .ghost()
                                    .w_full()
                                    .on_click(cx.listener(move |v, _, w, cx| {
                                        v.tab_menu = None;
                                        if let Some(i) = v.tabs.iter().position(|t| t.id == tab_id)
                                        {
                                            v.new_tab_from(Some(i), w, cx);
                                        }
                                    })),
                            )
                            .child(
                                button(
                                    "context-icon",
                                    "选择标签图标",
                                    Some(IconName::Settings2),
                                    cx,
                                )
                                .ghost()
                                .w_full()
                                .on_click(cx.listener(
                                    move |v, _, w, cx| {
                                        v.tab_menu = None;
                                        v.open_icon_picker(tab_id, w, cx);
                                    },
                                )),
                            )
                            .child(
                                button(
                                    "context-move-group",
                                    "移动到分组",
                                    Some(IconName::FolderOpen),
                                    cx,
                                )
                                .ghost()
                                .w_full()
                                .on_click(cx.listener(
                                    |view, _, window, cx| {
                                        view.tab_group_options_open = !view.tab_group_options_open;
                                        if view.tab_group_options_open {
                                            if let Some((tab_id, x, y)) = view.tab_menu {
                                                let max_y =
                                                    f32::from(window.viewport_size().height) - 300.;
                                                view.tab_menu =
                                                    Some((tab_id, x, y.min(max_y.max(8.))));
                                            }
                                        }
                                        cx.notify();
                                    },
                                )),
                            )
                            .when(self.tab_group_options_open, |menu| {
                                menu.child(group_options)
                            })
                            .child(
                                button("context-close", "关闭上部标签", Some(IconName::X), cx)
                                    .ghost()
                                    .w_full()
                                    .disabled(!self.open_tab_ids.contains(&tab_id))
                                    .on_click(cx.listener(move |v, _, w, cx| {
                                        v.tab_menu = None;
                                        if let Some(i) = v.tabs.iter().position(|t| t.id == tab_id)
                                        {
                                            v.request_close(i, w, cx);
                                        }
                                    })),
                            )
                            .child(
                                button("context-delete", "删除配置", Some(IconName::Trash), cx)
                                    .ghost()
                                    .w_full()
                                    .disabled(self.tabs.len() == 1)
                                    .on_click(cx.listener(move |v, _, w, cx| {
                                        v.tab_menu = None;
                                        if let Some(i) = v.tabs.iter().position(|t| t.id == tab_id)
                                        {
                                            v.request_delete(i, w, cx);
                                        }
                                    })),
                            ),
                    );
                if self.reduced_motion {
                    d.child(menu.into_any_element())
                } else {
                    d.child(
                        menu.with_animation(
                            "tab-menu-enter",
                            Animation::new(std::time::Duration::from_millis(120)),
                            |menu, progress| menu.opacity(progress),
                        )
                        .into_any_element(),
                    )
                }
            })
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
            .when(
                self.sidebar_auto_expanded || self.sidebar_auto_suppressed,
                |root| {
                    root.child(
                        // An OS mouse-exit can arrive without a final MouseMove. The
                        // GPUI platform keeps the last in-window pointer coordinate,
                        // so hitbox-filtered on_mouse_exit cannot reliably dismiss an
                        // overlay. Listen at the window paint phase instead.
                        gpui::canvas(
                            |_, _, _| {},
                            move |_, _, window, _| {
                                let owner = exit_owner.clone();
                                window.on_mouse_event(
                                    move |_: &gpui::MouseExitEvent, phase, _, cx| {
                                        if phase == gpui::DispatchPhase::Bubble {
                                            let _ = owner.update(cx, |view, cx| {
                                                view.sidebar_auto_expanded = false;
                                                view.sidebar_auto_suppressed = false;
                                                view.sidebar_hovered = None;
                                                cx.notify();
                                            });
                                        }
                                    },
                                );
                            },
                        )
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left_0()
                        .right_0(),
                    )
                },
            )
    }
}

impl Drop for CommandWorkspace {
    fn drop(&mut self) {
        for handle in self.run_handles.values() {
            let _ = handle.stop();
        }
    }
}

#[cfg(all(test, feature = "ui-test"))]
mod tray_action_tests {
    use super::*;
    use gpui::component::WindowExt;
    use gpui::test::TestWindowExt;
    use gpui::{size, TestAppContext};

    #[cfg(unix)]
    #[gpui::test]
    fn tray_run_and_stop_use_real_selected_run_handles(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            notifications::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut workspace = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |window, cx| {
            let view = cx.new(|cx| CommandWorkspace::new(window, cx));
            workspace = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = workspace.unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |view, cx| {
                let tab = &mut view.tabs[0];
                tab.program.update(cx, |state, cx| {
                    state.set_value("while :; do sleep 0.05; done", window, cx)
                });
                tab.other
                    .update(cx, |state, cx| state.set_value("", window, cx));
                for row in &mut tab.rows {
                    row.enabled = false;
                }
                assert!(!view.tray_menu_state(false, cx).can_stop);
                view.perform_tray_action(TrayAction::Run, window, cx);
                assert!(view.tray_menu_state(false, cx).can_stop);
                assert_eq!(view.logs.len(), 1);
                view.perform_tray_action(TrayAction::Stop, window, cx);
            })
        })
        .unwrap();
        let mut stopped = false;
        for _ in 0..100 {
            std::thread::sleep(std::time::Duration::from_millis(20));
            cx.background_executor
                .advance_clock(std::time::Duration::from_millis(24));
            cx.run_until_parked();
            stopped = cx.update(|cx| !view.read(cx).tray_menu_state(false, cx).can_stop);
            if stopped {
                break;
            }
        }
        assert!(stopped, "停止操作应终止选中的真实进程组并禁用 Stop");
        assert!(cx.update(|cx| view.read(cx).logs[0].stopped));
        cx.update_window(handle.into(), |_, window, _| window.remove_window())
            .unwrap();
    }

    #[cfg(unix)]
    #[gpui::test]
    fn running_log_copy_uses_launch_snapshot_in_docked_and_detached_windows(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut workspace = None;
        let main = cx.open_window(size(px(850.), px(800.)), |window, cx| {
            let view = cx.new(|cx| CommandWorkspace::new(window, cx));
            workspace = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = workspace.unwrap();
        let running = "while :; do sleep 0.05; done";
        cx.update_window(main.into(), |_, window, cx| {
            view.update(cx, |view, cx| {
                let tab = &mut view.tabs[0];
                tab.program
                    .update(cx, |input, cx| input.set_value(running, window, cx));
                tab.other
                    .update(cx, |input, cx| input.set_value("", window, cx));
                for row in &mut tab.rows {
                    row.enabled = false;
                }
                view.run_current_command(window, cx);
                assert_eq!(view.selected_running_command(), Some(running));
                view.tabs[0]
                    .program
                    .update(cx, |input, cx| input.set_value("echo changed", window, cx));
            });
            window.render_frame(cx);
            let stop = window.find("stop-log").bounds();
            let copy = window.find("copy-running-command").bounds();
            assert!(copy.left() >= stop.right(), "复制 SVG 应紧挨停止按钮右侧");
            window.click("copy-running-command", cx);
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some(running.into())
            );
        })
        .unwrap();
        cx.update_window(main.into(), |_, _, cx| {
            view.update(cx, |view, cx| view.detach_log(cx));
        })
        .unwrap();
        cx.run_until_parked();
        let detached = cx.update(|cx| view.read(cx).detached_log).unwrap();
        cx.update_window(detached.into(), |_, window, cx| {
            window.render_frame(cx);
            let stop = window.find("detached-stop-log").bounds();
            let copy = window.find("detached-copy-running-command").bounds();
            assert!(copy.left() >= stop.right());
            cx.write_to_clipboard(ClipboardItem::new_string("stale".into()));
            window.click("detached-copy-running-command", cx);
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some(running.into())
            );
            window.click("detached-stop-log", cx);
        })
        .unwrap();
        for _ in 0..100 {
            std::thread::sleep(std::time::Duration::from_millis(20));
            cx.background_executor
                .advance_clock(std::time::Duration::from_millis(24));
            cx.run_until_parked();
            if cx.update(|cx| view.read(cx).selected_running_command().is_none()) {
                break;
            }
        }
        cx.update(|cx| {
            let snapshot = view.read(cx).log_snapshot();
            assert!(!snapshot.can_stop);
            assert!(snapshot.selected_command.is_none());
            assert!(
                view.read(cx).logs[0].command.is_none(),
                "运行结束后清除命令快照"
            );
        });
        cx.update_window(detached.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("detached-copy-running-command").is_none());
            window.remove_window();
        })
        .unwrap();
        cx.update_window(main.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("copy-running-command").is_none());
            window.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn tray_actions_reuse_logs_settings_theme_and_explicit_save_without_autosave(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let dir = std::env::temp_dir().join(format!(
            "ecr-tray-actions-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = ConfigStore::at(dir.join("config.json"), dir.join("backup"));
        store
            .save(&serde_json::json!({"tabs":[{"name":"current","program":"echo original"}]}))
            .unwrap();
        let original = std::fs::read(store.config_path()).unwrap();
        let mut workspace = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |window, cx| {
            let view = cx.new(|cx| {
                CommandWorkspace::new_with_backend(
                    window,
                    cx,
                    Some(store.clone()),
                    store.load().unwrap(),
                    None,
                )
            });
            workspace = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = workspace.unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |view, cx| {
                let program = view.tabs[0].program.clone();
                program.update(cx, |state, cx| {
                    state.set_value("echo unsaved draft", window, cx)
                });
                let state = view.tray_menu_state(false, cx);
                assert!(state.can_run && state.can_save);
                assert!(!state.can_stop);
                view.perform_tray_action(TrayAction::Stop, window, cx);
                view.perform_tray_action(TrayAction::Logs, window, cx);
                assert!(view.show_log);
                view.perform_tray_action(TrayAction::Logs, window, cx);
                assert!(view.show_log, "打开运行日志不应反向收起");
                view.perform_tray_action(TrayAction::ToggleTheme, window, cx);
                assert_eq!(*cx.global::<Theme>(), Theme::Light);
                view.perform_tray_action(TrayAction::Settings, window, cx);
                assert!(view.settings_panel.is_some());
                assert!(window.has_active_dialog(cx));
                window.close_dialog(cx);
                assert_eq!(
                    std::fs::read(store.config_path()).unwrap(),
                    original,
                    "打开菜单／日志／设置及换主题都不可暗中保存命令"
                );
                assert_eq!(program.read(cx).value().as_ref(), "echo unsaved draft");
                view.perform_tray_action(TrayAction::Save, window, cx);
                assert_eq!(
                    store.load().unwrap().unwrap()["tabs"][0]["program"],
                    "echo unsaved draft"
                );
                view.open_tab_ids.clear();
                assert!(
                    !view.tray_menu_state(true, cx).can_run,
                    "没有打开配置时不可运行"
                );
            });
            window.remove_window();
        })
        .unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(all(test, feature = "ui-test"))]
mod log_scroll_tests {
    use super::*;
    use gpui::test::TestWindowExt;
    use gpui::{size, EntityInputHandler, TestAppContext};

    fn assert_tail(output: &Entity<TextareaState>, cx: &App) {
        let state = output.read(cx);
        let value = state.value();
        assert_eq!(state.selected_range(), value.len()..value.len());
        let rows = state.visible_row_range().expect("日志已布局");
        assert!(
            rows.contains(&value.matches('\n').count()),
            "最新一行必须可见：{rows:?}"
        );
        let (mut caret, _) = state.cursor_layout().expect("末尾光标已布局");
        caret.origin += state.scroll_offset();
        let viewport = state.input_bounds();
        assert!(
            caret.top() >= viewport.top() - px(1.) && caret.bottom() <= viewport.bottom() + px(1.),
            "末尾必须真正处于可视区，而非仅改变选区：{caret:?}/{viewport:?}"
        );
    }

    #[gpui::test]
    fn docked_output_follows_batches_and_wrapped_tail_without_stealing_focus(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui::init(cx);
            notifications::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut workspace = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |window, cx| {
            let view = cx.new(|cx| {
                let mut view = CommandWorkspace::new(window, cx);
                view.show_log = true;
                view.log_height_override = Some(180.);
                view.logs.push(SessionLog {
                    id: 0,
                    command: None,
                    command_name: "连续输出".into(),
                    caption: "连续输出 · 运行 #1".into(),
                    output: String::new(),
                    finished: false,
                    exit_code: None,
                    stopped: false,
                });
                view.selected_log_id = Some(0);
                view
            });
            workspace = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = workspace.unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            let program = view.read(cx).tabs[0].program.clone();
            window.focus(&program.focus_handle(cx), cx);
            let draft = program.update(cx, |state, cx| {
                state.set_value("", window, cx);
                state.replace_and_mark_text_in_range(None, "正在组字", Some(1..2), window, cx);
                (
                    state.value(),
                    state.selected_range(),
                    state.marked_text_range(window, cx),
                )
            });
            for batch in [
                (0..100)
                    .map(|i| format!("中文输出 🌟 第 {i} 行\n"))
                    .collect::<String>(),
                format!("{}最新的长行末尾", "中文自动换行🌟".repeat(120)),
                "\n最后一批输出".into(),
            ] {
                view.update(cx, |view, cx| {
                    view.receive_run_events(
                        0,
                        vec![RunEvent::Output {
                            stream: OutputStream::Stdout,
                            text: batch,
                        }],
                        window,
                        cx,
                    );
                });
                for _ in 0..3 {
                    window.render_frame(cx);
                }
                assert_tail(&view.read(cx).log_output, cx);
                assert!(
                    program.focus_handle(cx).is_focused(window),
                    "日志更新不能抢焦点"
                );
                let actual = program.update(cx, |state, cx| {
                    (
                        state.value(),
                        state.selected_range(),
                        state.marked_text_range(window, cx),
                    )
                });
                assert_eq!(actual, draft, "日志置底不能影响命令草稿、选区或 IME");
            }
            view.update(cx, |view, cx| {
                view.receive_run_events(
                    0,
                    vec![
                        RunEvent::Output {
                            stream: OutputStream::Stderr,
                            text: "\n完成前的最后一批标准错误".into(),
                        },
                        RunEvent::Finished {
                            exit_code: 0,
                            stopped: false,
                        },
                    ],
                    window,
                    cx,
                );
            });
            for _ in 0..3 {
                window.render_frame(cx);
            }
            assert_tail(&view.read(cx).log_output, cx);
            window.remove_window();
        })
        .unwrap();
    }

    fn long_output_workspace(
        window: &mut Window,
        cx: &mut Context<CommandWorkspace>,
    ) -> CommandWorkspace {
        let mut view = CommandWorkspace::new(window, cx);
        view.show_log = true;
        view.log_height_override = Some(180.);
        let output = (0..90)
            .map(|i| format!("历史输出 {i}\n"))
            .collect::<String>();
        view.logs = (0..2)
            .map(|id| SessionLog {
                id,
                command: None,
                command_name: format!("任务 {id}"),
                caption: format!("运行 #{}", id + 1),
                output: output.clone(),
                finished: false,
                exit_code: None,
                stopped: false,
            })
            .collect();
        view.selected_log_id = Some(0);
        view.select_log(0, window, cx);
        view
    }

    #[gpui::test]
    fn docked_log_preserves_paused_view_until_tail_button_and_follows_truncation(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui::init(cx);
            notifications::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut workspace = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |window, cx| {
            let view = cx.new(|cx| long_output_workspace(window, cx));
            workspace = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = workspace.unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            for _ in 0..3 {
                window.render_frame(cx);
            }
            let output = view.read(cx).log_output.clone();
            assert_tail(&output, cx);
            let original = output.read(cx).value();
            window.scroll(
                "log-output",
                gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(250.))),
                cx,
            );
            for _ in 0..3 {
                window.render_frame(cx);
            }
            let browsed = output.read(cx).scroll_offset();
            assert!(
                !view.read(cx).log_auto_scroll.is_following(),
                "向上滚动暂停跟随"
            );
            let selected = output.read(cx).selected_range();
            view.update(cx, |view, cx| {
                view.receive_run_events(
                    1,
                    vec![
                        RunEvent::Output {
                            stream: OutputStream::Stdout,
                            text: "后台任务的新输出".into(),
                        },
                        RunEvent::Finished {
                            exit_code: 0,
                            stopped: false,
                        },
                    ],
                    window,
                    cx,
                );
            });
            for _ in 0..3 {
                window.render_frame(cx);
            }
            assert_eq!(
                output.read(cx).value(),
                original,
                "别的任务不可覆盖当前日志"
            );
            assert_eq!(
                output.read(cx).scroll_offset(),
                browsed,
                "状态更新不可把历史视口重置到顶或底"
            );

            view.update(cx, |view, cx| {
                view.show_log = false;
                cx.notify();
            });
            window.render_frame(cx);
            view.update(cx, |view, cx| {
                view.receive_run_events(
                    0,
                    vec![RunEvent::Output {
                        stream: OutputStream::Stdout,
                        text: (90..180).map(|i| format!("收起时输出 {i}\n")).collect(),
                    }],
                    window,
                    cx,
                );
            });
            window.render_frame(cx);
            view.update(cx, |view, cx| {
                view.show_log = true;
                cx.notify();
            });
            for _ in 0..3 {
                window.render_frame(cx);
            }
            assert_eq!(
                output.read(cx).scroll_offset(),
                browsed,
                "新输出及展开不能拉走暂停视口"
            );
            assert_eq!(output.read(cx).selected_range(), selected);
            assert!(!view.read(cx).log_auto_scroll.is_following());
            view.update(cx, |view, cx| {
                view.log_height_override = Some(140.);
                view.zoom_log(8, cx);
            });
            for _ in 0..3 {
                window.render_frame(cx);
            }
            assert!(
                !view.read(cx).log_auto_scroll.is_following(),
                "调整高度或字号不恢复跟随"
            );
            window.click("log-scroll-bottom", cx);
            for _ in 0..3 {
                window.render_frame(cx);
            }
            assert!(view.read(cx).log_auto_scroll.is_following());
            assert_tail(&output, cx);

            view.update(cx, |view, cx| {
                view.receive_run_events(
                    0,
                    vec![RunEvent::Output {
                        stream: OutputStream::Stdout,
                        text: format!("{}\n", "中文输出🌟".repeat(24)).repeat(5200),
                    }],
                    window,
                    cx,
                );
            });
            for _ in 0..3 {
                window.render_frame(cx);
            }
            assert!(output.read(cx).value().starts_with("[前序输出已截断"));
            assert_tail(&output, cx);
            view.update(cx, |view, cx| {
                view.select_log(1, window, cx);
            });
            for _ in 0..3 {
                window.render_frame(cx);
            }
            assert_tail(&output, cx);
            window.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn detached_log_follows_initial_selection_new_output_and_reattachment(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut workspace = None;
        let main = cx.open_window(size(px(850.), px(800.)), |window, cx| {
            let view = cx.new(|cx| long_output_workspace(window, cx));
            workspace = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = workspace.unwrap();
        cx.update_window(main.into(), |_, _, cx| {
            view.update(cx, |view, cx| view.detach_log(cx));
        })
        .unwrap();
        cx.run_until_parked();
        let detached = cx
            .update(|cx| view.read(cx).detached_log)
            .expect("日志窗口已分离");
        let output = cx
            .update_window(detached.into(), |root, window, cx| {
                for _ in 0..3 {
                    window.render_frame(cx);
                }
                let pane = root
                    .downcast::<Root>()
                    .unwrap()
                    .read(cx)
                    .view()
                    .clone()
                    .downcast::<DetachedLogWindow>()
                    .unwrap();
                let output = pane.read(cx).output.clone();
                assert_tail(&output, cx);
                output.update(cx, |state, cx| {
                    state.set_scroll_offset(gpui::point(px(0.), px(-80.)), cx)
                });
                for _ in 0..3 {
                    window.render_frame(cx);
                }
                assert_eq!(output.read(cx).scroll_offset().y, px(-80.));
                output
            })
            .unwrap();
        // Both records deliberately have identical text: selection changes must
        // still reveal the tail, even without a set_value call.
        cx.update_window(main.into(), |_, window, cx| {
            view.update(cx, |view, cx| view.select_log(1, window, cx));
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(detached.into(), |_, window, cx| {
            for _ in 0..3 {
                window.render_frame(cx);
            }
            assert_tail(&output, cx);
        })
        .unwrap();
        cx.update_window(main.into(), |_, window, cx| {
            view.update(cx, |view, cx| {
                view.receive_run_events(
                    1,
                    (90..160)
                        .map(|i| RunEvent::Output {
                            stream: OutputStream::Stdout,
                            text: format!("分离窗口的新输出 {i}\n"),
                        })
                        .collect(),
                    window,
                    cx,
                );
            });
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(detached.into(), |_, window, cx| {
            for _ in 0..3 {
                window.render_frame(cx);
            }
            assert!(output.read(cx).value().contains("分离窗口的新输出 159"));
            assert_tail(&output, cx);
            window.remove_window();
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(main.into(), |_, window, cx| {
            for _ in 0..3 {
                window.render_frame(cx);
            }
            assert!(view.read(cx).detached_log.is_none());
            assert_tail(&view.read(cx).log_output, cx);
            window.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn upward_scroll_during_streaming_pauses_and_tail_button_resumes(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut workspace = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |window, cx| {
            let view = cx.new(|cx| long_output_workspace(window, cx));
            workspace = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = workspace.unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            for _ in 0..3 {
                window.render_frame(cx);
            }
            let output = view.read(cx).log_output.clone();
            assert_tail(&output, cx);
            // Pause while a preceding output batch still has a queued tail reveal.
            view.update(cx, |view, cx| {
                view.receive_run_events(
                    0,
                    vec![RunEvent::Output {
                        stream: OutputStream::Stdout,
                        text: "最新的第一批\n".into(),
                    }],
                    window,
                    cx,
                )
            });
            window.scroll(
                "log-output",
                gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(300.))),
                cx,
            );
            for _ in 0..3 {
                window.render_frame(cx);
            }
            let paused = output.read(cx).scroll_offset();
            assert!(!view.read(cx).log_auto_scroll.is_following());
            for _ in 0..4 {
                view.update(cx, |view, cx| {
                    view.receive_run_events(
                        0,
                        vec![RunEvent::Output {
                            stream: OutputStream::Stdout,
                            text: "暂停时的新输出\n".into(),
                        }],
                        window,
                        cx,
                    )
                });
                for _ in 0..3 {
                    window.render_frame(cx);
                }
                assert_eq!(output.read(cx).scroll_offset(), paused);
            }
            window.scroll(
                "log-output",
                gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(100000.))),
                cx,
            );
            for _ in 0..3 {
                window.render_frame(cx);
            }
            assert_eq!(
                output.read(cx).scroll_offset().y,
                px(0.),
                "向上过量滚动应钳制到顶部"
            );
            output.update(cx, |state, cx| state.set_selected_range(0..8, cx));
            for _ in 0..3 {
                window.render_frame(cx);
            }
            let selected = output.read(cx).selected_range();
            view.update(cx, |view, cx| {
                view.receive_run_events(
                    0,
                    vec![RunEvent::Output {
                        stream: OutputStream::Stdout,
                        text: "复制旧输出时的新内容\n".into(),
                    }],
                    window,
                    cx,
                )
            });
            for _ in 0..3 {
                window.render_frame(cx);
            }
            assert_eq!(
                output.read(cx).selected_range(),
                selected,
                "暂停时不破坏复制选区"
            );
            assert_eq!(output.read(cx).scroll_offset().y, px(0.));
            window.click("log-scroll-bottom", cx);
            for _ in 0..3 {
                window.render_frame(cx);
            }
            assert!(view.read(cx).log_auto_scroll.is_following());
            assert_tail(&output, cx);
            view.update(cx, |view, cx| {
                view.receive_run_events(
                    0,
                    vec![RunEvent::Output {
                        stream: OutputStream::Stdout,
                        text: "按钮后继续跟随🌟\n".into(),
                    }],
                    window,
                    cx,
                )
            });
            for _ in 0..3 {
                window.render_frame(cx);
            }
            assert_tail(&output, cx);
            window.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn detached_pause_survives_updates_and_reattachment_until_tail_button(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut workspace = None;
        let main = cx.open_window(size(px(850.), px(800.)), |window, cx| {
            let view = cx.new(|cx| long_output_workspace(window, cx));
            workspace = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = workspace.unwrap();
        cx.update_window(main.into(), |_, _, cx| {
            view.update(cx, |view, cx| view.detach_log(cx))
        })
        .unwrap();
        cx.run_until_parked();
        let detached = cx.update(|cx| view.read(cx).detached_log).unwrap();
        let (output, paused) = cx
            .update_window(detached.into(), |root, window, cx| {
                for _ in 0..3 {
                    window.render_frame(cx);
                }
                let pane = root
                    .downcast::<Root>()
                    .unwrap()
                    .read(cx)
                    .view()
                    .clone()
                    .downcast::<DetachedLogWindow>()
                    .unwrap();
                let output = pane.read(cx).output.clone();
                assert_tail(&output, cx);
                window.scroll(
                    "detached-log-output",
                    gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(300.))),
                    cx,
                );
                for _ in 0..3 {
                    window.render_frame(cx);
                }
                assert!(!view.read(cx).log_auto_scroll.is_following());
                let paused = output.read(cx).scroll_offset();
                (output, paused)
            })
            .unwrap();
        cx.update_window(main.into(), |_, window, cx| {
            view.update(cx, |view, cx| {
                view.receive_run_events(
                    0,
                    vec![RunEvent::Output {
                        stream: OutputStream::Stdout,
                        text: "分离暂停时的新输出\n".into(),
                    }],
                    window,
                    cx,
                );
            })
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(detached.into(), |_, window, cx| {
            for _ in 0..3 {
                window.render_frame(cx);
            }
            assert_eq!(output.read(cx).scroll_offset(), paused);
            window.click("detached-log-scroll-bottom", cx);
            for _ in 0..3 {
                window.render_frame(cx);
            }
            assert_tail(&output, cx);
            assert!(view.read(cx).log_auto_scroll.is_following());
            window.scroll(
                "detached-log-output",
                gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(200.))),
                cx,
            );
            for _ in 0..3 {
                window.render_frame(cx);
            }
            assert!(!view.read(cx).log_auto_scroll.is_following());
            window.remove_window();
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(main.into(), |_, window, cx| {
            for _ in 0..3 {
                window.render_frame(cx);
            }
            assert!(
                !view.read(cx).log_auto_scroll.is_following(),
                "重新停靠不取消暂停状态"
            );
            window.click("log-scroll-bottom", cx);
            for _ in 0..3 {
                window.render_frame(cx);
            }
            assert!(view.read(cx).log_auto_scroll.is_following());
            assert_tail(&view.read(cx).log_output, cx);
            window.remove_window();
        })
        .unwrap();
    }
}

#[cfg(all(test, feature = "ui-test"))]
mod completion_notification_tests {
    use super::*;
    use gpui::{size, TestAppContext};

    #[gpui::test]
    fn finished_events_notify_once_and_ignore_output_stop_and_unknown_runs(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui::init(cx);
            notifications::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut workspace = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |window, cx| {
            let view = cx.new(|cx| {
                let mut view = CommandWorkspace::new(window, cx);
                view.logs = ["成功任务", "失败任务", "主动停止"]
                    .into_iter()
                    .enumerate()
                    .map(|(id, name)| SessionLog {
                        id,
                        command: None,
                        command_name: name.into(),
                        caption: format!("运行 #{}", id + 1),
                        output: String::new(),
                        finished: false,
                        exit_code: None,
                        stopped: false,
                    })
                    .collect();
                view
            });
            workspace = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = workspace.unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |view, cx| {
                view.receive_run_events(
                    0,
                    vec![RunEvent::Output {
                        stream: OutputStream::Stderr,
                        text: "仍在运行的日志".into(),
                    }],
                    window,
                    cx,
                );
            });
        })
        .unwrap();
        assert!(cx.shown_system_notifications().is_empty());
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |view, cx| {
                for (id, exit_code, stopped) in [
                    (0, 0, false),
                    (0, 0, false),
                    (1, 1, false),
                    (2, 1, true),
                    (99, 1, false),
                ] {
                    view.receive_run_events(
                        id,
                        vec![RunEvent::Finished { exit_code, stopped }],
                        window,
                        cx,
                    );
                }
                assert_eq!(view.logs[0].caption, "运行 #1 · 退出 0");
                assert_eq!(view.logs[1].exit_code, Some(1));
                assert!(view.logs[2].stopped);
            });
            window.remove_window();
        })
        .unwrap();
        let shown = cx.shown_system_notifications();
        assert_eq!(shown.len(), 2);
        assert_eq!(shown[0].title.as_ref(), "正常结束");
        assert_eq!(shown[0].body.as_ref(), "「成功任务」· 退出代码：0");
        assert_eq!(shown[1].title.as_ref(), "运行报错");
        assert_eq!(shown[1].body.as_ref(), "「失败任务」· 退出代码：1");
    }
}

#[cfg(all(test, feature = "ui-test"))]
mod page_switch_tests {
    use super::*;
    use gpui::component::Root;
    use gpui::test::TestWindowExt;
    use gpui::{size, AppContext, TestAppContext};

    #[gpui::test]
    fn all_accent_keys_restore_and_save_without_losing_command_edits(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        for accent in Accent::ALL {
            let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
                let view = cx.new(|cx| {
                    CommandWorkspace::new_with_backend(
                        w,
                        cx,
                        None,
                        Some(serde_json::json!({"gpui":{"accent":accent.key()}})),
                        None,
                    )
                });
                view.update(cx, |v, cx| {
                    assert_eq!(*cx.global::<Accent>(), accent);
                    assert_eq!(v.configuration(cx)["gpui"]["accent"], accent.key());
                    let program = v.tabs[0].program.read(cx).value().to_string();
                    let preferences = Preferences {
                        theme: Theme::Light,
                        accent,
                        font_size: 14,
                        font_weight: 400,
                        language: Language::Chinese,
                        reduced_motion: true,
                    };
                    v.apply_preferences(preferences, w, cx).unwrap();
                    assert_eq!(v.configuration(cx)["gpui"]["accent"], accent.key());
                    assert_eq!(v.tabs[0].program.read(cx).value().as_ref(), program);
                });
                Root::new(view, w, cx)
            });
            cx.update_window(handle.into(), |_, w, _| w.remove_window())
                .unwrap();
        }
        assert_eq!(Accent::from_key("unknown"), Accent::Blue);
    }

    #[gpui::test]
    fn switches_animate_only_for_a_different_page_and_keep_live_inputs(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut workspace = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |window, cx| {
            let view = cx.new(|cx| CommandWorkspace::new(window, cx));
            workspace = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = workspace.unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(
                view.read(cx).page_switch.is_none(),
                "startup should not animate"
            );
            let editor = window.find("command-editor").bounds();
            let status_bar = window.find("status-bar").bounds();
            view.update(cx, |v, cx| v.select_tab(99, window, cx));
            assert!(
                view.read(cx).page_switch.is_none(),
                "invalid index is ignored"
            );
            view.update(cx, |v, cx| v.new_tab(false, window, cx));
            assert_eq!(view.read(cx).page_switch, Some((1, 1)));
            window.render_frame(cx);
            let outgoing = window.find("page-slide-outgoing").bounds();
            let incoming = window.find("page-slide-incoming").bounds();
            assert_eq!(outgoing.left(), editor.left());
            assert_eq!(incoming.left(), editor.right());
            assert_eq!(outgoing.size, editor.size);
            assert_eq!(incoming.size, editor.size);
            assert!(window.try_find("page-switch-swipe").is_none());
            assert!(window.try_find("page-switch-cue").is_none());
            assert_eq!(
                window.find("page-switch-content").bounds().left(),
                editor.left(),
                "动画不得移动真实输入控件"
            );
            assert_eq!(window.find("command-editor").bounds(), editor);
            assert_eq!(window.find("status-bar").bounds(), status_bar);
            window.click("program", cx);
            assert!(
                view.read(cx).page_switch.is_none(),
                "点击编辑区应立即结束滑动并保留本次点击"
            );
            window.input("echo new page", cx);
            assert_eq!(
                view.read(cx).tabs[1].program.read(cx).value(),
                "echo new page"
            );
            assert_ne!(
                view.read(cx).tabs[0].program.read(cx).value(),
                "echo new page"
            );
            let epoch = view.read(cx).page_switch_epoch;
            view.update(cx, |v, cx| v.select_tab(1, window, cx));
            assert_eq!(
                view.read(cx).page_switch_epoch,
                epoch,
                "reselect must not replay"
            );
            view.update(cx, |v, cx| v.select_tab(0, window, cx));
            assert_eq!(view.read(cx).page_switch, Some((epoch + 1, -1)));
            window.render_frame(cx);
            assert_eq!(
                window.find("page-slide-incoming").bounds().left(),
                editor.left() - editor.size.width,
                "前一页应从左侧整页滑入"
            );
            assert_eq!(
                window.find("page-switch-content").bounds().left(),
                editor.left()
            );
            view.update(cx, |v, cx| v.select_tab(1, window, cx));
            assert_eq!(view.read(cx).page_switch, Some((epoch + 2, 1)));
            assert_eq!(
                view.read(cx).tabs[1].program.read(cx).value(),
                "echo new page"
            );
            // A removed preceding tab changes the index, not the selected page.
            view.update(cx, |v, cx| v.close_tab(0, window, cx));
            assert_eq!(view.read(cx).page_switch_epoch, epoch + 2);
            view.update(cx, |v, cx| v.new_tab(false, window, cx));
            assert_eq!(view.read(cx).page_switch_epoch, epoch + 3);
            view.update(cx, |v, cx| v.close_tab(1, window, cx));
            assert_eq!(view.read(cx).page_switch_epoch, epoch + 4);
            window.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn sliding_sheets_keep_live_inputs_and_finish_or_retarget_without_resetting(
        cx: &mut TestAppContext,
    ) {
        use gpui::component::input::InputState;
        use std::time::Duration;
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut workspace = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            let view = cx.new(|cx| CommandWorkspace::new(w, cx));
            workspace = Some(view.clone());
            Root::new(view, w, cx)
        });
        let view = workspace.unwrap();
        let inputs = cx
            .update_window(handle.into(), |_, w, cx| {
                view.update(cx, |v, cx| {
                    v.new_tab(false, w, cx);
                    v.finish_page_switch();
                    v.tabs[0].program.update(cx, |s: &mut InputState, cx| {
                        s.set_value("旧页中文草稿", w, cx)
                    });
                    v.tabs[1].program.update(cx, |s: &mut InputState, cx| {
                        s.set_value("新页中文草稿", w, cx)
                    });
                });
                (
                    view.read(cx).tabs[0].program.entity_id(),
                    view.read(cx).tabs[1].program.entity_id(),
                )
            })
            .unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            view.update(cx, |v, cx| {
                v.reduced_motion = true;
                v.select_tab(0, w, cx);
                v.reduced_motion = false;
            });
            w.render_frame(cx);
            view.update(cx, |v, cx| v.select_tab(1, w, cx));
            w.render_frame(cx);
            let transition = view.read(cx).page_transition.as_ref().unwrap();
            match transition.outgoing.as_ref() {
                PageVisual::Page(page) => assert_eq!(page.data.program, "旧页中文草稿"),
                _ => panic!("首次滑出必须是实际旧页内容"),
            }
            assert_eq!(
                transition.incoming.as_ref().unwrap().data.program,
                "新页中文草稿"
            );
        })
        .unwrap();
        cx.background_executor
            .advance_clock(Duration::from_millis(90));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            let viewport = w.find("command-editor").bounds();
            let old = w.find("page-slide-outgoing").bounds();
            let new = w.find("page-slide-incoming").bounds();
            assert!(old.left() < viewport.left() && old.right() > viewport.left());
            assert!(new.left() > viewport.left() && new.left() < viewport.right());
            assert!((old.right() - new.left()).abs() <= px(1.));
            view.update(cx, |v, cx| v.select_tab(0, w, cx));
            assert!(matches!(
                view.read(cx)
                    .page_transition
                    .as_ref()
                    .unwrap()
                    .outgoing
                    .as_ref(),
                PageVisual::Composite { .. }
            ));
            w.render_frame(cx);
            assert_eq!(view.read(cx).tabs[0].program.entity_id(), inputs.0);
            assert_eq!(view.read(cx).tabs[1].program.entity_id(), inputs.1);
            assert_eq!(
                view.read(cx).tabs[0].program.read(cx).value().as_ref(),
                "旧页中文草稿"
            );
            assert_eq!(
                view.read(cx).tabs[1].program.read(cx).value().as_ref(),
                "新页中文草稿"
            );
        })
        .unwrap();
        cx.background_executor
            .advance_clock(Duration::from_millis(300));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            assert!(view.read(cx).page_switch.is_none());
            assert!(view.read(cx).page_transition.is_none());
            assert!(w.try_find("page-slide-outgoing").is_none());
            assert!(w.try_find("page-slide-incoming").is_none());
            w.click("program", cx);
            w.input("追加", cx);
            assert!(view.read(cx).tabs[0]
                .program
                .read(cx)
                .value()
                .contains("追加"));
            w.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn page_snapshots_preserve_selection_and_ime_marked_text(cx: &mut TestAppContext) {
        use gpui::EntityInputHandler;
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut workspace = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            let view = cx.new(|cx| CommandWorkspace::new(w, cx));
            workspace = Some(view.clone());
            Root::new(view, w, cx)
        });
        let view = workspace.unwrap();
        let (program, before) = cx
            .update_window(handle.into(), |_, w, cx| {
                view.update(cx, |v, cx| {
                    v.reduced_motion = true;
                    v.new_tab(false, w, cx);
                    v.select_tab(0, w, cx);
                    v.reduced_motion = false;
                });
                w.click("program", cx);
                let program = view.read(cx).tabs[0].program.clone();
                let before = program.update(cx, |state, cx| {
                    state.set_value("", w, cx);
                    state.replace_and_mark_text_in_range(None, "正在组字", Some(1..2), w, cx);
                    (
                        state.value(),
                        state.selected_range(),
                        state.marked_text_range(w, cx),
                    )
                });
                assert!(before.2.is_some());
                (program, before)
            })
            .unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            view.update(cx, |v, cx| v.select_tab(1, w, cx));
            w.render_frame(cx);
            assert_eq!(
                program.entity_id(),
                view.read(cx).tabs[0].program.entity_id()
            );
            let after = program.update(cx, |state, cx| {
                (
                    state.value(),
                    state.selected_range(),
                    state.marked_text_range(w, cx),
                )
            });
            assert_eq!(after, before, "视觉快照不可改动真实输入的组字或选区");
            w.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn app_and_system_reduced_motion_switch_immediately(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Light, cx);
        });
        let mut workspace = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |window, cx| {
            let view = cx.new(|cx| CommandWorkspace::new(window, cx));
            workspace = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = workspace.unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |v, cx| {
                v.new_tab(false, window, cx);
                v.reduced_motion = true;
                theme::set_reduced_motion(true, cx);
                v.select_tab(0, window, cx);
            });
            window.render_frame(cx);
            assert!(view.read(cx).page_switch.is_none());
            assert!(window.try_find("page-slide-outgoing").is_none());
            assert!(window.try_find("page-slide-incoming").is_none());
            assert!(view.read(cx).page_transition.is_none());
            assert_eq!(
                window.find("page-switch-content").bounds().left(),
                window.find("command-editor").bounds().left()
            );
            view.update(cx, |v, cx| {
                v.reduced_motion = false;
                theme::set_reduced_motion(false, cx);
            });
        })
        .unwrap();
        cx.update(|cx| cx.set_reduce_motion(true));
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |v, cx| v.select_tab(1, window, cx));
            window.render_frame(cx);
            assert!(view.read(cx).page_switch.is_none());
            assert!(view.read(cx).page_transition.is_none());
            assert!(window.try_find("page-slide-incoming").is_none());
            assert_eq!(view.read(cx).active, 1);
            window.remove_window();
        })
        .unwrap();
    }
}

#[cfg(all(test, feature = "ui-test"))]
mod sidebar_group_tests {
    use super::*;
    use gpui::component::Root;
    use gpui::test::TestWindowExt;
    use gpui::{size, AppContext, TestAppContext};

    #[gpui::test]
    fn sidebar_groups_create_move_collapse_and_persist(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let dir = std::env::temp_dir().join(format!(
            "ecr-sidebar-groups-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::at(dir.join("config.json"), dir.join("backup"));
        store
            .save(&serde_json::json!({
                "tabs": [
                    {"name":"视频工具","program":"ffmpeg"},
                    {"name":"下载工具","program":"yt-dlp"},
                    {"name":"脚本","program":"python"}
                ]
            }))
            .unwrap();
        let initial = store.load().unwrap().unwrap();
        let mut workspace = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |window, cx| {
            let view = cx.new(|cx| {
                let mut view = CommandWorkspace::new_with_backend(
                    window,
                    cx,
                    Some(store.clone()),
                    Some(initial),
                    None,
                );
                view.reduced_motion = true;
                view
            });
            workspace = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = workspace.unwrap();
        let group_id = cx
            .update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                assert!(window.try_find("sidebar-add-group").is_some());
                window.click("sidebar-add-group", cx);
                window.render_frame(cx);
                window.input("媒体工具", cx);
                window.render_frame(cx);
                assert_eq!(
                    window.find("sidebar-group-name-input").value(),
                    Some("媒体工具")
                );
                window.click("save-sidebar-group", cx);
                window.render_frame(cx);
                let group_id = view.read(cx).sidebar_groups[0].id;
                assert!(window
                    .try_find(("sidebar-group-header", group_id))
                    .is_some());
                assert!(window.try_find(("sidebar-tab", 0usize)).is_some());

                // Move an existing configuration using its row context menu.
                window.right_click(("sidebar-tab", 0usize), cx);
                window.click("context-move-group", cx);
                window.render_frame(cx);
                assert!(window
                    .try_find(("context-group-option", group_id))
                    .is_some());
                window.click(("context-group-option", group_id), cx);
                window.render_frame(cx);
                assert_eq!(view.read(cx).tabs[0].sidebar_group_id, Some(group_id));
                let ungrouped = window.find(("sidebar-group-header", u64::MAX)).bounds();
                let section = window.find(("sidebar-group-header", group_id)).bounds();
                let grouped_row = window.find(("sidebar-tab", 0usize)).bounds();
                assert!(
                    ungrouped.top() < section.top(),
                    "未分组区域应排在用户分组前"
                );
                assert_eq!(section.size.height, px(CONTROL));
                assert!(
                    grouped_row.top() >= section.bottom() + px(4.),
                    "group header and its first row must be separated: {section:?} / {grouped_row:?}"
                );
                theme::apply(Theme::Light, cx);
                window.render_frame(cx);
                assert_eq!(
                    window.find(("sidebar-group-header", group_id)).bounds(),
                    section,
                    "分组布局在浅色主题下保持稳定"
                );
                theme::apply(Theme::Dark, cx);
                window.render_frame(cx);

                window.click(("sidebar-group-toggle", group_id), cx);
                window.render_frame(cx);
                assert!(window.try_find(("sidebar-tab", 0usize)).is_none());
                assert!(window
                    .try_find(("sidebar-group-header", group_id))
                    .is_some());
                window.click(("sidebar-group-toggle", group_id), cx);
                window.render_frame(cx);
                assert!(window.try_find(("sidebar-tab", 0usize)).is_some());
                window.click(("sidebar-group-toggle", group_id), cx);
                view.update(cx, |view, cx| view.save_configuration(cx).unwrap());
                assert_eq!(
                    store.load().unwrap().unwrap()["tabs"][0]["gpui_group_id"],
                    serde_json::json!(group_id)
                );
                window.click("collapse-sidebar", cx);
                window.render_frame(cx);
                let header = window.find(("sidebar-group-header", group_id)).bounds();
                let disclosure = window.find(("sidebar-group-toggle", group_id)).bounds();
                assert_eq!(disclosure.center().x, header.center().x);
                assert!(window.try_find("sidebar-scrollbar-lane").is_none());
                window.remove_window();
                group_id
            })
            .unwrap();

        let saved = store.load().unwrap().unwrap();
        assert_eq!(saved["gpui"]["sidebar_groups"][0]["name"], "媒体工具");
        assert_eq!(saved["gpui"]["sidebar_groups"][0]["collapsed"], true);
        let mut restored = None;
        let restored_window = cx.open_window(size(px(850.), px(800.)), |window, cx| {
            let view = cx.new(|cx| {
                CommandWorkspace::new_with_backend(
                    window,
                    cx,
                    Some(store.clone()),
                    Some(saved),
                    None,
                )
            });
            restored = Some(view.clone());
            Root::new(view, window, cx)
        });
        let restored = restored.unwrap();
        cx.update_window(restored_window.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(restored.read(cx).sidebar_groups[0].id, group_id);
            assert_eq!(restored.read(cx).tabs[0].sidebar_group_id, Some(group_id));
            assert!(window
                .try_find(("sidebar-group-header", group_id))
                .is_some());
            assert!(window.try_find(("sidebar-tab", 0usize)).is_none());
            restored.update(cx, |view, cx| {
                assert!(view
                    .rename_sidebar_group_named(group_id, "未分组", cx)
                    .is_err());
                view.rename_sidebar_group_named(group_id, "影音工具", cx)
                    .unwrap();
                view.delete_sidebar_group(group_id, cx);
                assert!(view.sidebar_groups.is_empty());
                assert_eq!(view.tabs.len(), 3, "删除分组不能删除配置");
                assert!(view.tabs.iter().all(|tab| tab.sidebar_group_id.is_none()));
            });
            window.remove_window();
        })
        .unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
}
