use gpui::{App, Global};
use std::{collections::HashMap, sync::LazyLock};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Language {
    #[default]
    Chinese,
    English,
}
impl Global for Language {}
impl Language {
    pub fn code(self) -> &'static str {
        match self {
            Self::Chinese => "zh_CN",
            Self::English => "en_US",
        }
    }
    pub fn from_code(code: &str) -> Self {
        if code == "en_US" {
            Self::English
        } else {
            Self::Chinese
        }
    }
}
static ENGLISH: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../locales/en.json")).expect("valid English catalog")
});

/// Only application-owned labels are translated. Never pass command contents or process output here.
pub fn tr(cx: &App, text: &str) -> String {
    translate(
        cx.try_global::<Language>().copied().unwrap_or_default(),
        text,
    )
}

pub fn translate(language: Language, text: &str) -> String {
    if language != Language::English {
        return text.to_owned();
    }
    ENGLISH
        .get(text)
        .cloned()
        .unwrap_or_else(|| text.to_owned())
}

pub fn apply(language: Language, cx: &mut App) {
    cx.set_global(language);
    gpui::component::set_locale(if language == Language::English {
        "en"
    } else {
        "zh-CN"
    });
}

pub fn format(cx: &App, text: &str, values: &[&str]) -> String {
    let translated = tr(cx, text);
    let mut parts = translated.split("{}");
    let mut output = parts.next().unwrap_or_default().to_owned();
    for (index, part) in parts.enumerate() {
        output.push_str(values.get(index).copied().unwrap_or("{}"));
        output.push_str(part);
    }
    output
}

/// Translate application diagnostics with parameters without modifying paths or OS error text.
pub fn message(cx: &App, text: &str) -> String {
    if cx.try_global::<Language>() != Some(&Language::English) {
        return text.to_owned();
    }
    if let Some(value) = ENGLISH.get(text) {
        return value.clone();
    }
    let mut templates = ENGLISH
        .iter()
        .filter(|(key, _)| key.contains("{}"))
        .collect::<Vec<_>>();
    templates.sort_by_key(|(key, _)| std::cmp::Reverse(key.len()));
    for (key, _) in templates {
        let parts = key.split("{}").collect::<Vec<_>>();
        let Some(mut tail) = text.strip_prefix(parts[0]) else {
            continue;
        };
        let mut values = Vec::new();
        let mut matched = true;
        for part in &parts[1..] {
            if part.is_empty() {
                values.push(tail);
                tail = "";
            } else if let Some(index) = tail.find(part) {
                values.push(&tail[..index]);
                tail = &tail[index + part.len()..];
            } else {
                matched = false;
                break;
            }
        }
        if matched && tail.is_empty() {
            return format(cx, key, &values);
        }
    }
    text.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_keeps_template_arguments_and_language_fallback() {
        for (key, value) in ENGLISH.iter() {
            assert!(!value.trim().is_empty(), "{key}");
            assert_eq!(
                key.matches("{}").count(),
                value.matches("{}").count(),
                "{key}"
            );
        }
        assert_eq!(Language::from_code("unsupported"), Language::Chinese);
        assert_eq!(
            translate(Language::English, "显示主窗口"),
            "Show main window"
        );
        assert_eq!(translate(Language::Chinese, "主题色"), "主题色");
    }
    #[gpui::test]
    fn diagnostics_preserve_paths_and_user_command_values(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            cx.set_global(Language::English);
            assert_eq!(
                message(cx, "读取配置失败（/tmp/保存.json）：permission denied"),
                "Could not read configuration (/tmp/保存.json): permission denied"
            );
            assert_eq!(
                format(cx, "正在运行：{}", &["保存 {} 中文"]),
                "Running: 保存 {} 中文"
            );
            assert_eq!(
                message(cx, "标签 2 的参数行 3 enabled 必须是布尔值。"),
                "Tab 2, row 3: enabled must be a boolean."
            );
        });
    }
}
