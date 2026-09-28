//! Settings are a draft until Apply/OK, matching the Qt dialog.
use crate::{
    app::CommandWorkspace,
    components::*,
    i18n::{tr, Language},
    theme::{self, Accent, Theme},
    tokens::*,
};
use gpui::component::{
    select::{Select, SelectState},
    IndexPath, Sizable, Size, WindowExt,
};
use gpui::{
    prelude::*, px, App, AppContext, Context, Entity, Render, SharedString, WeakEntity, Window,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub theme: Theme,
    pub accent: Accent,
    pub font_size: u8,
    pub font_weight: u16,
    pub language: Language,
    pub reduced_motion: bool,
}

pub struct SettingsPanel {
    owner: WeakEntity<CommandWorkspace>,
    pub(crate) theme: Entity<SelectState<Vec<SharedString>>>,
    pub(crate) accent: Entity<SelectState<Vec<SharedString>>>,
    pub(crate) size: Entity<SelectState<Vec<SharedString>>>,
    pub(crate) weight: Entity<SelectState<Vec<SharedString>>>,
    pub(crate) language: Entity<SelectState<Vec<SharedString>>>,
    pub(crate) motion: Entity<SelectState<Vec<SharedString>>>,
    message: String,
}

fn choice(
    items: Vec<SharedString>,
    index: usize,
    w: &mut Window,
    cx: &mut App,
) -> Entity<SelectState<Vec<SharedString>>> {
    cx.new(|cx| SelectState::new(items, Some(IndexPath::new(index)), w, cx))
}

impl SettingsPanel {
    pub fn new(
        owner: WeakEntity<CommandWorkspace>,
        p: Preferences,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            owner,
            theme: choice(
                ["深色", "浅色"].map(|s| tr(cx, s).into()).to_vec(),
                usize::from(p.theme == Theme::Light),
                w,
                cx,
            ),
            accent: choice(
                Accent::ALL
                    .iter()
                    .map(|a| tr(cx, a.label()).into())
                    .collect(),
                Accent::ALL.iter().position(|a| *a == p.accent).unwrap_or(0),
                w,
                cx,
            ),
            size: choice(
                (10..=24).map(|n| format!("{n} px").into()).collect(),
                (p.font_size - 10) as usize,
                w,
                cx,
            ),
            weight: choice(
                theme::FONT_WEIGHTS
                    .iter()
                    .map(|n| format!("{} ({n})", tr(cx, theme::weight_label(*n))).into())
                    .collect(),
                theme::FONT_WEIGHTS
                    .iter()
                    .position(|n| *n == p.font_weight)
                    .unwrap_or(1),
                w,
                cx,
            ),
            language: choice(
                vec!["简体中文".into(), "English".into()],
                usize::from(p.language == Language::English),
                w,
                cx,
            ),
            motion: choice(
                ["标准动效", "减少动效"].map(|s| tr(cx, s).into()).to_vec(),
                usize::from(p.reduced_motion),
                w,
                cx,
            ),
            message: String::new(),
        }
    }

    fn preferences(&self, cx: &App) -> Preferences {
        let index = |state: &Entity<SelectState<Vec<SharedString>>>| {
            state.read(cx).selected_index(cx).map_or(0, |ix| ix.row)
        };
        Preferences {
            theme: if index(&self.theme) == 0 {
                Theme::Dark
            } else {
                Theme::Light
            },
            accent: Accent::ALL[index(&self.accent)],
            font_size: index(&self.size) as u8 + 10,
            font_weight: theme::FONT_WEIGHTS[index(&self.weight)],
            language: if index(&self.language) == 0 {
                Language::Chinese
            } else {
                Language::English
            },
            reduced_motion: index(&self.motion) == 1,
        }
    }

    fn apply(&mut self, close: bool, w: &mut Window, cx: &mut Context<Self>) {
        let p = self.preferences(cx);
        let result = self.owner.update(cx, |v, cx| v.apply_preferences(p, w, cx));
        match result {
            Ok(Ok(())) => {
                self.message = "设置已应用".into();
                for (state, labels, index) in [
                    (
                        &self.theme,
                        vec![tr(cx, "深色").into(), tr(cx, "浅色").into()],
                        usize::from(p.theme == Theme::Light),
                    ),
                    (
                        &self.accent,
                        Accent::ALL
                            .iter()
                            .map(|a| tr(cx, a.label()).into())
                            .collect(),
                        Accent::ALL.iter().position(|a| *a == p.accent).unwrap_or(0),
                    ),
                    (
                        &self.weight,
                        theme::FONT_WEIGHTS
                            .iter()
                            .map(|n| format!("{} ({n})", tr(cx, theme::weight_label(*n))).into())
                            .collect(),
                        theme::FONT_WEIGHTS
                            .iter()
                            .position(|n| *n == p.font_weight)
                            .unwrap_or(1),
                    ),
                    (
                        &self.motion,
                        vec![tr(cx, "标准动效").into(), tr(cx, "减少动效").into()],
                        usize::from(p.reduced_motion),
                    ),
                ] {
                    state.update(cx, |s, cx| {
                        s.set_items(labels, w, cx);
                        s.set_selected_index(Some(IndexPath::new(index)), w, cx);
                    });
                }
                if close {
                    w.close_dialog(cx);
                }
            }
            Ok(Err(e)) => self.message = e,
            Err(e) => self.message = e.to_string(),
        }
        cx.notify();
    }
}

impl Render for SettingsPanel {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let field =
            |id: &'static str, label: &str, state: &Entity<SelectState<Vec<SharedString>>>| {
                row()
                    .child(gpui::div().w(px(108.)).flex_shrink_0().child(tr(cx, label)))
                    .child(
                        Select::new(state)
                            .id(id)
                            .w_full()
                            .h(px(CONTROL))
                            .with_size(Size::Medium)
                            .menu_max_h(px(240.)),
                    )
            };
        let backups = self.owner.clone();
        let import = self.owner.clone();
        let export = self.owner.clone();
        let copy = self.owner.clone();
        column()
            .gap(px(GAP))
            .text_size(px(14.))
            .font_weight(gpui::FontWeight::NORMAL)
            .child(
                frame("settings-body")
                    .max_h((w.viewport_size().height - px(260.)).max(px(160.)))
                    .overflow_y_scroll()
                    .child(
                        column()
                            .gap(px(GAP))
                            .child(field("settings-theme", "主题", &self.theme))
                            .child(field("settings-accent", "主题色", &self.accent))
                            .child(field("settings-font-size", "界面字号", &self.size))
                            .child(field("settings-font-weight", "字重", &self.weight))
                            .child(field("settings-language", "语言", &self.language))
                            .child(field("settings-motion", "动效", &self.motion))
                            .child(tr(cx, "应用后自动保存设置，不会保存尚未保存的命令编辑。"))
                            .child(
                                row()
                                    .flex_wrap()
                                    .child(
                                        button("settings-backups", "备份管理", None, cx).on_click(
                                            move |_, w, cx| {
                                                w.close_dialog(cx);
                                                let _ = backups
                                                    .update(cx, |v, cx| v.open_backups(w, cx));
                                            },
                                        ),
                                    )
                                    .child(
                                        button("settings-import", "导入配置…", None, cx).on_click(
                                            move |_, w, cx| {
                                                w.close_dialog(cx);
                                                let _ = import.update(cx, |v, cx| {
                                                    v.prompt_import(false, w, cx)
                                                });
                                            },
                                        ),
                                    )
                                    .child(
                                        button("settings-export", "导出配置…", None, cx).on_click(
                                            move |_, w, cx| {
                                                let _ = export.update(cx, |v, cx| {
                                                    v.prompt_export(None, w, cx)
                                                });
                                            },
                                        ),
                                    ),
                            )
                            .child(
                                button("settings-copy-json", "复制 Qt 兼容配置 JSON", None, cx)
                                    .on_click(move |_, _, cx| {
                                        let _ = copy.update(cx, |v, cx| v.copy_configuration(cx));
                                    }),
                            ),
                    ),
            )
            .child(
                gpui::div()
                    .text_size(px(12.))
                    .child(crate::i18n::message(cx, &self.message)),
            )
            .child(
                row()
                    .justify_end()
                    .child(
                        button("settings-close", "取消", None, cx)
                            .on_click(|_, w, cx| w.close_dialog(cx)),
                    )
                    .child(
                        button("settings-apply", "应用", None, cx)
                            .on_click(cx.listener(|v, _, w, cx| v.apply(false, w, cx))),
                    )
                    .child(
                        button("settings-ok", "确定", None, cx)
                            .on_click(cx.listener(|v, _, w, cx| v.apply(true, w, cx))),
                    ),
            )
    }
}
