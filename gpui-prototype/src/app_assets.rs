//! Embed the full offline Lucide catalog, shared by controls and the emoji-style icon picker.
use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;

#[derive(Clone, Copy, Debug, Default)]
pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        gpui::assets::AllAssets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        gpui::assets::AllAssets.list(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::assets::IconName;

    #[test]
    fn every_icon_used_by_the_ui_has_an_embedded_svg() {
        let icons = [
            IconName::Ellipsis,
            IconName::Plus,
            IconName::Copy,
            IconName::Terminal,
            IconName::X,
            IconName::FolderOpen,
            IconName::Settings2,
            IconName::Sun,
            IconName::Moon,
            IconName::ChevronLeft,
            IconName::ChevronRight,
            IconName::ChevronDown,
            IconName::FolderPlus,
            IconName::ListIndentDecrease,
            IconName::ListIndentIncrease,
            IconName::MessageSquare,
            IconName::Trash,
            IconName::GripVertical,
            IconName::Check,
            IconName::CheckCheck,
            IconName::Square,
            IconName::Minus,
            IconName::FileText,
            IconName::ExternalLink,
            IconName::ArrowDownToLine,
            IconName::ArrowDownAZ,
            IconName::AppWindow,
            IconName::LogOut,
            IconName::ScanText,
            IconName::RefreshCw,
            IconName::Shuffle,
            IconName::Play,
            IconName::Save,
        ];
        for icon in icons {
            let path = icon.path();
            let svg = AppAssets
                .load(&path)
                .unwrap()
                .unwrap_or_else(|| panic!("图标没有被打包：{path}"));
            assert!(
                std::str::from_utf8(&svg).unwrap().contains("<svg"),
                "{path}"
            );
        }
        assert!(AppAssets.list("icons/").unwrap().len() >= 1800);
    }
}
