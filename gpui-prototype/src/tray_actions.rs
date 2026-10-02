//! One action/state contract shared by Windows' rounded popup and native macOS/GTK menus.
use crate::{
    i18n::{self, Language},
    theme::Theme,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TrayAction {
    ToggleWindow,
    Run,
    Stop,
    Logs,
    Save,
    Settings,
    ToggleTheme,
    Quit,
}

impl TrayAction {
    pub const ALL: [Self; 8] = [
        Self::ToggleWindow,
        Self::Run,
        Self::Stop,
        Self::Logs,
        Self::Save,
        Self::Settings,
        Self::ToggleTheme,
        Self::Quit,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::ToggleWindow => "ecr-tray-window",
            Self::Run => "ecr-tray-run",
            Self::Stop => "ecr-tray-stop",
            Self::Logs => "ecr-tray-logs",
            Self::Save => "ecr-tray-save",
            Self::Settings => "ecr-tray-settings",
            Self::ToggleTheme => "ecr-tray-theme",
            Self::Quit => "ecr-tray-quit",
        }
    }
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.id() == id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TrayMenuState {
    pub language: Language,
    pub visible: bool,
    pub can_hide: bool,
    pub can_run: bool,
    pub can_stop: bool,
    pub can_save: bool,
    pub theme: Theme,
}

impl TrayMenuState {
    pub fn enabled(&self, action: TrayAction) -> bool {
        match action {
            TrayAction::ToggleWindow => !self.visible || self.can_hide,
            TrayAction::Run => self.can_run,
            TrayAction::Stop => self.can_stop,
            TrayAction::Save => self.can_save,
            _ => true,
        }
    }
    pub fn checked(&self, action: TrayAction) -> bool {
        action == TrayAction::ToggleTheme && self.theme == Theme::Dark
    }
    pub fn label(&self, action: TrayAction) -> String {
        i18n::translate(
            self.language,
            match action {
                TrayAction::ToggleWindow if self.visible => "隐藏主窗口",
                TrayAction::ToggleWindow => "显示主窗口",
                TrayAction::Run => "运行当前配置",
                TrayAction::Stop => "停止选中的运行",
                TrayAction::Logs => "运行日志",
                TrayAction::Save => "保存配置",
                TrayAction::Settings => "工作区设置",
                TrayAction::ToggleTheme => "深色模式",
                TrayAction::Quit => "退出…",
            },
        )
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_and_popup_actions_have_unique_round_trip_ids() {
        let ids: std::collections::HashSet<_> =
            TrayAction::ALL.into_iter().map(TrayAction::id).collect();
        assert_eq!(ids.len(), 8);
        for action in TrayAction::ALL {
            assert_eq!(TrayAction::from_id(action.id()), Some(action));
        }
        assert_eq!(TrayAction::from_id("foreign-menu"), None);
    }
    #[test]
    fn run_stop_save_and_hide_follow_real_capabilities() {
        let mut state = TrayMenuState {
            language: Language::Chinese,
            visible: false,
            can_hide: false,
            can_run: false,
            can_stop: false,
            can_save: false,
            theme: Theme::Dark,
        };
        assert!(state.enabled(TrayAction::ToggleWindow));
        assert_eq!(state.label(TrayAction::ToggleWindow), "显示主窗口");
        for action in [TrayAction::Run, TrayAction::Stop, TrayAction::Save] {
            assert!(!state.enabled(action));
        }
        state.visible = true;
        assert_eq!(state.label(TrayAction::ToggleWindow), "隐藏主窗口");
        assert!(!state.enabled(TrayAction::ToggleWindow));
        state.can_run = true;
        state.can_stop = true;
        state.can_save = true;
        assert!(
            state.enabled(TrayAction::Run)
                && state.enabled(TrayAction::Stop)
                && state.enabled(TrayAction::Save)
        );
        assert!(state.checked(TrayAction::ToggleTheme));
        state.theme = Theme::Light;
        assert!(!state.checked(TrayAction::ToggleTheme));
    }
}
