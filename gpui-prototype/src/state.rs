use crate::{components::frame, i18n::tr, tab_icons, theme::palette, tokens::ICON};
use gpui::{
    assets::IconName,
    component::{
        input::{InputEvent, InputState, TextareaState},
        searchable_list::{SearchableListDelegate, SearchableListItem},
        Icon, IndexPath,
    },
    prelude::*,
    px, rgb, AnyElement, App, AppContext, Context, Entity, SharedString, Subscription, Window,
};

/// 以稳定记录 ID 区分同名/同时间的运行；文本只是下拉列表的显示标签。
#[derive(Clone)]
pub struct LogItem {
    pub id: usize,
    pub label: SharedString,
}

impl SearchableListItem for LogItem {
    type Value = usize;
    fn title(&self) -> SharedString {
        self.label.clone()
    }
    fn value(&self) -> &Self::Value {
        &self.id
    }
}

pub struct LogItems(pub Vec<LogItem>);
impl SearchableListDelegate for LogItems {
    type Item = LogItem;
    fn items_count(&self, section: usize) -> usize {
        if section == 0 {
            self.0.len()
        } else {
            0
        }
    }
    fn item(&self, ix: IndexPath) -> Option<&Self::Item> {
        self.0.get(ix.row)
    }
    fn position<V>(&self, value: &V) -> Option<IndexPath>
    where
        Self::Item: SearchableListItem<Value = V>,
        V: PartialEq,
    {
        self.0
            .iter()
            .position(|item| item.value() == value)
            .map(IndexPath::new)
    }
    fn render_item(
        &self,
        ix: IndexPath,
        item: &Self::Item,
        checked: bool,
        _: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        Some(
            menu_row_content(
                ("log-menu-content", ix.row),
                item.label.clone(),
                checked,
                cx,
            )
            .into_any_element(),
        )
    }
}

/// 日志记录下拉列表：32px 行与固定右侧勾选列。
pub fn menu_row_content(
    id: (&'static str, usize),
    title: SharedString,
    checked: bool,
    cx: &App,
) -> impl IntoElement + ParentElement {
    frame(id)
        .flex()
        .w_full()
        .min_w_0()
        .items_center()
        .h(px(24.))
        .child(
            frame((gpui::ElementId::from(id), "title"))
                .flex_1()
                .min_w_0()
                .truncate()
                .child(title),
        )
        .child(
            frame((gpui::ElementId::from(id), "check"))
                .size(px(ICON))
                .flex_shrink_0()
                .child(
                    Icon::new(IconName::Check)
                        .size(px(ICON))
                        .text_color(rgb(palette(cx).text))
                        .when(!checked, |icon| icon.invisible()),
                ),
        )
}
use crate::{app::CommandWorkspace, core::parser::CommandParser};

/// GPUI also accepts pasted complete shell commands and raw extra arguments. In
/// those fields an unquoted HTTP query `&key=value` would otherwise be parsed
/// by cmd/sh as another command. Quote only bare URL tokens; keep genuine shell
/// fragments (`&&`, pipes, redirects) and already quoted URLs unchanged.
fn quote_bare_query_urls(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut double_quoted = false;
    let mut single_quoted = false;
    let mut remaining = text;
    while !remaining.is_empty() {
        if remaining.starts_with('"') && !single_quoted {
            double_quoted = !double_quoted;
            result.push('"');
            remaining = &remaining[1..];
            continue;
        }
        if remaining.starts_with('\'')
            && !double_quoted
            && (single_quoted
                || result
                    .chars()
                    .last()
                    .is_none_or(|ch| ch.is_whitespace() || matches!(ch, '=' | '(')))
        {
            single_quoted = !single_quoted;
            result.push('\'');
            remaining = &remaining[1..];
            continue;
        }
        let url_start = remaining.starts_with("https://") || remaining.starts_with("http://");
        if !double_quoted && !single_quoted && url_start {
            let end = remaining
                .find(|ch: char| ch.is_whitespace() || matches!(ch, '"' | '|' | '<' | '>'))
                .unwrap_or(remaining.len());
            let url = &remaining[..end];
            if url.contains('&') && !url.contains("^&") && !url.contains("&&") {
                result.push('"');
                result.push_str(url);
                result.push('"');
                remaining = &remaining[end..];
                continue;
            }
        }
        let ch = remaining.chars().next().unwrap();
        result.push(ch);
        remaining = &remaining[ch.len_utf8()..];
    }
    result
}

fn enabled_by_default() -> bool {
    true
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct RowData {
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    #[serde(rename = "function")]
    pub option: String,
    #[serde(rename = "parameter")]
    pub value: String,
    #[serde(rename = "comment")]
    pub note: String,
}
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct TabData {
    pub name: String,
    #[serde(rename = "working_dir")]
    pub directory: String,
    pub program: String,
    #[serde(rename = "functions")]
    pub rows: Vec<RowData>,
    #[serde(rename = "other_args")]
    pub other: String,
    pub description: String,
    /// GPUI-only icon choice: built-in name, local SVG path or HTTPS URL. Qt ignores these keys.
    #[serde(rename = "gpui_icon", skip_serializing_if = "Option::is_none")]
    pub icon_source: Option<String>,
    /// Cached validated SVG: allows offline restore after a file moves or a URL goes offline.
    #[serde(rename = "gpui_icon_svg", skip_serializing_if = "Option::is_none")]
    pub icon_svg: Option<String>,
}
impl TabData {
    pub fn example() -> Self {
        Self {
            name: "视频转换".into(),
            program: "ffmpeg".into(),
            rows: vec![
                RowData {
                    enabled: true,
                    option: "-i".into(),
                    value: "input file.mp4".into(),
                    note: "输入视频".into(),
                },
                RowData {
                    enabled: true,
                    option: "-c:v".into(),
                    value: "libx264".into(),
                    note: "视频编码".into(),
                },
                RowData {
                    enabled: false,
                    option: "-crf".into(),
                    value: "23".into(),
                    note: "画面质量，数值越小质量越高".into(),
                },
            ],
            other: "output.mp4".into(),
            ..Default::default()
        }
    }
}

pub struct Parameter {
    pub id: usize,
    pub enabled: bool,
    pub option: Entity<InputState>,
    pub value: Entity<InputState>,
    pub note: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

pub struct CommandTab {
    pub id: usize,
    pub dirty: bool,
    pub icon_source: Option<String>,
    pub icon_svg: Option<String>,
    /// 参数区期望显示的行数随标签页保留；临时空间不足不覆盖此偏好。
    pub(crate) row_height_override: Option<usize>,
    pub name: Entity<InputState>,
    pub directory: Entity<InputState>,
    pub program: Entity<InputState>,
    pub append: Entity<InputState>,
    pub other: Entity<InputState>,
    pub description: Entity<TextareaState>,
    pub rows: Vec<Parameter>,
    _subscriptions: Vec<Subscription>,
}

fn text(
    value: String,
    placeholder: &str,
    window: &mut Window,
    cx: &mut Context<CommandWorkspace>,
) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .default_value(value)
            .placeholder(tr(cx, placeholder))
    })
}
fn watch(
    input: &Entity<InputState>,
    tab: usize,
    window: &mut Window,
    cx: &mut Context<CommandWorkspace>,
) -> Subscription {
    cx.subscribe_in(input, window, move |view, _, event, window, cx| {
        if matches!(event, InputEvent::Change) {
            view.changed(tab, window, cx);
        }
    })
}
impl Parameter {
    pub fn new(
        id: usize,
        tab: usize,
        data: RowData,
        window: &mut Window,
        cx: &mut Context<CommandWorkspace>,
    ) -> Self {
        let option = text(data.option, "选项 / 功能", window, cx);
        let value = text(data.value, "参数值", window, cx);
        let note = text(data.note, "备注（不参与执行）", window, cx);
        let subscriptions = [&option, &value, &note]
            .iter()
            .map(|s| watch(s, tab, window, cx))
            .collect();
        Self {
            id,
            enabled: data.enabled,
            option,
            value,
            note,
            _subscriptions: subscriptions,
        }
    }
    pub fn data(&self, cx: &App) -> RowData {
        RowData {
            enabled: self.enabled,
            option: self.option.read(cx).value().to_string(),
            value: self.value.read(cx).value().to_string(),
            note: self.note.read(cx).value().to_string(),
        }
    }
}
impl CommandTab {
    pub fn translate_placeholders(&self, w: &mut Window, cx: &mut Context<CommandWorkspace>) {
        for (input, label) in [
            (&self.name, "给这条命令起个名字"),
            (&self.directory, "为空则使用程序当前目录"),
            (&self.program, "输入程序路径，或粘贴完整命令后解析"),
            (&self.other, "其他参数 / 重定向 / 管道（按原文追加）"),
            (&self.append, "仅解析 · 粘贴命令"),
        ] {
            input.update(cx, |s, cx| s.set_placeholder(tr(cx, label), w, cx));
        }
        for row in &self.rows {
            for (input, label) in [
                (&row.option, "选项 / 功能"),
                (&row.value, "参数值"),
                (&row.note, "备注（不参与执行）"),
            ] {
                input.update(cx, |s, cx| s.set_placeholder(tr(cx, label), w, cx));
            }
        }
        self.description.update(cx, |s, cx| {
            s.set_placeholder(tr(cx, "描述 / 使用说明"), w, cx)
        });
    }

    pub fn new(
        id: usize,
        data: TabData,
        window: &mut Window,
        cx: &mut Context<CommandWorkspace>,
    ) -> Self {
        let icon_source = data.icon_source.clone();
        let icon_svg = data
            .icon_svg
            .as_deref()
            .and_then(|s| tab_icons::validate_svg(s.as_bytes()).ok());
        let name = text(data.name, "给这条命令起个名字", window, cx);
        let directory = text(data.directory, "为空则使用程序当前目录", window, cx);
        let program = text(
            data.program,
            "输入程序路径，或粘贴完整命令后解析",
            window,
            cx,
        );
        let other = text(
            data.other,
            "其他参数 / 重定向 / 管道（按原文追加）",
            window,
            cx,
        );
        let append = text(String::new(), "仅解析 · 粘贴命令", window, cx);
        let description = cx.new(|cx| {
            TextareaState::new(window, cx)
                .default_value(data.description)
                .placeholder(tr(cx, "描述 / 使用说明"))
        });
        let mut subscriptions: Vec<_> = [&name, &directory, &program, &other]
            .iter()
            .map(|s| watch(s, id, window, cx))
            .collect();
        subscriptions.push(cx.subscribe_in(
            &description,
            window,
            move |view, _, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    view.changed(id, window, cx);
                }
            },
        ));
        let rows = data
            .rows
            .into_iter()
            .enumerate()
            .map(|(i, row)| Parameter::new(i, id, row, window, cx))
            .collect();
        Self {
            id,
            dirty: false,
            icon_source,
            icon_svg,
            row_height_override: None,
            name,
            directory,
            program,
            append,
            other,
            description,
            rows,
            _subscriptions: subscriptions,
        }
    }
    pub fn label(&self, cx: &App) -> String {
        let name = self.name.read(cx).value();
        if name.trim().is_empty() {
            tr(cx, "未命名")
        } else {
            name.to_string()
        }
    }
    pub fn command(&self, cx: &App) -> String {
        let program = self.program.read(cx).value();
        if program.trim().is_empty() {
            return String::new();
        }
        let parameters = self
            .rows
            .iter()
            .filter(|p| p.enabled)
            .map(|p| {
                (
                    p.option.read(cx).value().to_string(),
                    p.value.read(cx).value().to_string(),
                )
            })
            .collect::<Vec<_>>();
        CommandParser::build(
            &quote_bare_query_urls(&program),
            &parameters,
            &quote_bare_query_urls(&self.other.read(cx).value()),
        )
    }
    pub fn data(&self, cx: &App) -> TabData {
        TabData {
            name: self.name.read(cx).value().to_string(),
            directory: self.directory.read(cx).value().to_string(),
            program: self.program.read(cx).value().to_string(),
            rows: self.rows.iter().map(|r| r.data(cx)).collect(),
            other: self.other.read(cx).value().to_string(),
            description: self.description.read(cx).value().to_string(),
            icon_source: self.icon_source.clone(),
            icon_svg: self.icon_svg.clone(),
        }
    }
}

#[cfg(test)]
mod query_url_tests {
    use super::*;

    #[test]
    fn pasted_stream_url_is_one_shell_argument_without_changing_real_operators() {
        let url =
            "https://example.invalid/stream?x=1&browser_version=123&cdm=widevine&os_name=windows";
        let program = format!("N_m3u8DL-RE {url} --live-real-time-merge");
        assert_eq!(
            quote_bare_query_urls(&program),
            format!("N_m3u8DL-RE \"{url}\" --live-real-time-merge")
        );
        assert_eq!(
            quote_bare_query_urls(&format!("--url={url} --save-name video")),
            format!("--url=\"{url}\" --save-name video")
        );
        assert_eq!(
            quote_bare_query_urls(&format!("tool \"{url}\"")),
            format!("tool \"{url}\"")
        );
        assert_eq!(
            quote_bare_query_urls(&format!("tool '{url}'")),
            format!("tool '{url}'"),
            "不改写用户明确输入的引号语法"
        );
        assert_eq!(
            quote_bare_query_urls(&format!("tool --name don't {url}")),
            format!("tool --name don't \"{url}\""),
            "普通词内的撇号不能让后续网址脱离保护"
        );
        assert_eq!(
            quote_bare_query_urls("echo first && echo second | more"),
            "echo first && echo second | more"
        );
        assert_eq!(
            quote_bare_query_urls("tool https://example.invalid/a?x=1^&y=2"),
            "tool https://example.invalid/a?x=1^&y=2"
        );
        // Preview/launch use the same protected command; a raw shell operator
        // elsewhere in the command must still be available to deliberate scripts.
        let preview = CommandParser::build(&quote_bare_query_urls(&program), &[], "");
        assert!(preview.contains(&format!("\"{url}\"")), "{preview}");
        assert!(CommandParser::build("echo", &[], "hello | more").contains(" | more"));
    }

    #[test]
    fn pasted_stream_url_reaches_a_local_process_as_one_argument() {
        use crate::backend::{self, OutputStream, RunEvent};
        use std::time::{Duration, Instant};

        let url =
            "https://example.invalid/watch?x=1&browser_version=123&cdm=widevine&os_name=windows";
        let python = if cfg!(windows) { "python" } else { "python3" };
        let program = format!(r#"{python} -c "import sys; print(sys.argv[1])" {url}"#);
        let command = CommandParser::build(&quote_bare_query_urls(&program), &[], "");
        let (_handle, events) = backend::start_shell(&command, "").unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let (mut stdout, mut stderr, mut exit) = (String::new(), String::new(), None);
        while Instant::now() < deadline {
            if let Ok(event) = events.recv_timeout(Duration::from_millis(100)) {
                match event {
                    RunEvent::Output {
                        stream: OutputStream::Stdout,
                        text,
                    } => stdout.push_str(&text),
                    RunEvent::Output {
                        stream: OutputStream::Stderr,
                        text,
                    } => stderr.push_str(&text),
                    RunEvent::Finished { exit_code, .. } => {
                        exit = Some(exit_code);
                        break;
                    }
                }
            }
        }
        assert_eq!(
            exit,
            Some(0),
            "命令应完成而非将查询字段当成 shell 命令：{stderr:?}"
        );
        assert_eq!(stdout.trim(), url, "必须将完整 URL 作为同一个参数传入");
        assert!(
            stderr.trim().is_empty(),
            "查询字段不应成为独立命令：{stderr:?}"
        );
    }
}
