//! 使用 Kit 的真实布局 / Metal 离屏渲染，不用“编译成功”代替 UI 验收。
use crate::{
    app::CommandWorkspace,
    app_assets::AppAssets,
    tab_icons,
    theme::{self, Theme},
};
use gpui::component::{Root, WindowExt};
use gpui::test::TestWindowExt;
use gpui::{px, size, AppContext, HeadlessAppContext};
use std::{collections::BTreeMap, sync::Arc};

fn fixture(
    rows: usize,
    window: &mut gpui::Window,
    cx: &mut gpui::Context<CommandWorkspace>,
) -> CommandWorkspace {
    use crate::state::{CommandTab, TabData};
    let mut view = CommandWorkspace::new(window, cx);
    let mut data = TabData::example();
    let examples = data.rows.clone();
    data.rows = (0..rows)
        .map(|i| examples[i % examples.len()].clone())
        .collect();
    view.tabs[0] = CommandTab::new(0, data, window, cx);
    view.select_tab(0, window, cx);
    view
}

fn geometry(window: &gpui::Window) -> BTreeMap<String, [f32; 4]> {
    let mut geometry = BTreeMap::new();
    for id in [
        "workspace",
        "command-editor",
        "command-form",
        "command-form-divider",
        "command-extras",
        "other-args-cell",
        "append-fragment",
        "append-input-cell",
        "append-only-hint",
        "status-bar",
        "program",
        "command-name",
        "working-directory",
        "parse-command",
        "browse-directory",
        "append-command",
        "append-parse",
        "other-args",
        "description-frame",
        "parameter-header",
        "parameter-list",
        "tab-strip",
        "tab-sidebar",
        "sidebar-resize-handle",
        "sidebar-items",
        "collapse-sidebar",
        "sidebar-add-tab",
        "preview-header",
        "description-header",
        "parameter-items",
        "parameter-actions",
        "add-parameter",
        "enabled-first",
        "select-all",
        "select-none",
        "option-heading",
        "value-heading",
        "note-heading",
        "preview-frame",
        "preview-splitter",
        "run-actions",
        "run-command",
        "copy-command",
        "app-brand",
        "application-menu",
        "previous-tab",
        "add-tab-title",
        "next-tab",
        "title-drag-space",
        "reload",
        "settings",
        "save",
        "theme-toggle",
        "log-dock",
        "log-select-frame",
        "log-selector",
        "detach-log",
        "delete-log",
        "close-log",
    ] {
        if let Some(item) = window.try_find(id) {
            let b = item.bounds();
            geometry.insert(
                id.to_string(),
                [
                    b.origin.x.into(),
                    b.origin.y.into(),
                    b.size.width.into(),
                    b.size.height.into(),
                ],
            );
        }
    }
    for (id, name) in [
        ("option", "option-row"),
        ("value", "value-row"),
        ("note", "note-row"),
        ("remove-row", "remove-row"),
    ] {
        if let Some(item) = window.try_find((id, 0usize)) {
            let b = item.bounds();
            geometry.insert(
                name.to_string(),
                [
                    b.origin.x.into(),
                    b.origin.y.into(),
                    b.size.width.into(),
                    b.size.height.into(),
                ],
            );
        }
    }
    geometry
}

fn closed_sidebar_snapshots(directory: &str, cx: &mut HeadlessAppContext) {
    for mode in [Theme::Light, Theme::Dark] {
        cx.update(|cx| {
            theme::apply(mode, cx);
            theme::set_font_size(14, cx);
            crate::i18n::apply(crate::i18n::Language::Chinese, cx);
        });
        let handle = cx
            .open_window(size(px(850.), px(800.)), |w, cx| {
                let view = cx.new(|cx| fixture(3, w, cx));
                cx.new(|cx| Root::new(view, w, cx))
            })
            .unwrap();
        let icons = cx
            .update_window(handle.into(), |_, w, cx| {
                w.click("add-tab-title", cx);
                w.click(("close-tab", 1usize), cx);
                w.click(("close-tab", 0usize), cx);
                w.render_frame(cx);
                assert!(w.try_find("empty-editor").is_some());
                assert!(w.try_find(("sidebar-tab", 0usize)).is_some());
                assert!(w.try_find(("sidebar-tab", 1usize)).is_some());
                (0usize..2)
                    .map(|id| w.find(("sidebar-icon", id)).bounds())
                    .collect::<Vec<_>>()
            })
            .unwrap();
        let image = cx
            .capture_screenshot(handle.into())
            .expect("全关标签的侧栏截图失败");
        let scale = image.width() as f32 / 850.;
        let foreground = cx.update(|cx| theme::palette(cx).text);
        let expected = [
            ((foreground >> 16) & 255) as u8,
            ((foreground >> 8) & 255) as u8,
            (foreground & 255) as u8,
        ];
        for (index, box_) in icons.into_iter().enumerate() {
            let x0 = (f32::from(box_.left()) * scale) as u32;
            let x1 = (f32::from(box_.right()) * scale) as u32;
            let y0 = (f32::from(box_.top()) * scale) as u32;
            let y1 = (f32::from(box_.bottom()) * scale) as u32;
            let matching = (y0..y1)
                .flat_map(|y| (x0..x1).map(move |x| (x, y)))
                .filter(|(x, y)| {
                    let pixel = image.get_pixel(*x, *y).0;
                    (0..3).all(|channel| pixel[channel].abs_diff(expected[channel]) <= 16)
                })
                .count();
            assert!(
                matching >= 12,
                "{mode:?}/配置{index}: 关闭全部编辑器后图标仍应使用普通文字色，实际{matching}px"
            );
        }
        image
            .save(format!("{directory}/sidebar-no-open-{mode:?}.png"))
            .unwrap();
        cx.update_window(handle.into(), |_, w, _| w.remove_window())
            .unwrap();
        println!("PASS sidebar-no-open-{mode:?}");
    }
}

fn sidebar_resize_snapshots(directory: &str, cx: &mut HeadlessAppContext) {
    use std::time::Duration;
    for mode in [Theme::Dark, Theme::Light] {
        cx.update(|cx| {
            theme::apply(mode, cx);
            theme::set_font_size(14, cx);
        });
        let handle = cx
            .open_window(size(px(850.), px(800.)), |w, cx| {
                let view = cx.new(|cx| fixture(3, w, cx));
                cx.new(|cx| Root::new(view, w, cx))
            })
            .unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            let grip = w.find("sidebar-resize-handle").bounds().center();
            w.drag(grip, gpui::point(grip.x + px(80.), grip.y), cx);
        })
        .unwrap();
        cx.background_executor()
            .advance_clock(Duration::from_millis(900));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            assert!((w.find("tab-sidebar").bounds().size.width - px(288.)).abs() <= px(1.));
            assert!(w.try_find("sidebar-motion-edge").is_none());
            let grip = w.find("sidebar-resize-handle").bounds();
            let sidebar = w.find("tab-sidebar").bounds();
            assert!((grip.right() - sidebar.right()).abs() <= px(1.));
            assert_eq!(grip.size.width, px(10.));
        })
        .unwrap();
        cx.capture_screenshot(handle.into())
            .unwrap()
            .save(format!("{directory}/sidebar-resized-{mode:?}.png"))
            .unwrap();
        cx.update_window(handle.into(), |_, w, _| w.remove_window())
            .unwrap();
        println!("PASS sidebar-resized-{mode:?}");
    }
}

// The native drag surface is painted by the platform separately from the window screenshot.
// Render the exact same preview view directly to assert its actual foreground/background colors.
fn parameter_drag_preview_snapshots(directory: &str, cx: &mut HeadlessAppContext) {
    use crate::app::RowDragPreview;
    for mode in [Theme::Dark, Theme::Light] {
        cx.update(|cx| {
            theme::apply(mode, cx);
            theme::set_font_size(14, cx);
        });
        let handle = cx
            .open_window(size(px(360.), px(120.)), |w, cx| {
                let view = cx.new(|_| RowDragPreview {
                    label: "-i input.mp4".into(),
                });
                cx.new(|cx| Root::new(view, w, cx))
            })
            .unwrap();
        let bounds = cx
            .update_window(handle.into(), |_, w, cx| {
                w.render_frame(cx);
                w.find("parameter-drag-preview").bounds()
            })
            .unwrap();
        let image = cx.capture_screenshot(handle.into()).unwrap();
        let scale = image.width() as f32 / 360.;
        let x0 = (f32::from(bounds.left()) * scale).round().max(0.) as u32;
        let x1 = (f32::from(bounds.right()) * scale)
            .round()
            .min(image.width() as f32) as u32;
        let y0 = (f32::from(bounds.top()) * scale).round().max(0.) as u32;
        let y1 = (f32::from(bounds.bottom()) * scale)
            .round()
            .min(image.height() as f32) as u32;
        let (text, background) =
            cx.update(|cx| (theme::palette(cx).text, theme::palette(cx).button));
        let matches = |rgb: u32, pixel: &[u8; 4]| {
            [16, 8, 0]
                .into_iter()
                .enumerate()
                .all(|(i, shift)| pixel[i].abs_diff(((rgb >> shift) & 255) as u8) <= 16)
        };
        let mut text_count = 0;
        let mut bg_count = 0;
        let text_start = x0 + ((crate::tokens::GAP * 2. + crate::tokens::ICON) * scale) as u32;
        for y in y0..y1 {
            for x in x0..x1 {
                let pixel = image.get_pixel(x, y).0;
                // Exclude the drag-handle icon: the label itself must use theme text color.
                if x >= text_start {
                    text_count += usize::from(matches(text, &pixel));
                }
                bg_count += usize::from(matches(background, &pixel));
            }
        }
        assert!(
            text_count > 20 && bg_count > 20,
            "{mode:?} 拖动预览文字/背景必须可见: text={text_count}, bg={bg_count}"
        );
        image
            .save(format!("{directory}/drag-preview-{mode:?}.png"))
            .unwrap();
        cx.update_window(handle.into(), |_, w, _| w.remove_window())
            .unwrap();
        println!("PASS drag-preview-{mode:?}");
    }
}

fn compact_note_hover_snapshots(directory: &str, cx: &mut HeadlessAppContext) {
    use std::time::Duration;
    for mode in [Theme::Dark, Theme::Light] {
        cx.update(|cx| {
            theme::apply(mode, cx);
            theme::set_font_size(14, cx);
            crate::i18n::apply(crate::i18n::Language::Chinese, cx);
        });
        let handle = cx
            .open_window(size(px(850.), px(800.)), |w, cx| {
                let view = cx.new(|cx| fixture(3, w, cx));
                cx.new(|cx| Root::new(view, w, cx))
            })
            .unwrap();
        let button = cx
            .update_window(handle.into(), |_, w, cx| {
                w.render_frame(cx);
                assert_eq!(w.find(("note-details", 0usize)).label(), Some("输入视频"));
                w.find(("note-details", 0usize)).bounds()
            })
            .unwrap();
        let before = cx.capture_screenshot(handle.into()).unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            w.hover(("note-details", 0usize), cx)
        })
        .unwrap();
        cx.background_executor()
            .advance_clock(Duration::from_millis(650));
        cx.run_until_parked();
        cx.background_executor()
            .advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| w.render_frame(cx))
            .unwrap();
        let after = cx.capture_screenshot(handle.into()).unwrap();
        let scale = after.width() as f32 / 850.;
        let bx0 = (f32::from(button.left()) * scale).floor() as u32;
        let bx1 = (f32::from(button.right()) * scale).ceil() as u32;
        let by0 = (f32::from(button.top()) * scale).floor() as u32;
        let by1 = (f32::from(button.bottom()) * scale).ceil() as u32;
        let changed_outside_button = (0..after.height())
            .flat_map(|y| (0..after.width()).map(move |x| (x, y)))
            .filter(|(x, y)| !(*x >= bx0 && *x < bx1 && *y >= by0 && *y < by1))
            .filter(|(x, y)| before.get_pixel(*x, *y) != after.get_pixel(*x, *y))
            .count();
        assert!(
            changed_outside_button > 200,
            "{mode:?} 备注悬停应弹出内容，而不只是按钮变色: {changed_outside_button}px"
        );
        after
            .save(format!("{directory}/note-hover-{mode:?}.png"))
            .unwrap();
        cx.update_window(handle.into(), |_, w, _| w.remove_window())
            .unwrap();
        println!("PASS note-hover-{mode:?}");
    }
}

fn heading_text_snapshots(directory: &str, cx: &mut HeadlessAppContext) {
    use crate::components::frame;
    use crate::tokens::{COMMAND_NAME_HEIGHT, COMMAND_NAME_LINE_HEIGHT, FIELD_PADDING};
    use gpui::{prelude::*, px, rgb, Context, FontWeight, Render};

    const TEXT: &str = "中文标题 Ågjpq";
    struct GlyphReference {
        width: gpui::Pixels,
    }
    impl Render for GlyphReference {
        fn render(&mut self, _: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
            let p = theme::palette(cx);
            frame("heading-glyph-reference")
                .w(self.width)
                .h(px(COMMAND_NAME_HEIGHT))
                .flex()
                .items_center()
                .px(px(FIELD_PADDING + 1.))
                .bg(rgb(p.panel))
                .text_color(rgb(p.text))
                .font_family(cx.global::<theme::Fonts>().body.clone())
                .text_size(px(32.))
                .line_height(px(COMMAND_NAME_LINE_HEIGHT))
                .font_weight(FontWeight::BOLD)
                .child(TEXT)
        }
    }
    for mode in [Theme::Dark, Theme::Light] {
        cx.update(|cx| theme::apply(mode, cx));
        let handle = cx
            .open_window(size(px(850.), px(800.)), |w, cx| {
                let view = cx.new(|cx| {
                    let mut view = fixture(3, w, cx);
                    view.font_size = 24;
                    theme::set_font_size(24, cx);
                    view.tabs[0]
                        .name
                        .update(cx, |state, cx| state.set_value(TEXT, w, cx));
                    view
                });
                cx.new(|cx| Root::new(view, w, cx))
            })
            .unwrap();
        let bounds = cx
            .update_window(handle.into(), |_, w, cx| {
                w.click("program", cx); // No title caret or selection in the glyph comparison.
                w.render_frame(cx);
                w.find("command-name").bounds()
            })
            .unwrap();
        let actual = cx.capture_screenshot(handle.into()).unwrap();
        let reference = cx
            .open_window(size(px(850.), px(200.)), |w, cx| {
                let view = cx.new(|_| GlyphReference {
                    width: bounds.size.width,
                });
                cx.new(|cx| Root::new(view, w, cx))
            })
            .unwrap();
        let reference_bounds = cx
            .update_window(reference.into(), |_, w, cx| {
                w.render_frame(cx);
                w.find("heading-glyph-reference").bounds()
            })
            .unwrap();
        let expected = cx.capture_screenshot(reference.into()).unwrap();
        let foreground = cx.update(|cx| theme::palette(cx).text);
        let [actual_pixels, expected_pixels] = [(&actual, bounds), (&expected, reference_bounds)]
            .map(|(image, bounds)| {
                let scale = image.width() as f32 / 850.;
                let x0 = (f32::from(bounds.left()) * scale).ceil() as u32;
                let x1 = (f32::from(bounds.right()) * scale).floor() as u32;
                let y0 = (f32::from(bounds.top()) * scale).ceil() as u32;
                let y1 = (f32::from(bounds.bottom()) * scale).floor() as u32;
                (y0..y1)
                    .flat_map(|y| (x0..x1).map(move |x| image.get_pixel(x, y).0))
                    .filter(|pixel| {
                        [16, 8, 0].into_iter().enumerate().all(|(i, shift)| {
                            pixel[i].abs_diff(((foreground >> shift) & 255) as u8) <= 16
                        })
                    })
                    .count()
            });
        assert!(expected_pixels > 100, "参考标题必须包含可见字形");
        assert!(
            actual_pixels as f32 >= expected_pixels as f32 * 0.95,
            "{mode:?} 大标题不得裁去上下字形：{actual_pixels} / {expected_pixels}"
        );
        actual
            .save(format!("{directory}/heading-large-{mode:?}.png"))
            .unwrap();
        cx.update_window(reference.into(), |_, w, _| w.remove_window())
            .unwrap();
        cx.update_window(handle.into(), |_, w, _| w.remove_window())
            .unwrap();
        println!("PASS heading-large-{mode:?}");
    }
}

fn focus_snapshots(directory: &str, cx: &mut HeadlessAppContext) {
    use std::time::Duration;
    for mode in [Theme::Dark, Theme::Light] {
        for target in ["command-name", "program", "description-frame"] {
            cx.update(|cx| theme::apply(mode, cx));
            let handle = cx
                .open_window(size(px(850.), px(800.)), |w, cx| {
                    let view = cx.new(|cx| fixture(3, w, cx));
                    cx.new(|cx| Root::new(view, w, cx))
                })
                .unwrap();
            let (before, bounds) = cx
                .update_window(handle.into(), |_, w, cx| {
                    w.render_frame(cx);
                    (geometry(w), w.find(target).bounds())
                })
                .unwrap();
            let mut center_colors = Vec::new();
            let mut left_colors = Vec::new();
            let mut right_colors = Vec::new();
            for phase in [
                "idle",
                "middle",
                "focused",
                "blur-middle",
                "blurred",
                "reduced",
            ] {
                if phase == "middle" || phase == "reduced" {
                    cx.update_window(handle.into(), |_, w, cx| {
                        if phase == "reduced" {
                            theme::set_reduced_motion(true, cx);
                        }
                        w.click(target, cx);
                    })
                    .unwrap();
                } else if phase == "blur-middle" {
                    cx.update_window(handle.into(), |_, w, cx| {
                        w.click(
                            if target == "command-name" {
                                "program"
                            } else {
                                "command-name"
                            },
                            cx,
                        )
                    })
                    .unwrap();
                }
                if phase == "middle" || phase == "blur-middle" {
                    cx.background_executor()
                        .advance_clock(Duration::from_millis(40));
                }
                if phase == "focused" || phase == "blurred" {
                    cx.background_executor()
                        .advance_clock(Duration::from_millis(200));
                }
                cx.update_window(handle.into(), |_, w, cx| {
                    w.render_frame(cx);
                    assert_eq!(
                        geometry(w),
                        before,
                        "{mode:?}/{target}/{phase}: 焦点动效不能改变几何"
                    );
                })
                .unwrap();
                let image = cx
                    .capture_screenshot(handle.into())
                    .expect("输入框焦点动效截图失败");
                let scale = image.width() as f32 / 850.;
                let center_x = (f32::from(bounds.center().x) * scale).round() as u32;
                let center_y = (f32::from(bounds.top() - px(2.)) * scale).round() as u32;
                let left_x = (f32::from(bounds.left() - px(2.)) * scale).round() as u32;
                let right_x = (f32::from(bounds.right() + px(2.)) * scale).round() as u32;
                let edge_y = (f32::from(bounds.center().y) * scale).round() as u32;
                center_colors.push(image.get_pixel(center_x, center_y).0);
                left_colors.push(image.get_pixel(left_x, edge_y).0);
                right_colors.push(image.get_pixel(right_x, edge_y).0);
                image
                    .save(format!("{directory}/focus-{mode:?}-{target}-{phase}.png"))
                    .unwrap();
            }
            assert_ne!(center_colors[0], center_colors[1], "{target}: 中心应先亮起");
            for (side, colors) in [("左", &left_colors), ("右", &right_colors)] {
                assert_eq!(colors[0], colors[1], "{target}: {side}端不能和中心同时亮起");
                assert_ne!(colors[0], colors[2], "{target}: 完成后应延伸至{side}端");
                assert_eq!(colors[0], colors[3], "{target}: 收起时应先离开{side}端");
                assert_eq!(colors[0], colors[4], "{target}: 失焦后{side}侧外环应消失");
                assert_eq!(
                    colors[2], colors[5],
                    "{target}: 减少动效应立即显示{side}侧外环"
                );
            }
            assert_ne!(
                center_colors[0], center_colors[3],
                "{target}: 收起过程中中心仍可见"
            );
            assert_eq!(
                center_colors[0], center_colors[4],
                "{target}: 失焦后中心应恢复"
            );
            assert_eq!(
                center_colors[2], center_colors[5],
                "{target}: 减少动效应立即显示中心"
            );
            println!("FOCUS {mode:?}/{target}: center={center_colors:?}, left={left_colors:?}, right={right_colors:?}");
            cx.update_window(handle.into(), |_, w, _| w.remove_window())
                .unwrap();
        }
    }
}

fn sidebar_icon_hover_snapshots(directory: &str, cx: &mut HeadlessAppContext) {
    for mode in [Theme::Dark, Theme::Light] {
        cx.update(|cx| {
            theme::apply(mode, cx);
            theme::set_reduced_motion(true, cx);
        });
        let handle = cx
            .open_window(size(px(850.), px(800.)), |w, cx| {
                let view = cx.new(|cx| {
                    let mut view = CommandWorkspace::new(w, cx);
                    view.reduced_motion = true;
                    view
                });
                cx.new(|cx| Root::new(view, w, cx))
            })
            .unwrap();
        cx.update_window(handle.into(), |_, w, cx| w.click("add-tab-title", cx))
            .unwrap();
        for id in [0usize, 1] {
            let bounds = cx
                .update_window(handle.into(), |_, w, cx| {
                    w.hover(("sidebar-tab", id), cx);
                    w.find(("sidebar-icon", id)).bounds()
                })
                .unwrap();
            let before = cx.capture_screenshot(handle.into()).unwrap();
            cx.update_window(handle.into(), |_, w, cx| w.hover(("sidebar-icon", id), cx))
                .unwrap();
            let after = cx.capture_screenshot(handle.into()).unwrap();
            let scale = after.width() as f32 / 850.;
            let x0 = (f32::from(bounds.left()) * scale).round() as u32;
            let y0 = (f32::from(bounds.top()) * scale).round() as u32;
            let x1 = (f32::from(bounds.right()) * scale).round() as u32;
            let y1 = (f32::from(bounds.bottom()) * scale).round() as u32;
            let changed = (y0..y1)
                .flat_map(|y| (x0..x1).map(move |x| (x, y)))
                .filter(|(x, y)| before.get_pixel(*x, *y) != after.get_pixel(*x, *y))
                .count();
            assert!(
                changed > 100,
                "{mode:?}/{id}: 图标悬停高光必须明显可见，变化像素={changed}"
            );
            after
                .save(format!("{directory}/sidebar-icon-hover-{mode:?}-{id}.png"))
                .unwrap();
        }
        cx.update_window(handle.into(), |_, w, _| w.remove_window())
            .unwrap();
    }
}

fn top_tab_drag_visual_snapshots(directory: &str, cx: &mut HeadlessAppContext) {
    use crate::app::TopTabDragPreview;
    use gpui::{InputEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent};
    for mode in [Theme::Dark, Theme::Light] {
        cx.update(|cx| {
            theme::apply(mode, cx);
            theme::set_reduced_motion(true, cx);
        });
        let pill = cx
            .open_window(size(px(360.), px(100.)), |w, cx| {
                let view = cx.new(|_| TopTabDragPreview {
                    label: "示例标签".into(),
                });
                cx.new(|cx| Root::new(view, w, cx))
            })
            .unwrap();
        let pill_bounds = cx
            .update_window(pill.into(), |_, w, cx| {
                w.render_frame(cx);
                let bounds = w.find("top-tab-drag-preview").bounds();
                assert_eq!(bounds.size.height, px(32.));
                assert!(bounds.size.width > px(80.));
                bounds
            })
            .unwrap();
        let pill_image = cx.capture_screenshot(pill.into()).unwrap();
        let scale = pill_image.width() as f32 / 360.;
        let left = (f32::from(pill_bounds.left()) * scale).round() as u32;
        let mid = (f32::from(pill_bounds.center().x) * scale).round() as u32;
        let top = (f32::from(pill_bounds.top()) * scale).round() as u32;
        assert_ne!(
            pill_image.get_pixel(left + 2, top + 2),
            pill_image.get_pixel(mid, top + 2),
            "{mode:?}: 拖动预览的四角必须留空，保持药丸形状"
        );
        pill_image
            .save(format!("{directory}/top-tab-pill-{mode:?}.png"))
            .unwrap();
        cx.update_window(pill.into(), |_, w, _| w.remove_window())
            .unwrap();

        let handle = cx
            .open_window(size(px(850.), px(800.)), |w, cx| {
                let view = cx.new(|cx| {
                    let mut view = CommandWorkspace::new(w, cx);
                    view.reduced_motion = true;
                    view
                });
                cx.new(|cx| Root::new(view, w, cx))
            })
            .unwrap();
        cx.update_window(handle.into(), |_, w, cx| w.click("add-tab-title", cx))
            .unwrap();
        let (from, to) = cx
            .update_window(handle.into(), |_, w, cx| {
                w.render_frame(cx);
                (
                    w.find(("tab-context", 0usize)).bounds().center(),
                    w.find(("tab-context", 1usize)).bounds().center(),
                )
            })
            .unwrap();
        let marker = cx
            .update_window(handle.into(), |_, w, cx| {
                w.dispatch_event(
                    MouseMoveEvent {
                        position: from,
                        pressed_button: None,
                        modifiers: Default::default(),
                    }
                    .to_platform_input(),
                    cx,
                );
                w.render_frame(cx);
                w.dispatch_event(
                    MouseDownEvent {
                        button: MouseButton::Left,
                        position: from,
                        modifiers: Default::default(),
                        click_count: 1,
                        first_mouse: false,
                    }
                    .to_platform_input(),
                    cx,
                );
                w.render_frame(cx);
                for step in 1..=8 {
                    let fraction = step as f32 / 8.;
                    let pos = gpui::point(
                        from.x + (to.x - from.x) * fraction,
                        from.y + (to.y - from.y) * fraction,
                    );
                    w.dispatch_event(
                        MouseMoveEvent {
                            position: pos,
                            pressed_button: Some(MouseButton::Left),
                            modifiers: Default::default(),
                        }
                        .to_platform_input(),
                        cx,
                    );
                    w.render_frame(cx);
                }
                let marker = w.find(("top-tab-drop", 1usize)).bounds();
                assert!(
                    w.try_find(("top-tab-drop", 0usize)).is_none(),
                    "离开源标签后不可有错误落点"
                );
                assert_eq!(marker.size.width, px(4.), "顶部落点须显示可见的 4px 指示线");
                assert!(marker.size.height >= px(16.), "顶部落点线不能缩成点");
                marker
            })
            .unwrap();
        let image = cx.capture_screenshot(handle.into()).unwrap();
        let scale = image.width() as f32 / 850.;
        let pixel = image
            .get_pixel(
                (f32::from(marker.center().x) * scale).round() as u32,
                (f32::from(marker.center().y) * scale).round() as u32,
            )
            .0;
        let colors = cx.update(|cx| theme::palette(cx));
        let expected = [
            ((colors.selection >> 16) & 255) as u8,
            ((colors.selection >> 8) & 255) as u8,
            (colors.selection & 255) as u8,
        ];
        assert!(
            pixel[..3]
                .iter()
                .zip(expected)
                .all(|(actual, expected)| actual.abs_diff(expected) <= 8),
            "{mode:?}: 选中标签的落点线应使用同色系选区色而非白色: {pixel:?} / {expected:?}"
        );
        assert!(
            expected.iter().any(|channel| *channel < 220),
            "落点线不可退化为白色"
        );
        image
            .save(format!("{directory}/top-tab-drop-{mode:?}.png"))
            .unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            w.dispatch_event(
                MouseUpEvent {
                    button: MouseButton::Left,
                    position: to,
                    modifiers: Default::default(),
                    click_count: 1,
                }
                .to_platform_input(),
                cx,
            );
            w.remove_window();
        })
        .unwrap();
        println!("PASS top-tab-drag-{mode:?}");
    }
}

fn close_button_hover_snapshots(directory: &str, cx: &mut HeadlessAppContext) {
    use std::time::Duration;
    for mode in [Theme::Dark, Theme::Light] {
        cx.update(|cx| theme::apply(mode, cx));
        let handle = cx
            .open_window(size(px(850.), px(800.)), |w, cx| {
                let view = cx.new(|cx| fixture(3, w, cx));
                cx.new(|cx| Root::new(view, w, cx))
            })
            .unwrap();
        cx.update_window(handle.into(), |_, w, cx| w.click("add-tab-title", cx))
            .unwrap();
        std::thread::sleep(Duration::from_millis(330));
        let bounds = cx
            .update_window(handle.into(), |_, w, cx| {
                w.render_frame(cx);
                w.hover(("tab-context", 1usize), cx);
                w.find(("close-tab", 1usize)).bounds()
            })
            .unwrap();
        let before = cx.capture_screenshot(handle.into()).unwrap();
        cx.update_window(handle.into(), |_, w, cx| w.hover(("close-tab", 1usize), cx))
            .unwrap();
        let after = cx.capture_screenshot(handle.into()).unwrap();
        let scale = after.width() as f32 / 850.;
        let sample = |x: f32, y: f32| {
            let x = (x * scale).round() as u32;
            let y = (y * scale).round() as u32;
            (before.get_pixel(x, y).0, after.get_pixel(x, y).0)
        };
        let corner = (f32::from(bounds.left()) + 1., f32::from(bounds.top()) + 1.);
        let cap = (f32::from(bounds.left()) + 10., f32::from(bounds.top()) + 2.);
        let corner_pixels = sample(corner.0, corner.1);
        let cap_pixels = sample(cap.0, cap.1);
        assert_eq!(
            corner_pixels.0, corner_pixels.1,
            "{mode:?}: 关闭按钮圆形高光不能填满方角"
        );
        assert_ne!(cap_pixels.0, cap_pixels.1, "{mode:?}: 圆形高光顶部应可见");
        after
            .save(format!("{directory}/close-hover-{mode:?}.png"))
            .unwrap();
        cx.update_window(handle.into(), |_, w, _| w.remove_window())
            .unwrap();
        println!("PASS close-hover-{mode:?}");
    }
}

fn page_switch_snapshots(directory: &str, cx: &mut HeadlessAppContext) {
    use std::time::Duration;
    for mode in [Theme::Dark, Theme::Light] {
        cx.update(|cx| theme::apply(mode, cx));
        let mut workspace = None;
        let handle = cx
            .open_window(size(px(850.), px(800.)), |window, cx| {
                let view = cx.new(|cx| fixture(3, window, cx));
                workspace = Some(view.clone());
                cx.new(|cx| Root::new(view, window, cx))
            })
            .unwrap();
        let view = workspace.unwrap();
        let (editor, status_bar, start) = cx
            .update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                let editor = window.find("command-editor").bounds();
                let status_bar = window.find("status-bar").bounds();
                let start = std::time::Instant::now();
                // Avoid multiple click-generated frames: the animation uses wall-clock time.
                view.update(cx, |view, cx| {
                    let mut data = crate::state::TabData::example();
                    data.name = "手机式切页 · 新配置".into();
                    data.program = "echo mobile_page".into();
                    let next = crate::state::CommandTab::new(1, data, window, cx);
                    view.tabs.push(next);
                    view.select_tab(1, window, cx);
                });
                window.render_frame(cx);
                assert_eq!(window.find("command-editor").bounds(), editor);
                assert_eq!(window.find("status-bar").bounds(), status_bar);
                (editor, status_bar, start)
            })
            .unwrap();
        let first = cx
            .capture_screenshot(handle.into())
            .expect("切页入场截图失败");
        let first_frame_elapsed = start.elapsed();
        first
            .save(format!("{directory}/page-{mode:?}-enter.png"))
            .unwrap();
        cx.background_executor()
            .advance_clock(Duration::from_millis(100));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
            .unwrap();
        cx.capture_screenshot(handle.into())
            .unwrap()
            .save(format!("{directory}/page-{mode:?}-middle.png"))
            .unwrap();
        cx.background_executor()
            .advance_clock(Duration::from_millis(300));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("page-slide-outgoing").is_none());
            assert!(window.try_find("page-slide-incoming").is_none());
            assert_eq!(window.find("command-editor").bounds(), editor);
            assert_eq!(window.find("status-bar").bounds(), status_bar);
        })
        .unwrap();
        let settled = cx
            .capture_screenshot(handle.into())
            .expect("切页稳定帧截图失败");
        settled
            .save(format!("{directory}/page-{mode:?}-settled.png"))
            .unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |view, cx| {
                view.reduced_motion = true;
                theme::set_reduced_motion(true, cx);
                view.select_tab(0, window, cx);
                view.select_tab(1, window, cx);
            });
            window.render_frame(cx);
            assert!(window.try_find("page-slide-outgoing").is_none());
            assert!(window.try_find("page-slide-incoming").is_none());
            assert_eq!(window.find("command-editor").bounds(), editor);
            assert_eq!(window.find("status-bar").bounds(), status_bar);
        })
        .unwrap();
        let reduced = cx
            .capture_screenshot(handle.into())
            .expect("减少动效切页截图失败");
        reduced
            .save(format!("{directory}/page-{mode:?}-reduced.png"))
            .unwrap();
        let scale = first.width() as f32 / 850.;
        let mut changed = 0;
        let mut reduced_difference = 0;
        // 只比较页面内部，避开标签本身的高亮动画。
        for y in ((f32::from(editor.top()) + 16.) * scale) as u32
            ..((f32::from(editor.top()) + 260.) * scale) as u32
        {
            for x in ((f32::from(editor.left()) + 24.) * scale) as u32
                ..((f32::from(editor.left()) + 520.) * scale) as u32
            {
                if first.get_pixel(x, y) != settled.get_pixel(x, y) {
                    changed += 1;
                }
                if settled.get_pixel(x, y) != reduced.get_pixel(x, y) {
                    reduced_difference += 1;
                }
            }
        }
        // A saturated CI renderer can take longer than the 260ms animation to deliver
        // the first screenshot. In that case the state tests above still verify the
        // animation is scheduled; do not interpret a settled frame as no animation.
        if first_frame_elapsed < Duration::from_millis(190) {
            assert!(
                changed > 200,
                "{mode:?} 整页滑动必须真正改变页面像素：{changed}"
            );
        }
        assert_eq!(reduced_difference, 0, "{mode:?} 减少动效应立即显示稳定内容");
        cx.update_window(handle.into(), |_, window, _| window.remove_window())
            .unwrap();
        println!("PASS page-switch-{mode:?}");
    }
}

fn detached_log_snapshots(directory: &str, cx: &mut HeadlessAppContext) {
    for mode in [Theme::Dark, Theme::Light] {
        cx.update(|cx| theme::apply(mode, cx));
        let mut workspace = None;
        let main = cx
            .open_window(size(px(850.), px(800.)), |w, cx| {
                let view = cx.new(|cx| fixture(3, w, cx));
                workspace = Some(view.clone());
                cx.new(|cx| Root::new(view, w, cx))
            })
            .unwrap();
        let view = workspace.unwrap();
        cx.update_window(main.into(), |_, w, cx| {
            view.update(cx, |v, cx| {
                v.show_log = true;
                v.log_height_override = Some(180.);
                v.add_finished_log_for_test("生成示例", "第一行输出\n第二行输出", w, cx);
            });
            w.click("detach-log", cx);
        })
        .unwrap();
        cx.run_until_parked();
        let detached = cx
            .update(|cx| view.read(cx).detached_log)
            .expect("离屏独立日志窗口");
        cx.update_window(detached.into(), |_, w, cx| {
            w.render_frame(cx);
            let panel = w.find("detached-log-window").bounds();
            let output = w.find("detached-log-output").bounds();
            assert!(output.top() >= panel.top() && output.bottom() <= panel.bottom());
            assert!(output.size.height > px(100.));
            assert!(w.try_find("reattach-log").is_some());
        })
        .unwrap();
        cx.capture_screenshot(detached.into())
            .expect("离屏日志分离截图失败")
            .save(format!("{directory}/log-detached-{mode:?}.png"))
            .unwrap();
        cx.update_window(detached.into(), |_, w, cx| w.click("reattach-log", cx))
            .unwrap();
        cx.run_until_parked();
        cx.update_window(main.into(), |_, w, cx| {
            w.render_frame(cx);
            assert!(w.try_find("log-dock").is_some());
            w.remove_window();
        })
        .unwrap();
        println!("PASS log-detached-{mode:?}");
    }
}

pub fn snapshots(directory: &str) {
    std::fs::create_dir_all(directory).unwrap();
    let mut cx = HeadlessAppContext::with_platform(
        gpui::platform::current_platform(true).text_system(),
        Arc::new(AppAssets),
        gpui::platform::current_headless_renderer,
    );
    cx.update(gpui::init);
    closed_sidebar_snapshots(directory, &mut cx);
    sidebar_resize_snapshots(directory, &mut cx);
    parameter_drag_preview_snapshots(directory, &mut cx);
    compact_note_hover_snapshots(directory, &mut cx);
    heading_text_snapshots(directory, &mut cx);
    focus_snapshots(directory, &mut cx);
    page_switch_snapshots(directory, &mut cx);
    close_button_hover_snapshots(directory, &mut cx);
    sidebar_icon_hover_snapshots(directory, &mut cx);
    top_tab_drag_visual_snapshots(directory, &mut cx);
    detached_log_snapshots(directory, &mut cx);
    // Keep visual evidence that the parameter-sort switch is neutral when off and
    // follows the user-selected accent when on, at every supported text size.
    for (mode, accent) in [
        (Theme::Dark, theme::Accent::Teal),
        (Theme::Light, theme::Accent::Violet),
    ] {
        for font_size in [12u8, 14, 16] {
            cx.update(|cx| {
                cx.set_global(accent);
                theme::apply(mode, cx);
                theme::set_font_size(font_size, cx);
            });
            let mut workspace = None;
            let handle = cx
                .open_window(size(px(850.), px(800.)), |w, cx| {
                    let view = cx.new(|cx| fixture(3, w, cx));
                    workspace = Some(view.clone());
                    cx.new(|cx| Root::new(view, w, cx))
                })
                .unwrap();
            cx.update_window(handle.into(), |_, w, cx| w.render_frame(cx))
                .unwrap();
            for enabled in [false, true] {
                workspace.as_ref().unwrap().update(&mut cx, |view, cx| {
                    view.enabled_first = enabled;
                    cx.notify();
                });
                cx.update_window(handle.into(), |_, w, cx| w.render_frame(cx))
                    .unwrap();
                let name = format!(
                    "priority-{}-{}px-{}",
                    if mode == Theme::Dark { "dark" } else { "light" },
                    font_size,
                    if enabled { "on" } else { "off" }
                );
                cx.capture_screenshot(handle.into())
                    .expect("参数优先开关截图失败")
                    .save(format!("{directory}/{name}.png"))
                    .unwrap();
            }
            cx.update_window(handle.into(), |_, w, _| w.remove_window())
                .unwrap();
        }
    }
    for (width, height, log) in [
        (850., 800., false),
        (850., 700., false),
        (640., 660., false),
        (1200., 900., false),
        (759., 760., false),
        (760., 760., false),
        (850., 800., true),
        (640., 660., true),
    ] {
        for rows in [0, 1, 3, 6, 7, 12] {
            let mut previous = None;
            for mode in [Theme::Dark, Theme::Light] {
                cx.update(|cx| theme::apply(mode, cx));
                let mut workspace = None;
                let handle = cx
                    .open_window(size(px(width), px(height)), |window, cx| {
                        let view = cx.new(|cx| {
                            let mut view = fixture(rows, window, cx);
                            view.show_log = log;
                            view
                        });
                        workspace = Some(view.clone());
                        cx.new(|cx| Root::new(view, window, cx))
                    })
                    .unwrap();
                let geometry = cx
                    .update_window(handle.into(), |_, window, cx| {
                        window.render_frame(cx);
                        geometry(window)
                    })
                    .unwrap();
                let name = format!(
                    "{}-{}x{}{}-{}rows",
                    if mode == Theme::Dark { "dark" } else { "light" },
                    width,
                    height,
                    if log { "-log" } else { "" },
                    rows
                );
                let image = cx
                    .capture_screenshot(handle.into())
                    .expect("Metal 离屏渲染不可用");
                image.save(format!("{directory}/{name}.png")).unwrap();
                std::fs::write(
                    format!("{directory}/{name}.json"),
                    serde_json::to_vec_pretty(&geometry).unwrap(),
                )
                .unwrap();
                validate(&geometry, height, rows, log);
                if let Some(previous) = &previous {
                    assert_eq!(previous, &geometry, "主题改变了布局：{name}");
                }
                previous = Some(geometry);
                if width == 1200. && height == 900. && rows == 3 && !log {
                    cx.update_window(handle.into(), |_, w, cx| {
                        workspace.as_ref().unwrap().update(cx, |v, cx| {
                            v.tabs[0].dirty = true;
                            cx.notify();
                        });
                        w.click("add-tab-title", cx);
                    })
                    .unwrap();
                    let image = cx
                        .capture_screenshot(handle.into())
                        .expect("标签页截图失败");
                    image
                        .save(format!(
                            "{directory}/tab-colors-{}.png",
                            if mode == Theme::Dark { "dark" } else { "light" }
                        ))
                        .unwrap();
                }
                if ((width == 850. && height == 800.) || (width == 640. && height == 660.))
                    && !log
                    && rows == 3
                {
                    cx.update_window(handle.into(), |_, w, cx| {
                        w.click("settings", cx);
                        for id in [
                            "settings-copy-json",
                            "settings-close",
                            "settings-font-weight",
                        ] {
                            let b = w.find(id).bounds();
                            assert!(
                                b.left() >= px(0.)
                                    && b.right() <= px(width)
                                    && b.bottom() <= px(height),
                                "设置控件溢出窗口：{id} {b:?}"
                            );
                        }
                    })
                    .unwrap();
                    let settings = cx
                        .capture_screenshot(handle.into())
                        .expect("设置页面截图失败");
                    settings
                        .save(format!(
                            "{directory}/settings-{}-{}x{}.png",
                            if mode == Theme::Dark { "dark" } else { "light" },
                            width,
                            height
                        ))
                        .unwrap();
                }
                cx.update_window(handle.into(), |_, w, _| w.remove_window())
                    .unwrap();
                println!("PASS {name}");
            }
        }
    }
    // Font/language extremes and the scrollable settings panel on a small desktop.
    for mode in [Theme::Dark, Theme::Light] {
        for language in [
            crate::i18n::Language::Chinese,
            crate::i18n::Language::English,
        ] {
            for font_size in [10, 14, 24] {
                cx.update(|cx| theme::apply(mode, cx));
                let handle = cx
                    .open_window(size(px(640.), px(660.)), |w, cx| {
                        let v = cx.new(|cx| {
                            let mut v = fixture(3, w, cx);
                            v.apply_preferences(
                                crate::settings::Preferences {
                                    theme: mode,
                                    accent: theme::Accent::Violet,
                                    font_size,
                                    font_weight: 800,
                                    language,
                                    reduced_motion: true,
                                },
                                w,
                                cx,
                            )
                            .unwrap();
                            v
                        });
                        cx.new(|cx| Root::new(v, w, cx))
                    })
                    .unwrap();
                let name = format!("fonts-{mode:?}-{}-{font_size}px", language.code());
                cx.capture_screenshot(handle.into())
                    .unwrap()
                    .save(format!("{directory}/{name}.png"))
                    .unwrap();
                cx.update_window(handle.into(), |_, w, cx| {
                    w.render_frame(cx);
                    validate(&geometry(w), 660., 3, false);
                    w.click("settings", cx);
                })
                .unwrap();
                std::thread::sleep(std::time::Duration::from_millis(300));
                cx.capture_screenshot(handle.into())
                    .unwrap()
                    .save(format!("{directory}/{name}-settings.png"))
                    .unwrap();
                if language == crate::i18n::Language::English && font_size == 24 {
                    cx.update_window(handle.into(), |_, w, cx| w.click("settings-accent", cx))
                        .unwrap();
                    cx.capture_screenshot(handle.into())
                        .unwrap()
                        .save(format!("{directory}/{name}-accent.png"))
                        .unwrap();
                }
                cx.update_window(handle.into(), |_, w, _| w.remove_window())
                    .unwrap();
                println!("PASS {name}");
            }
        }
    }
    // Expanded/collapsed sidebar and bundled SVG picker replace the removed tab dropdown.
    for mode in [Theme::Dark, Theme::Light] {
        cx.update(|cx| theme::apply(mode, cx));
        let mut workspace = None;
        let handle = cx
            .open_window(size(px(850.), px(800.)), |w, cx| {
                let view = cx.new(|cx| fixture(3, w, cx));
                workspace = Some(view.clone());
                cx.new(|cx| Root::new(view, w, cx))
            })
            .unwrap();
        let view = workspace.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            w.click("add-tab-title", cx);
            w.click("add-tab-title", cx);
            w.render_frame(cx);
            assert!(w.try_find("tab-selector").is_none());
            assert_eq!(
                w.find(("sidebar-tab", 0usize)).bounds().size.height,
                px(40.)
            );
        })
        .unwrap();
        let default_icon = cx
            .update_window(handle.into(), |_, w, cx| {
                w.render_frame(cx);
                w.find(("sidebar-icon", 0usize)).bounds()
            })
            .unwrap();
        let expanded_image = cx.capture_screenshot(handle.into()).unwrap();
        let scale = expanded_image.width() as f32 / 850.;
        let x0 = (f32::from(default_icon.left()) * scale).round() as u32;
        let y0 = (f32::from(default_icon.top()) * scale).round() as u32;
        let x1 = (f32::from(default_icon.right()) * scale).round() as u32;
        let y1 = (f32::from(default_icon.bottom()) * scale).round() as u32;
        let image_ref = &expanded_image;
        let foreground = cx.update(|cx| theme::palette(cx).text);
        let matches_foreground = |rgba: &[u8; 4]| {
            [16, 8, 0]
                .into_iter()
                .enumerate()
                .all(|(i, shift)| rgba[i].abs_diff(((foreground >> shift) & 255) as u8) <= 16)
        };
        let default_pixels = (y0..y1)
            .flat_map(|y| (x0..x1).map(move |x| image_ref.get_pixel(x, y).0))
            .filter(matches_foreground)
            .count();
        assert!(
            default_pixels >= 12,
            "默认未选中图标在 {mode:?} 模式下应跟随文字颜色"
        );
        expanded_image
            .save(format!("{directory}/sidebar-{mode:?}-expanded.png"))
            .unwrap();
        cx.update_window(handle.into(), |_, w, cx| w.click("collapse-sidebar", cx))
            .unwrap();
        cx.background_executor()
            .advance_clock(std::time::Duration::from_millis(60));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            let panel = w.find("tab-sidebar").bounds();
            let grip = w.find("sidebar-resize-handle").bounds();
            assert!(panel.size.width > px(52.) && panel.size.width < px(208.));
            assert!((grip.right() - panel.right()).abs() <= px(1.));
            assert_eq!(grip.size.width, px(10.));
        })
        .unwrap();
        cx.capture_screenshot(handle.into())
            .unwrap()
            .save(format!("{directory}/sidebar-{mode:?}-folding.png"))
            .unwrap();
        cx.background_executor()
            .advance_clock(std::time::Duration::from_millis(840));
        cx.run_until_parked();
        cx.capture_screenshot(handle.into())
            .unwrap()
            .save(format!("{directory}/sidebar-{mode:?}-collapsed.png"))
            .unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            assert_eq!(w.find("tab-sidebar").bounds().size.width, px(56.));
            w.click("collapse-sidebar", cx);
        })
        .unwrap();
        cx.background_executor()
            .advance_clock(std::time::Duration::from_millis(60));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            let panel = w.find("tab-sidebar").bounds();
            let grip = w.find("sidebar-resize-handle").bounds();
            assert!(panel.size.width > px(52.) && panel.size.width < px(220.));
            assert!((grip.right() - panel.right()).abs() <= px(1.));
            assert_eq!(grip.size.width, px(10.));
        })
        .unwrap();
        cx.capture_screenshot(handle.into())
            .unwrap()
            .save(format!("{directory}/sidebar-{mode:?}-unfolding.png"))
            .unwrap();
        cx.background_executor()
            .advance_clock(std::time::Duration::from_millis(840));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            w.click(("sidebar-icon", 0usize), cx)
        })
        .unwrap();
        cx.run_until_parked();
        let symbols = cx
            .update_window(handle.into(), |_, w, cx| {
                w.render_frame(cx);
                tab_icons::BUILTINS
                    .iter()
                    .map(|(name, _)| w.find(format!("icon-choice-symbol-{name}")).bounds())
                    .collect::<Vec<_>>()
            })
            .unwrap();
        cx.run_until_parked();
        let screenshot = cx.capture_screenshot(handle.into()).unwrap();
        let scale = screenshot.width() as f32 / 850.;
        let image_ref = &screenshot;
        let pixels = symbols
            .into_iter()
            .map(|bounds| {
                let left = (f32::from(bounds.left()) * scale).round() as u32;
                let top = (f32::from(bounds.top()) * scale).round() as u32;
                let right = (f32::from(bounds.right()) * scale).round() as u32;
                let bottom = (f32::from(bounds.bottom()) * scale).round() as u32;
                (top..bottom)
                    .flat_map(|y| (left..right).map(move |x| image_ref.get_pixel(x, y).0))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        // Picker glyphs use the theme foreground, with no colored icon tile.
        for ((name, _), colors) in tab_icons::BUILTINS.iter().zip(&pixels) {
            let foreground_pixels = colors
                .iter()
                .filter(|color| matches_foreground(color))
                .count();
            assert!(
                foreground_pixels >= 24,
                "{name} 在 {mode:?} 下缺少主题前景色轮廓: {foreground_pixels}px"
            );
            assert!(
                !colors.iter().any(|[r, g, b, _]| *r > 155
                    && *g > 110
                    && *b < 95
                    && *r > g.saturating_add(15)
                    && *g > b.saturating_add(45)),
                "{name} 不应残留彩色品牌填充"
            );
        }
        screenshot
            .save(format!("{directory}/sidebar-{mode:?}-picker.png"))
            .unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            assert!(w.try_find("icon-choice-python").is_some());
            assert!(w.try_find("icon-choice-rust").is_some());
            w.close_dialog(cx);
            view.update(cx, |v, cx| {
                v.reduced_motion = true;
                v.set_tab_icon_from_source(0, "builtin:python".into(), cx);
                v.sidebar_collapsed = true;
                cx.notify();
            });
        })
        .unwrap();
        cx.run_until_parked();
        let bounds = cx
            .update_window(handle.into(), |_, w, cx| {
                w.render_frame(cx);
                w.find(("sidebar-icon", 0usize)).bounds()
            })
            .unwrap();
        cx.run_until_parked();
        let image = cx.capture_screenshot(handle.into()).unwrap();
        let scale = image.width() as f32 / 850.;
        let left = (f32::from(bounds.left()) * scale).round() as u32;
        let top = (f32::from(bounds.top()) * scale).round() as u32;
        let right = (f32::from(bounds.right()) * scale).round() as u32;
        let bottom = (f32::from(bounds.bottom()) * scale).round() as u32;
        let image_ref = &image;
        let foreground_pixels = (top..bottom)
            .flat_map(|y| (left..right).map(move |x| image_ref.get_pixel(x, y).0))
            .filter(matches_foreground)
            .count();
        assert!(
            foreground_pixels >= 24,
            "折叠侧栏未选中的 Python 图标应跟随主题前景色: {foreground_pixels}px"
        );
        // The dirty, unselected middle row must not retain the old full-height accent border.
        let dirty_row = cx
            .update_window(handle.into(), |_, w, _| {
                w.find(("sidebar-tab", 1usize)).bounds()
            })
            .unwrap();
        let surface = cx.update(|cx| theme::palette(cx).sidebar);
        let expected = [(surface >> 16) as u8, (surface >> 8) as u8, surface as u8];
        for inset in [1.5, 2.5] {
            for y_offset in [12., 20., 28.] {
                let x = ((f32::from(dirty_row.right()) - inset) * scale).round() as u32;
                let y = ((f32::from(dirty_row.top()) + y_offset) * scale).round() as u32;
                let pixel = image.get_pixel(x, y).0;
                assert_eq!(
                    &pixel[..3],
                    &expected,
                    "{mode:?} 折叠未选中图标右侧不能残留主题色竖条"
                );
            }
        }
        image
            .save(format!("{directory}/sidebar-{mode:?}-python.png"))
            .unwrap();
        if mode == Theme::Light {
            // Real Metal pixels: the selected Lucide glyph must be white on selection;
            // the unselected glyph must be dark directly on the sidebar surface.
            for accent in [
                theme::Accent::Blue,
                theme::Accent::Rose,
                theme::Accent::Amber,
            ] {
                cx.update_window(handle.into(), |_, w, cx| {
                    cx.set_global(accent);
                    theme::apply(Theme::Light, cx);
                    view.update(cx, |v, cx| {
                        v.sidebar_collapsed = false;
                        v.reduced_motion = true;
                        v.set_tab_icon_from_source(0, "builtin:lucide/rocket".into(), cx);
                        v.set_tab_icon_from_source(2, "builtin:lucide/heart".into(), cx);
                        cx.notify();
                    });
                    w.render_frame(cx);
                    w.hover("program", cx); // Remove the prior picker-trigger hover.
                })
                .unwrap();
                let (unselected, selected) = cx
                    .update_window(handle.into(), |_, w, cx| {
                        w.render_frame(cx);
                        (
                            w.find(("sidebar-icon", 0usize)).bounds(),
                            w.find(("sidebar-icon", 2usize)).bounds(),
                        )
                    })
                    .unwrap();
                let image = cx.capture_screenshot(handle.into()).unwrap();
                let scale = image.width() as f32 / 850.;
                let image_ref = &image;
                let sample = |bounds: gpui::Bounds<gpui::Pixels>| {
                    let x0 = (f32::from(bounds.left()) * scale).round() as u32;
                    let y0 = (f32::from(bounds.top()) * scale).round() as u32;
                    let x1 = (f32::from(bounds.right()) * scale).round() as u32;
                    let y1 = (f32::from(bounds.bottom()) * scale).round() as u32;
                    (y0..y1)
                        .flat_map(|y| (x0..x1).map(move |x| image_ref.get_pixel(x, y).0))
                        .collect::<Vec<_>>()
                };
                let plain = sample(unselected);
                let active = sample(selected);
                let sidebar = cx.update(|cx| theme::palette(cx).sidebar);
                let surface_rgb = [
                    ((sidebar >> 16) & 255) as u8,
                    ((sidebar >> 8) & 255) as u8,
                    (sidebar & 255) as u8,
                ];
                assert!(
                    plain
                        .iter()
                        .filter(|[r, g, b, _]| *r < 80 && *g < 80 && *b < 100)
                        .count()
                        >= 24,
                    "{accent:?} 白底未选中 Lucide 必须是深色"
                );
                assert!(
                    plain
                        .iter()
                        .filter(|[r, g, b, _]| {
                            [*r, *g, *b]
                                .into_iter()
                                .zip(surface_rgb)
                                .all(|(actual, expected)| actual.abs_diff(expected) <= 2)
                        })
                        .count()
                        >= 80,
                    "{accent:?} 未选中图标应保持侧栏底色，不能垫主题色块"
                );
                assert!(
                    active
                        .iter()
                        .filter(|[r, g, b, _]| *r >= 245 && *g >= 245 && *b >= 245)
                        .count()
                        >= 24,
                    "{accent:?} 选中的 Lucide 必须是白色"
                );
                let sample_width = (f32::from(unselected.size.width) * scale).round() as usize;
                let corner = plain[2 * sample_width + 2];
                assert!(
                    [corner[0], corner[1], corner[2]]
                        .into_iter()
                        .zip(surface_rgb)
                        .all(|(actual, expected)| actual.abs_diff(expected) <= 2),
                    "图标底应透明、露出侧栏底色"
                );
                image
                    .save(format!("{directory}/sidebar-Light-contrast-{accent:?}.png"))
                    .unwrap();
            }
        }
        cx.update_window(handle.into(), |_, w, _| w.remove_window())
            .unwrap();
        println!("PASS sidebar-{mode:?}");
    }
    cx.update(|cx| {
        cx.set_global(theme::Accent::Blue);
        theme::apply(Theme::Light, cx);
    });
    let mut many_tabs_view = None;
    let many_tabs = cx
        .open_window(size(px(850.), px(800.)), |w, cx| {
            let view = cx.new(|cx| CommandWorkspace::new(w, cx));
            many_tabs_view = Some(view.clone());
            cx.new(|cx| Root::new(view, w, cx))
        })
        .unwrap();
    let many_tabs_view = many_tabs_view.unwrap();
    let selected = cx
        .update_window(many_tabs.into(), |_, w, cx| {
            many_tabs_view.update(cx, |v, _| v.reduced_motion = true);
            for _ in 1..28 {
                w.click("add-tab-title", cx);
            }
            w.render_frame(cx);
            assert_eq!(many_tabs_view.read(cx).active, 27);
            w.find(("sidebar-tab", 27usize)).bounds()
        })
        .unwrap();
    let image = cx.capture_screenshot(many_tabs.into()).unwrap();
    let scale = image.width() as f32 / 850.;
    let x = (f32::from(selected.left() + px(3.)) * scale).round() as u32;
    let y = (f32::from(selected.top() + px(4.)) * scale).round() as u32;
    let pixel = image.get_pixel(x, y).0;
    assert_eq!(
        [pixel[0], pixel[1], pixel[2]],
        [0x25, 0x63, 0xeb],
        "滚动后选中标签的滑块必须可见：{pixel:?}"
    );
    image
        .save(format!("{directory}/sidebar-Light-scrolled-selection.png"))
        .unwrap();
    cx.update_window(many_tabs.into(), |_, w, _| w.remove_window())
        .unwrap();
    println!("PASS sidebar-Light-scrolled-selection");
    for mode in [Theme::Dark, Theme::Light] {
        cx.update(|cx| theme::apply(mode, cx));
        let handle = cx
            .open_window(size(px(1200.), px(1200.)), |w, cx| {
                let view = cx.new(|cx| fixture(12, w, cx));
                cx.new(|cx| Root::new(view, w, cx))
            })
            .unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            let from = w.find("parameter-resize").bounds().center();
            w.drag(from, gpui::point(from.x, from.y + px(180.)), cx);
            assert_eq!(
                w.find("parameter-list").bounds().size.height,
                px(crate::tokens::parameter_height(9))
            );
            w.click("toggle-log", cx);
            let from = w.find("log-resize").bounds().center();
            w.drag(from, gpui::point(from.x, from.y - px(110.)), cx);
            assert!(w.find("log-dock").bounds().size.height >= px(120.));
        })
        .unwrap();
        let image = cx
            .capture_screenshot(handle.into())
            .expect("拖动高度截图失败");
        image
            .save(format!(
                "{directory}/resized-panels-{}.png",
                if mode == Theme::Dark { "dark" } else { "light" }
            ))
            .unwrap();
        cx.update_window(handle.into(), |_, w, _| w.remove_window())
            .unwrap();
    }
}

fn validate(geometry: &BTreeMap<String, [f32; 4]>, height: f32, rows: usize, log: bool) {
    use crate::tokens::*;
    let equal = |a: f32, b: f32, label: &str| assert!((a - b).abs() < 0.1, "{label}: {a} != {b}");
    let bottom = |id: &str| geometry[id][1] + geometry[id][3];
    let right = |id: &str| geometry[id][0] + geometry[id][2];
    let layout = WorkspaceLayout::new(height, rows, log);
    let editor = geometry["command-editor"];
    equal(editor[1], TITLE_HEIGHT, "标题栏高度");
    equal(
        geometry["command-form"][1] - editor[1],
        SECTION,
        "编辑器上内边距",
    );
    equal(
        bottom("command-editor") - bottom("run-actions"),
        SECTION,
        "编辑器下内边距",
    );
    let sidebar_right = right("tab-sidebar");
    equal(geometry["tab-sidebar"][0], 0., "侧栏贴左");
    for id in [
        "command-form",
        "parameter-list",
        "command-extras",
        "run-actions",
    ] {
        equal(
            geometry[id][0],
            sidebar_right + PAGE_INSET,
            &format!("{id} 左外边距"),
        );
        equal(
            right(id),
            geometry["workspace"][2] - PAGE_INSET,
            &format!("{id} 右外边距"),
        );
    }
    equal(
        geometry["preview-frame"][0],
        sidebar_right + PAGE_INSET,
        "预览左外边距",
    );
    equal(
        right("description-frame"),
        geometry["workspace"][2] - PAGE_INSET,
        "描述右外边距",
    );
    equal(
        geometry["preview-splitter"][0] - right("preview-frame"),
        0.,
        "分隔拖柄紧邻预览",
    );
    equal(geometry["preview-splitter"][2], GAP, "分隔拖柄宽度");
    equal(
        geometry["description-frame"][0] - right("preview-splitter"),
        0.,
        "分隔拖柄紧邻描述",
    );
    equal(
        geometry["working-directory"][1] - bottom("command-name"),
        GAP,
        "标题与执行字段间距",
    );
    equal(
        geometry["program"][1] - bottom("working-directory"),
        GAP,
        "紧凑执行字段行距",
    );
    equal(
        geometry["command-name"][3],
        COMMAND_NAME_HEIGHT,
        "可编辑标题高度",
    );
    equal(
        geometry["command-form"][3],
        FORM_HEIGHT,
        "表单区实际高度与布局预算一致",
    );
    equal(geometry["command-form-divider"][3], 1., "程序下分隔线高度");
    equal(
        geometry["command-form-divider"][1] - bottom("program"),
        GAP,
        "程序与分隔线间距",
    );
    equal(
        geometry["command-form-divider"][0],
        geometry["command-form"][0],
        "分隔线左对齐",
    );
    equal(
        right("command-form-divider"),
        right("command-form"),
        "分隔线右对齐",
    );
    equal(
        geometry["parameter-header"][1] - bottom("command-form"),
        SECTION,
        "表单与参数区间距",
    );
    equal(
        geometry["parameter-list"][1] - bottom("parameter-header"),
        HEADER_GAP,
        "表头与列表间距",
    );
    equal(
        geometry["parameter-actions"][1] - bottom("parameter-list"),
        GAP,
        "参数操作行间距",
    );
    equal(
        geometry["command-extras"][1] - bottom("parameter-actions"),
        GAP,
        "附加字段与参数操作行间距",
    );
    equal(
        geometry["command-extras"][3],
        EXTRAS_HEIGHT,
        "附加字段只占一行",
    );
    equal(
        geometry["append-fragment"][3],
        EXTRAS_HEIGHT,
        "解析区描边包围整个输入行",
    );
    assert!(
        (geometry["other-args-cell"][2] - geometry["command-extras"][2] * 0.5).abs() <= 0.5,
        "其他参数占行宽的一半（允许像素对齐舍入）"
    );
    equal(
        geometry["other-args"][1],
        geometry["append-command"][1],
        "两个输入框同行",
    );
    equal(
        geometry["append-parse"][1],
        geometry["other-args"][1],
        "按钮与输入框同行",
    );
    equal(geometry["append-parse"][2], 104., "追加解析按钮固定宽度");
    let horizontal_left = geometry["append-command"][0] - geometry["append-fragment"][0];
    let horizontal_right = right("append-fragment") - right("append-parse");
    assert!(
        (horizontal_left - horizontal_right).abs() <= 0.5,
        "解析框左右内边距不对称：{horizontal_left} / {horizontal_right}"
    );
    let vertical_top = geometry["append-command"][1] - geometry["append-fragment"][1];
    let vertical_bottom = bottom("append-fragment") - bottom("append-command");
    assert!(
        (vertical_top - vertical_bottom).abs() <= 0.5,
        "解析框上下内边距不对称：{vertical_top} / {vertical_bottom}"
    );
    assert!(
        horizontal_left >= 4. && vertical_top >= 4.,
        "描边与控件之间必须有可见留白"
    );
    assert!(
        right("other-args") < geometry["append-parse"][0],
        "解析区与实际运行参数必须区分"
    );
    assert!(
        right("append-command") < geometry["append-parse"][0],
        "追加解析按钮必须在粘贴框右侧"
    );
    assert!(
        right("command-extras") - right("append-parse") <= 6.,
        "追加解析按钮应靠右且留与左侧相同的边距"
    );
    let hint = geometry["append-only-hint"];
    assert!(
        hint[1] >= bottom("command-extras") && hint[1] + hint[3] <= geometry["preview-header"][1],
        "持久的仅解析说明应在表单和预览的留白内"
    );
    assert!(
        hint[0] >= geometry["append-fragment"][0]
            && hint[0] + hint[2] <= right("command-extras") + 0.1,
        "仅解析说明只属于粘贴分组"
    );
    assert!(geometry["append-command"][2] > 0., "窄窗口仍需可编辑粘贴区");
    equal(
        geometry["preview-header"][3],
        PREVIEW_HEADER_HEIGHT,
        "预览标题行高",
    );
    equal(
        geometry["preview-header"][1] - bottom("command-extras"),
        SECTION,
        "预览区间距",
    );
    equal(
        geometry["preview-frame"][1] - bottom("preview-header"),
        GAP,
        "预览标题与内容区间距",
    );
    equal(
        geometry["run-actions"][1] - bottom("preview-frame"),
        GAP,
        "预览操作行间距",
    );
    equal(
        geometry["parameter-list"][3],
        layout.rows_height,
        "参数区按内容分配高度",
    );
    equal(
        geometry["parameter-items"][3],
        layout.rows_height - TABLE_INSET * 2.,
        "参数视口留白",
    );
    if let Some(row) = geometry.get("option-row") {
        equal(
            row[1] - geometry["parameter-list"][1],
            TABLE_INSET,
            "参数区上内边距",
        );
        let visible = ((layout.rows_height - TABLE_INSET * 2. + GAP) / (CONTROL + GAP)) as usize;
        equal(
            bottom("parameter-list") - (row[1] + (visible - 1) as f32 * (CONTROL + GAP) + CONTROL),
            TABLE_INSET,
            "参数区下内边距（不可裁切半行）",
        );
    }
    assert!(
        geometry.contains_key("description-frame"),
        "描述不能在小窗口隐藏"
    );
    equal(
        geometry["description-frame"][3],
        geometry["preview-frame"][3],
        "描述与预览并排等高",
    );
    equal(
        geometry["description-frame"][1],
        geometry["preview-frame"][1],
        "描述与预览对齐",
    );

    for id in [
        "program",
        "working-directory",
        "parse-command",
        "append-command",
        "append-parse",
        "other-args",
        "run-command",
        "copy-command",
        "app-brand",
        "application-menu",
        "previous-tab",
        "tab-strip",
        "add-tab-title",
        "next-tab",
        "reload",
        "settings",
        "save",
        "theme-toggle",
    ] {
        if let Some(bounds) = geometry.get(id) {
            assert!(
                (bounds[3] - 32.).abs() < 0.1,
                "{id}: 高度不是32px: {bounds:?}"
            );
        }
    }
    let width = geometry["workspace"][2];
    for (id, bounds) in geometry {
        assert!(
            bounds[1] >= 0.
                && bounds[1] + bounds[3] <= height + 1.
                && bounds[0] >= 0.
                && bounds[0] + bounds[2] <= width + 1.,
            "{id} 超出窗口: {bounds:?}"
        );
    }
    assert!(
        geometry["title-drag-space"][2] >= 40.,
        "标题栏失去可拖动空白区"
    );
    assert!(
        geometry["app-brand"][0] + geometry["app-brand"][2] <= geometry["application-menu"][0],
        "标题栏品牌与菜单重叠"
    );
    assert!(!geometry.contains_key("new-tab"), "左下角不应重复新建标签");
    for (heading, row) in [
        ("option-heading", "option-row"),
        ("value-heading", "value-row"),
    ] {
        if let Some(row) = geometry.get(row) {
            assert!(
                (geometry[heading][0] - row[0]).abs() < 1.,
                "表头与参数列错位: {heading} / {row:?}"
            );
            assert!((row[3] - 32.).abs() < 0.1, "参数行不是32px");
        }
    }
    if let Some(note) = geometry.get("note-row") {
        assert!(
            (note[0] - geometry["note-heading"][0]).abs() < 1.,
            "备注列错位"
        );
    }
    let preview = geometry["preview-frame"];
    assert!(preview[3] >= 72.);
    let run = geometry["run-command"];
    assert!(run[1] >= preview[1] + preview[3], "预览与运行按钮重叠");
    assert!(
        geometry["command-editor"][1] + geometry["command-editor"][3] >= run[1] + run[3],
        "运行按钮超出编辑区"
    );
    let editor_bottom = bottom("command-editor");
    if let Some(dock) = geometry.get("log-dock") {
        equal(dock[1], editor_bottom, "日志紧邻编辑区");
        equal(
            geometry["status-bar"][1],
            dock[1] + dock[3],
            "状态栏紧邻日志",
        );
    } else {
        equal(
            geometry["status-bar"][1],
            editor_bottom,
            "无底部工具栏时状态栏紧邻编辑区",
        );
    }
    if let (Some(dock), Some(selector)) = (geometry.get("log-dock"), geometry.get("log-selector")) {
        assert!(
            selector[1] + selector[3] <= dock[1] + dock[3] + 0.1,
            "紧凑日志面板的下拉控件不得压住状态栏"
        );
        if let Some(detach) = geometry.get("detach-log") {
            assert!(
                detach[0] + detach[2] <= dock[0] + dock[2],
                "分离按钮不能溢出日志栏"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        app::CommandWorkspace,
        tab_icons,
        theme::{self, Theme},
    };
    use gpui::component::{Root, WindowExt};
    use gpui::test::TestWindowExt;
    use gpui::{px, size, AppContext, TestAppContext};

    #[gpui::test]
    fn spacing_and_content_height_matrix(cx: &mut TestAppContext) {
        cx.update(gpui::init);
        for (width, height) in [(640., 660.), (850., 700.), (1200., 900.)] {
            for rows in [0, 1, 3, 6, 7] {
                for log in [false, true] {
                    let mut previous = None;
                    for mode in [Theme::Dark, Theme::Light] {
                        cx.update(|cx| theme::apply(mode, cx));
                        let handle = cx.open_window(size(px(width), px(height)), |window, cx| {
                            let view = cx.new(|cx| {
                                let mut view = super::fixture(rows, window, cx);
                                view.show_log = log;
                                view
                            });
                            Root::new(view, window, cx)
                        });
                        cx.update_window(handle.into(), |_, window, cx| {
                            window.render_frame(cx);
                            let current = super::geometry(window);
                            super::validate(&current, height, rows, log);
                            assert!(window.try_find("config-toolbar").is_none());
                            assert!(window.try_find("refresh-preview").is_none());
                            if let Some(previous) = &previous {
                                assert_eq!(previous, &current, "主题不应影响任何几何");
                            }
                            previous = Some(current);
                            window.remove_window();
                        })
                        .unwrap();
                    }
                }
            }
        }
    }

    #[gpui::test]
    fn adding_and_removing_rows_resizes_only_content_area(cx: &mut TestAppContext) {
        use crate::tokens::{parameter_height, CONTROL, GAP};
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let handle = cx.open_window(size(px(850.), px(1000.)), |window, cx| {
            let view = cx.new(|cx| super::fixture(1, window, cx));
            Root::new(view, window, cx)
        });
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let status_bar = window.find("status-bar").bounds();
            for count in 2..=7 {
                window.click("add-parameter", cx);
                assert_eq!(
                    f32::from(window.find("parameter-list").bounds().size.height),
                    parameter_height(count.min(6))
                );
                assert_eq!(status_bar, window.find("status-bar").bounds());
                let last = window.find(("option", count - 1)).bounds();
                let viewport = window.find("parameter-items").bounds();
                assert!(
                    last.top() >= viewport.top() && last.bottom() <= viewport.bottom(),
                    "新增行必须完整滚动到可视区：{last:?}/{viewport:?}"
                );
                assert_eq!(f32::from(last.size.height), CONTROL);
            }
            window.click(("remove-row", 6usize), cx);
            assert_eq!(
                f32::from(window.find("parameter-list").bounds().size.height),
                parameter_height(6)
            );
            window.click(("remove-row", 5usize), cx);
            assert_eq!(
                f32::from(window.find("parameter-list").bounds().size.height),
                parameter_height(5)
            );
            // 从六行缩成五行后，预览应得到完整的一行+行距，而不是留下空白。
            let before = window.find("preview-frame").bounds().size.height;
            window.click(("remove-row", 4usize), cx);
            assert_eq!(
                window.find("preview-frame").bounds().size.height - before,
                px(CONTROL + GAP)
            );
        })
        .unwrap();
    }

    #[gpui::test]
    fn titlebar_keeps_tabs_and_drag_space_inside_each_width(cx: &mut TestAppContext) {
        use crate::state::{CommandTab, TabData};
        use crate::tokens::GAP;
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        for width in [640., 760., 850., 1200.] {
            let handle = cx.open_window(size(px(width), px(800.)), |window, cx| {
                let view = cx.new(|cx| {
                    let mut view = CommandWorkspace::new(window, cx);
                    for id in 1..12 {
                        let data = TabData {
                            name: format!("这是一个很长的标签名称-{id}"),
                            ..TabData::example()
                        };
                        view.tabs.push(CommandTab::new(id, data, window, cx));
                    }
                    view.select_tab(0, window, cx);
                    view
                });
                Root::new(view, window, cx)
            });
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                let g = super::geometry(window);
                let right = |id: &str| px(g[id][0] + g[id][2]);
                assert_eq!(g["tab-strip"][0], f32::from(right("previous-tab")) + GAP);
                assert!(right("tab-strip") <= px(g["add-tab-title"][0]));
                assert_eq!(g["add-tab-title"][0] - f32::from(right("tab-strip")), GAP);
                assert_eq!(g["next-tab"][0] - f32::from(right("add-tab-title")), GAP);
                assert_eq!(g["title-drag-space"][0] - f32::from(right("next-tab")), GAP);
                assert!(
                    g["title-drag-space"][2] >= 40.,
                    "{width}px 下标题栏拖动区被挤没了"
                );
                assert_eq!(
                    g["previous-tab"][0] - f32::from(right("application-menu")),
                    GAP
                );
                assert_eq!(
                    g["theme-toggle"][0] - f32::from(right("title-drag-space")),
                    GAP
                );
                assert_eq!(g["save"][0] - f32::from(right("theme-toggle")), GAP);
                assert_eq!(g["settings"][0] - f32::from(right("save")), GAP);
                assert!(right("settings") <= px(width));
                assert!(window.try_find("config-toolbar").is_none());
                assert!(window.try_find("refresh-preview").is_none());
                let reload = window.find("reload").bounds();
                let copy = window.find("copy-command").bounds();
                assert_eq!(reload.top(), copy.top(), "重新加载应在预览操作行");
                assert!(reload.right() < copy.left(), "重新加载应位于复制左边");
                assert!(!g.contains_key("tab-selector"), "顶部标签下拉框已取消");
                assert!(g["tab-sidebar"][2] >= 160., "左侧标签栏必须可见");
                window.remove_window();
            })
            .unwrap();
        }
    }

    #[gpui::test]
    fn application_menu_uses_standard_row_height(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let handle = cx.open_window(size(px(640.), px(660.)), |window, cx| {
            let view = cx.new(|cx| CommandWorkspace::new(window, cx));
            Root::new(view, window, cx)
        });
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("application-menu", cx);
            for (first, second) in [
                ("新建标签", "复制当前标签"),
                ("启用全部参数", "禁用全部参数"),
            ] {
                let first = window.find(first).bounds();
                let second = window.find(second).bounds();
                assert_eq!(first.size.height, px(32.));
                assert_eq!(second.size.height, px(32.));
                // Kit 0.6.6 的 PopupMenu 仍带固定 2px 行间隔；不得把它算进控件高度。
                assert_eq!(second.top() - first.bottom(), px(2.));
            }
            let popup = window.find("popup-menu").bounds();
            assert!(popup.left() >= px(0.) && popup.right() <= px(640.));
            assert!(popup.bottom() <= px(660.));
            window.press("escape", cx);
        })
        .unwrap();
    }

    #[gpui::test]
    fn manually_added_options_stay_unchecked_until_enabled(cx: &mut TestAppContext) {
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
        cx.update_window(handle.into(), |_, w, cx| {
            let states = view.read(cx).tabs[0]
                .rows
                .iter()
                .map(|r| r.enabled)
                .collect::<Vec<_>>();
            let preview = view.read(cx).preview.read(cx).value().to_string();
            w.click("add-parameter", cx);
            let new_row = view.read(cx).tabs[0].rows.last().unwrap();
            assert!(!new_row.enabled);
            assert_eq!(
                view.read(cx).tabs[0].rows[..states.len()]
                    .iter()
                    .map(|r| r.enabled)
                    .collect::<Vec<_>>(),
                states
            );
            let id = new_row.id;
            w.input("--new-option", cx); // The new option field keeps focus after adding.
            assert_eq!(w.find(("option", id)).value(), Some("--new-option"));
            assert!(!view.read(cx).tabs[0].rows.last().unwrap().enabled);
            assert_eq!(view.read(cx).preview.read(cx).value().as_ref(), preview);
            let data = view.read(cx).tabs[0].data(cx);
            let saved = serde_json::to_value(data).unwrap();
            assert_eq!(saved["functions"][states.len()]["enabled"], false);
            w.click(("toggle-row", id), cx);
            assert!(view.read(cx).tabs[0].rows.last().unwrap().enabled);
            assert!(view
                .read(cx)
                .preview
                .read(cx)
                .value()
                .contains("--new-option"));
            w.click("add-tab-title", cx);
            assert!(
                !view.read(cx).tabs[1].rows[0].enabled,
                "空白新标签里的首行也默认不勾选"
            );
            // Importing an older config must retain its historic enabled default.
            let legacy: crate::state::RowData =
                serde_json::from_value(serde_json::json!({"function":"--existing"})).unwrap();
            assert!(legacy.enabled);
            w.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn compact_note_hover_shows_live_note_and_click_still_edits(cx: &mut TestAppContext) {
        use crate::i18n::{self, Language};
        cx.update(gpui::init);
        for mode in [Theme::Dark, Theme::Light] {
            cx.update(|cx| theme::apply(mode, cx));
            let mut workspace = None;
            let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
                let view = cx.new(|cx| CommandWorkspace::new(w, cx));
                workspace = Some(view.clone());
                Root::new(view, w, cx)
            });
            let view = workspace.unwrap();
            cx.update_window(handle.into(), |_, w, cx| {
                assert!(
                    w.try_find(("note", 0usize)).is_none(),
                    "紧凑模式应把备注折叠为按钮"
                );
                assert_eq!(w.find(("note-details", 0usize)).label(), Some("输入视频"));
                w.click(("note-details", 0usize), cx);
                assert_eq!(w.find("note-detail-input").value(), Some("输入视频"));
                let note = view.read(cx).tabs[0].rows[0].note.clone();
                note.update(cx, |s, cx| s.set_value("需要保留的备注 / 12", w, cx));
                w.render_frame(cx);
                assert_eq!(
                    w.find("note-detail-input").value(),
                    Some("需要保留的备注 / 12")
                );
                w.close_dialog(cx);
                w.render_frame(cx);
                assert_eq!(
                    w.find(("note-details", 0usize)).label(),
                    Some("需要保留的备注 / 12")
                );
                assert!(view.read(cx).tabs[0].rows[0]
                    .data(cx)
                    .note
                    .contains("需要保留的备注"));
                note.update(cx, |s, cx| s.set_value("  ", w, cx));
                w.render_frame(cx);
                assert_eq!(
                    w.find(("note-details", 0usize)).label(),
                    Some("编辑此参数的备注")
                );
                i18n::apply(Language::English, cx);
                w.render_frame(cx);
                assert_eq!(
                    w.find(("note-details", 0usize)).label(),
                    Some("Edit parameter note")
                );
                note.update(cx, |s, cx| s.set_value("Keep original text", w, cx));
                w.render_frame(cx);
                assert_eq!(
                    w.find(("note-details", 0usize)).label(),
                    Some("Keep original text")
                );
                w.remove_window();
                i18n::apply(Language::Chinese, cx);
            })
            .unwrap();
        }
    }

    #[gpui::test]
    fn pasting_a_file_into_single_line_input_replaces_old_value(cx: &mut TestAppContext) {
        use gpui::{ClipboardEntry, ClipboardItem, ExternalPaths};
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
        let path = r"C:\media\Lumina Charis 1-147\episode.ass";
        cx.update_window(handle.into(), |_, window, cx| {
            assert_eq!(window.find("program").value(), Some("ffmpeg"));
            window.click("program", cx);
            let mut paths = ExternalPaths::default();
            paths.0.push(std::path::PathBuf::from(path));
            cx.write_to_clipboard(ClipboardItem {
                entries: vec![ClipboardEntry::ExternalPaths(paths)],
            });
            window.press(
                if cfg!(target_os = "macos") {
                    "cmd-v"
                } else {
                    "ctrl-v"
                },
                cx,
            );
            assert_eq!(window.find("program").value(), Some(path));
            assert!(view.read(cx).has_unsaved_edits_now(cx));
            window.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn save_shortcut_persists_focused_editor_without_losing_focus(cx: &mut TestAppContext) {
        use gpui::Focusable;
        use std::time::{SystemTime, UNIX_EPOCH};
        let dir = std::env::temp_dir().join(format!(
            "ecr-gpui-save-shortcut-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let store = crate::backend::ConfigStore::at(dir.join("config.json"), dir.join("backup"));
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut workspace = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |window, cx| {
            let view = cx.new(|cx| {
                CommandWorkspace::new_with_backend(window, cx, Some(store.clone()), None, None)
            });
            workspace = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = workspace.unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("program", cx);
            view.update(cx, |workspace, cx| {
                let tab_id = workspace.tabs[0].id;
                workspace.tabs[0].program.update(cx, |state, cx| {
                    state.set_value("echo shortcut", window, cx);
                });
                workspace.changed(tab_id, window, cx);
            });
            assert!(view.read(cx).has_unsaved_edits());
            window.press(
                if cfg!(target_os = "macos") {
                    "cmd-s"
                } else {
                    "ctrl-s"
                },
                cx,
            );
            assert!(!view.read(cx).has_unsaved_edits());
            assert_eq!(
                store.load().unwrap().unwrap()["tabs"][0]["program"],
                view.read(cx).tabs[0].program.read(cx).value().to_string()
            );
            assert!(view.read(cx).tabs[0]
                .program
                .focus_handle(cx)
                .is_focused(window));
            window.remove_window();
        })
        .unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[gpui::test]
    fn editing_tabs_and_parameters(cx: &mut TestAppContext) {
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
            let preview = view.read(cx).preview.read(cx).value().to_string();
            let expected = if cfg!(windows) {
                "ffmpeg -i \"input file.mp4\" -c:v libx264 output.mp4"
            } else {
                "ffmpeg -i 'input file.mp4' -c:v libx264 output.mp4"
            };
            assert_eq!(
                preview, expected,
                "预览文本必须按目标 Shell 的引用规则构建命令并保留参数分隔",
            );
            window.click("command-name", cx);
            window.press(
                if cfg!(target_os = "macos") {
                    "cmd-a"
                } else {
                    "ctrl-a"
                },
                cx,
            );
            window.input("中文命令", cx);
            assert_eq!(window.find("command-name").value(), Some("中文命令"));
            window.click("select-none", cx);
            assert!(view.read(cx).tabs[0].rows.iter().all(|r| !r.enabled));
            window.click("select-all", cx);
            assert!(view.read(cx).tabs[0].rows.iter().all(|r| r.enabled));
            let before_parse = view.read(cx).tabs[0].command(cx);
            window.click("append-command", cx);
            window.input("--quality 22", cx);
            assert_eq!(
                view.read(cx).tabs[0].command(cx),
                before_parse,
                "粘贴原文不能直接加入运行命令"
            );
            assert_eq!(
                view.read(cx).preview.read(cx).value().as_ref(),
                before_parse
            );
            window.click("append-parse", cx);
            assert_eq!(view.read(cx).tabs[0].rows.len(), 4);
            assert_eq!(
                view.read(cx).tabs[0].append.read(cx).value().as_ref(),
                "",
                "解析后清空粘贴输入"
            );
            assert!(view
                .read(cx)
                .preview
                .read(cx)
                .value()
                .contains("--quality 22"));
            window.click("other-args", cx);
            window.input(" | wc -c", cx);
            assert!(
                view.read(cx).tabs[0].command(cx).ends_with(" | wc -c"),
                "其他参数按原文参与实际命令"
            );
            window.click("add-tab-title", cx);
            assert_eq!(view.read(cx).tabs.len(), 2);
            window.click("add-tab-title", cx);
            assert_eq!(view.read(cx).tabs.len(), 3);
            window.click("add-parameter", cx);
            assert_eq!(view.read(cx).tabs[2].rows.len(), 2);
            let first = window.find(("sidebar-tab", 0usize)).bounds();
            let second = window.find(("sidebar-tab", 1usize)).bounds();
            assert_eq!(first.size.height, px(40.));
            assert_eq!(second.top() - first.bottom(), px(4.));
            assert!(first.left() >= window.find("tab-sidebar").bounds().left());
            window.click(("sidebar-tab", 0usize), cx);
            assert_eq!(view.read(cx).active, 0);
            window.click(("sidebar-tab", 2usize), cx);
            assert_eq!(view.read(cx).active, 2);
            window.click("program", cx);
            window.input("echo", cx);
            window.click("copy-command", cx);
            assert_eq!(
                cx.read_from_clipboard().and_then(|i| i.text()),
                Some("echo".into())
            );
            // 没有注入磁盘存储的纯 UI fixture 不得假报已保存；Shell 行为由后端集成测试覆盖。
            let status = view.read(cx).status.clone();
            window.click("save", cx);
            assert_eq!(view.read(cx).status, status);
            assert!(!view.read(cx).show_log);
        })
        .unwrap();
    }
    #[gpui::test]
    fn dropdown_rows_have_only_one_hover_highlight(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            for mode in [Theme::Dark, Theme::Light] {
                theme::apply(mode, cx);
                let t = gpui::component::Theme::global(cx);
                assert_eq!(t.tokens.list_hover.a, 0., "列表外层不应叠加 hover 高光");
                assert!(t.accent.a > 0., "内层选项仍应保留 hover 高光");
            }
        });
    }

    #[gpui::test]
    fn sidebar_edge_drag_resizes_and_preserves_preferred_width(cx: &mut TestAppContext) {
        use gpui::{point, Focusable};
        cx.update(gpui::init);
        for mode in [Theme::Dark, Theme::Light] {
            cx.update(|cx| theme::apply(mode, cx));
            let mut view = None;
            let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
                let entity = cx.new(|cx| CommandWorkspace::new(w, cx));
                view = Some(entity.clone());
                Root::new(entity, w, cx)
            });
            let view = view.unwrap();
            cx.update_window(handle.into(), |_, w, cx| {
                view.update(cx, |v, cx| {
                    v.reduced_motion = true;
                    cx.notify();
                });
                w.click("program", cx);
                let input_id = view.read(cx).tabs[0].program.entity_id();
                let input_focus = view.read(cx).tabs[0].program.focus_handle(cx);
                assert!(input_focus.is_focused(w));
                let original = w.find("tab-sidebar").bounds();
                let grip = w.find("sidebar-resize-handle").bounds();
                assert!(
                    grip.size.width >= px(8.) && (grip.right() - original.right()).abs() <= px(1.),
                    "{grip:?} / {original:?}"
                );
                w.drag(
                    grip.center(),
                    point(grip.center().x + px(80.), grip.center().y),
                    cx,
                );
                assert!(
                    (w.find("tab-sidebar").bounds().size.width - px(288.)).abs() <= px(1.),
                    "拖动边缘应立即改变侧栏宽度"
                );
                assert_eq!(view.read(cx).tabs[0].program.entity_id(), input_id);
                assert!(input_focus.is_focused(w), "拖动侧栏不能重建输入并丢失焦点");
                assert!(w.find("append-command").bounds().size.width >= px(48.));
                let grip = w.find("sidebar-resize-handle").bounds();
                w.drag(
                    grip.center(),
                    point(grip.center().x + px(300.), grip.center().y),
                    cx,
                );
                assert!(
                    w.find("tab-sidebar").bounds().size.width <= px(383.),
                    "最大宽度不能挤掉编辑区"
                );
                let grip = w.find("sidebar-resize-handle").bounds();
                w.drag(
                    grip.center(),
                    point(grip.center().x - px(210.), grip.center().y),
                    cx,
                );
                assert!(
                    (w.find("tab-sidebar").bounds().size.width - px(172.)).abs() <= px(1.),
                    "向左拖应实时收窄，不能只展开"
                );
                w.click("collapse-sidebar", cx);
                assert_eq!(w.find("tab-sidebar").bounds().size.width, px(56.));
                w.click("collapse-sidebar", cx);
                assert!(
                    (w.find("tab-sidebar").bounds().size.width - px(172.)).abs() <= px(1.),
                    "折叠不覆盖手动宽度"
                );
                w.click("collapse-sidebar", cx);
                let grip = w.find("sidebar-resize-handle").bounds().center();
                w.drag(grip, point(grip.x + px(160.), grip.y), cx);
                assert!(!view.read(cx).sidebar_collapsed, "折叠时向外拖动应展开");
                assert!((w.find("tab-sidebar").bounds().size.width - px(216.)).abs() <= px(1.));
                w.remove_window();
            })
            .unwrap();
        }
    }

    #[gpui::test]
    fn sidebar_width_clamps_temporarily_and_restores_after_save(cx: &mut TestAppContext) {
        use crate::backend::ConfigStore;
        use gpui::point;
        use std::time::{SystemTime, UNIX_EPOCH};
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Light, cx);
        });
        let dir = std::env::temp_dir().join(format!(
            "ecr-sidebar-width-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = ConfigStore::at(dir.join("config.json"), dir.join("backup"));
        let mut workspace = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            let view = cx.new(|cx| {
                CommandWorkspace::new_with_backend(w, cx, Some(store.clone()), None, None)
            });
            workspace = Some(view.clone());
            Root::new(view, w, cx)
        });
        let view = workspace.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            view.update(cx, |v, cx| {
                v.reduced_motion = true;
                cx.notify();
            });
            let grip = w.find("sidebar-resize-handle").bounds().center();
            w.drag(grip, point(grip.x + px(170.), grip.y), cx);
            assert!((w.find("tab-sidebar").bounds().size.width - px(378.)).abs() <= px(1.));
        })
        .unwrap();
        cx.simulate_window_resize(handle.into(), size(px(640.), px(800.)));
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            assert!(
                (w.find("tab-sidebar").bounds().size.width - px(192.)).abs() <= px(1.),
                "窄窗口只临时钳制"
            );
            assert!(
                w.find("append-command").bounds().size.width >= px(45.),
                "粘贴框不应消失"
            );
        })
        .unwrap();
        cx.simulate_window_resize(handle.into(), size(px(850.), px(800.)));
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            assert!(
                (w.find("tab-sidebar").bounds().size.width - px(378.)).abs() <= px(1.),
                "窗口恢复后使用记忆宽度"
            );
            w.click("save", cx);
            w.click("collapse-sidebar", cx);
            w.click("save", cx);
            assert!(!view.read(cx).has_unsaved_edits());
            w.remove_window();
        })
        .unwrap();
        let saved = store.load().unwrap().unwrap();
        assert_eq!(saved["gpui"]["sidebar_width"], 378.);
        assert_eq!(saved["gpui"]["sidebar_collapsed"], true);
        let handle = cx.open_window(size(px(640.), px(800.)), |w, cx| {
            Root::new(
                cx.new(|cx| {
                    CommandWorkspace::new_with_backend(
                        w,
                        cx,
                        Some(store.clone()),
                        Some(saved),
                        None,
                    )
                }),
                w,
                cx,
            )
        });
        cx.update_window(handle.into(), |_, w, cx| {
            assert_eq!(w.find("tab-sidebar").bounds().size.width, px(56.));
            w.click("collapse-sidebar", cx);
            assert!((w.find("tab-sidebar").bounds().size.width - px(192.)).abs() <= px(1.));
        })
        .unwrap();
        cx.simulate_window_resize(handle.into(), size(px(850.), px(800.)));
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            assert!((w.find("tab-sidebar").bounds().size.width - px(378.)).abs() <= px(1.));
            w.remove_window();
        })
        .unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[gpui::test]
    fn sidebar_width_springs_and_reverses_without_changing_inputs(cx: &mut TestAppContext) {
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
        let program_value =
            cx.update(|cx| view.read(cx).tabs[0].program.read(cx).value().to_string());
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            assert_eq!(w.find("tab-sidebar").bounds().size.width, px(208.));
            w.click("collapse-sidebar", cx);
            assert!(view.read(cx).sidebar_collapsed);
            assert_eq!(
                w.find("tab-sidebar").bounds().size.width,
                px(208.),
                "弹簧起点不应瞬移到终点"
            );
        })
        .unwrap();
        cx.background_executor
            .advance_clock(std::time::Duration::from_millis(60));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            let sidebar = w.find("tab-sidebar").bounds();
            let grip = w.find("sidebar-resize-handle").bounds();
            assert!(
                sidebar.size.width < px(208.) && sidebar.size.width > px(52.),
                "侧栏应处于折叠途中: {sidebar:?}"
            );
            assert!(
                (grip.right() - sidebar.right()).abs() <= px(1.),
                "无装饰的拖动热区仍应跟随侧栏右边界"
            );
            assert_eq!(grip.size.width, px(10.));
            w.click("collapse-sidebar", cx);
            assert!(!view.read(cx).sidebar_collapsed, "途中反向不应等旧动画完成");
        })
        .unwrap();
        cx.background_executor
            .advance_clock(std::time::Duration::from_millis(900));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            assert!((w.find("tab-sidebar").bounds().size.width - px(208.)).abs() < px(1.));
            assert_eq!(
                view.read(cx).tabs[0].program.read(cx).value().as_ref(),
                program_value
            );
            w.click("collapse-sidebar", cx);
            view.update(cx, |v, cx| {
                v.reduced_motion = true;
                cx.notify();
            });
            w.render_frame(cx);
            assert_eq!(
                w.find("tab-sidebar").bounds().size.width,
                px(56.),
                "减少动效时立即折叠"
            );
            w.click("collapse-sidebar", cx);
            assert_eq!(
                w.find("tab-sidebar").bounds().size.width,
                px(208.),
                "减少动效时立即展开"
            );
            w.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn sidebar_rows_keep_geometry_and_icons_when_collapsed(cx: &mut TestAppContext) {
        cx.update(gpui::init);
        for mode in [Theme::Dark, Theme::Light] {
            cx.update(|cx| theme::apply(mode, cx));
            for (width, count) in [(640., 3), (850., 3), (640., 12), (850., 12)] {
                let mut workspace = None;
                let handle = cx.open_window(size(px(width), px(800.)), |w, cx| {
                    let entity = cx.new(|cx| CommandWorkspace::new(w, cx));
                    workspace = Some(entity.clone());
                    Root::new(entity, w, cx)
                });
                let view = workspace.unwrap();
                cx.update_window(handle.into(), |_, w, cx| {
                    view.update(cx, |v, _| v.reduced_motion = true);
                    for _ in 1..count {
                        w.click("add-tab-title", cx);
                    }
                    w.render_frame(cx);
                    let before = w.find("tab-sidebar").bounds();
                    assert!(before.size.width >= px(160.));
                    assert!(w.try_find("tab-selector").is_none());
                    assert_eq!(
                        w.find(("sidebar-tab", 0usize)).bounds().size.height,
                        px(40.)
                    );
                    assert!(w.try_find(("sidebar-icon", 0usize)).is_some());
                    w.click("collapse-sidebar", cx);
                    assert_eq!(w.find("tab-sidebar").bounds().size.width, px(56.));
                    assert!(w.try_find(("sidebar-icon", 0usize)).is_some());
                    assert!(w.try_find(("sidebar-edit-icon", 0usize)).is_none());
                    w.click(("sidebar-icon", 0usize), cx);
                    assert_eq!(view.read(cx).active, 0, "折叠时点击图标仍应切换标签");
                    assert!(!w.has_active_dialog(cx), "折叠时图标不应抢走切换入口");
                    w.click("collapse-sidebar", cx);
                    assert_eq!(w.find("tab-sidebar").bounds().size.width, before.size.width);
                    assert!(
                        w.try_find(("sidebar-edit-icon", 0usize)).is_none(),
                        "侧栏不再显示图标设置按钮"
                    );
                    w.remove_window();
                })
                .unwrap();
            }
        }
    }

    #[gpui::test]
    fn collapsed_sidebar_dirty_state_does_not_change_icon_or_highlight_geometry(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui::init);
        for mode in [Theme::Dark, Theme::Light] {
            cx.update(|cx| theme::apply(mode, cx));
            let mut workspace = None;
            let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
                let view = cx.new(|cx| CommandWorkspace::new(w, cx));
                workspace = Some(view.clone());
                Root::new(view, w, cx)
            });
            let view = workspace.unwrap();
            cx.update_window(handle.into(), |_, w, cx| {
                w.click("add-tab-title", cx);
                w.click("add-tab-title", cx);
                view.update(cx, |v, cx| {
                    v.reduced_motion = true;
                    v.sidebar_collapsed = true;
                    cx.notify();
                });
                let mut baseline = None;
                // The first row hosts the sliding backgrounds: its dirty marker must not
                // narrow the backgrounds for any other row or squeeze its own icon.
                for dirty in [false, true, false] {
                    view.update(cx, |v, cx| {
                        for tab in &mut v.tabs {
                            tab.dirty = dirty;
                        }
                        cx.notify();
                    });
                    for selected in 0..3 {
                        view.update(cx, |v, cx| v.select_tab(selected, w, cx));
                        w.render_frame(cx);
                        let pill = w.find("sidebar-selection-indicator").bounds();
                        let selected_row = w.find(("sidebar-tab", selected)).bounds();
                        assert_eq!(
                            pill, selected_row,
                            "高亮应完整覆盖同一行，不因未保存右边框变窄"
                        );
                        let mut icons = Vec::new();
                        for id in 0..3usize {
                            let row = w.find(("sidebar-tab", id)).bounds();
                            let icon = w.find(("sidebar-icon", id)).bounds();
                            assert_eq!(
                                icon.size,
                                size(px(24.), px(24.)),
                                "折叠图标不应被右边框挤压"
                            );
                            assert!(
                                (icon.center().x - row.center().x).abs() <= px(0.5),
                                "折叠图标应居中"
                            );
                            icons.push(icon);
                            let marker = w.try_find(("sidebar-dirty-dot", id));
                            assert_eq!(marker.is_some(), dirty);
                            if let Some(marker) = marker {
                                let dot = marker.bounds();
                                assert_eq!(dot.size, size(px(4.), px(4.)));
                                assert!(dot.left() >= row.left() && dot.right() <= row.right());
                                assert!(dot.top() >= row.top() && dot.bottom() <= row.bottom());
                            }
                        }
                        if let Some(previous) = &baseline {
                            assert_eq!(&icons, previous);
                        } else {
                            baseline = Some(icons);
                        }
                    }
                }
                w.remove_window();
            })
            .unwrap();
        }
    }

    #[gpui::test]
    fn top_tabs_scroll_the_sidebar_to_the_same_selected_item(cx: &mut TestAppContext) {
        use crate::state::{CommandTab, TabData};
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut workspace = None;
        let handle = cx.open_window(size(px(760.), px(700.)), |w, cx| {
            let view = cx.new(|cx| {
                let mut view = CommandWorkspace::new(w, cx);
                for id in 1..28 {
                    view.tabs.push(CommandTab::new(
                        id,
                        TabData {
                            name: format!("Tab {id}"),
                            ..Default::default()
                        },
                        w,
                        cx,
                    ));
                }
                view
            });
            workspace = Some(view.clone());
            Root::new(view, w, cx)
        });
        let view = workspace.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            view.update(cx, |v, cx| {
                v.reduced_motion = true;
                v.select_tab(27, w, cx);
            });
            w.render_frame(cx);
            let sidebar = w.find("sidebar-items").bounds();
            let selected = w.find(("sidebar-tab", 27usize)).bounds();
            assert!(
                selected.top() >= sidebar.top() - px(1.)
                    && selected.bottom() <= sidebar.bottom() + px(1.),
                "顶部切页要滚动左侧列表到当前项: {selected:?} / {sidebar:?}"
            );
            w.click("collapse-sidebar", cx);
            w.render_frame(cx);
            let selected = w.find(("sidebar-tab", 27usize)).bounds();
            let sidebar = w.find("sidebar-items").bounds();
            assert!(
                selected.top() >= sidebar.top() - px(1.)
                    && selected.bottom() <= sidebar.bottom() + px(1.)
            );
            assert_eq!(
                selected.size.height,
                px(40.),
                "折叠后选中标签不应被滚动视口裁切"
            );
            w.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn icon_browser_search_categories_scroll_and_keyboard_selection(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Light, cx);
        });
        let mut workspace = None;
        let handle = cx.open_window(size(px(640.), px(660.)), |w, cx| {
            let view = cx.new(|cx| CommandWorkspace::new(w, cx));
            workspace = Some(view.clone());
            Root::new(view, w, cx)
        });
        let view = workspace.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            w.click(("sidebar-icon", 0usize), cx);
        })
        .unwrap();
        cx.background_executor
            .advance_clock(std::time::Duration::from_millis(350));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            let grid = w.find("icon-grid").bounds();
            let first = w.find("icon-choice-python").bounds();
            let second = w.find("icon-choice-rust").bounds();
            assert_eq!(
                first.size,
                size(px(44.), px(44.)),
                "纯图标格子，不是名称列表"
            );
            assert_eq!(second.left() - first.left(), px(44.));
            for id in [
                "icon-search",
                "icon-choice-random",
                "icon-choice-default",
                "icon-source",
                "icon-source-apply",
                "icon-category-8",
            ] {
                let bounds = w.find(id).bounds();
                assert!(
                    bounds.left() >= px(0.) && bounds.right() <= px(640.),
                    "{id}: {bounds:?}"
                );
                assert!(
                    bounds.top() >= px(0.) && bounds.bottom() <= px(660.),
                    "{id}: {bounds:?}"
                );
            }
            assert!(grid.bottom() < w.find("icon-source").bounds().top());
            let last = &tab_icons::CATALOG.last().unwrap().key;
            assert!(
                w.try_find(format!("icon-choice-{last}")).is_none(),
                "不应一次构建 1800 多个格子"
            );
            w.scroll(
                "icon-grid",
                gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(-100000.))),
                cx,
            );
            w.render_frame(cx);
            assert!(
                w.find(format!("icon-choice-{last}")).visible(),
                "必须能滚动浏览整个图库"
            );
            w.click("icon-category-1", cx); // brands resets scroll
            assert!(w.find("icon-choice-python").visible());
            assert!(w.try_find(format!("icon-choice-{last}")).is_none());
            w.click("icon-category-0", cx);
            w.click("icon-search", cx);
            w.input("no-such-icon-xyz", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            assert!(w.try_find("icon-choice-python").is_none());
            let previous_icon = view.read(cx).tabs[0].icon_source.clone();
            let previously_dirty = view.read(cx).tabs[0].dirty;
            w.click("icon-choice-random", cx);
            assert!(w.has_active_dialog(cx), "空结果时随机选择不能关闭选择器");
            assert_eq!(view.read(cx).tabs[0].icon_source, previous_icon);
            assert_eq!(view.read(cx).tabs[0].dirty, previously_dirty);
            w.click("icon-search", cx);
            w.press(
                if cfg!(target_os = "macos") {
                    "cmd-a"
                } else {
                    "ctrl-a"
                },
                cx,
            );
            w.input("火箭", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            assert!(w.find("icon-choice-lucide/rocket").visible());
            w.press("down", cx); // search -> grid, without changing the command editor
            assert_eq!(w.find("icon-grid").focused(), Some(true));
            w.press("enter", cx);
            assert_eq!(
                view.read(cx).tabs[0].icon_source.as_deref(),
                Some("builtin:lucide/rocket")
            );
            assert!(view.read(cx).tabs[0].dirty);
            let data = view.read(cx).tabs[0].data(cx);
            let restored: crate::state::TabData =
                serde_json::from_value(serde_json::to_value(&data).unwrap()).unwrap();
            assert_eq!(restored.icon_source, data.icon_source);
            w.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn random_svg_choice_respects_the_current_search(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut workspace = None;
        let handle = cx.open_window(size(px(760.), px(700.)), |w, cx| {
            let view = cx.new(|cx| CommandWorkspace::new(w, cx));
            workspace = Some(view.clone());
            Root::new(view, w, cx)
        });
        let view = workspace.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            w.click(("sidebar-icon", 0usize), cx);
        })
        .unwrap();
        cx.background_executor
            .advance_clock(std::time::Duration::from_millis(350));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            w.click("icon-search", cx);
            w.input("python", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            assert_eq!(w.find("icon-search").value(), Some("python"));
            assert!(w.find("icon-choice-python").visible());
            let allowed = tab_icons::search("python", tab_icons::Category::All)
                .into_iter()
                .map(|index| tab_icons::CATALOG[index].key.as_str())
                .collect::<Vec<_>>();
            w.click("icon-choice-random", cx);
            let selected = view.read(cx).tabs[0].icon_source.clone().unwrap();
            assert!(
                allowed
                    .iter()
                    .any(|key| format!("builtin:{key}") == selected),
                "随机 SVG 必须从当前过滤结果中选择：{selected} / {allowed:?}"
            );
            assert!(view.read(cx).tabs[0].dirty);
            assert!(!w.has_active_dialog(cx));
            let data = view.read(cx).tabs[0].data(cx);
            let restored: crate::state::TabData =
                serde_json::from_value(serde_json::to_value(data).unwrap()).unwrap();
            assert_eq!(restored.icon_source.as_deref(), Some(selected.as_str()));
            w.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn sidebar_surface_is_distinct_from_editor_in_both_themes(cx: &mut TestAppContext) {
        cx.update(|cx| gpui::init(cx));
        for mode in [Theme::Dark, Theme::Light] {
            cx.update(|cx| theme::apply(mode, cx));
            let (sidebar, content) = cx.update(|cx| {
                let palette = theme::palette(cx);
                (palette.sidebar, palette.panel)
            });
            assert_ne!(sidebar, content, "{mode:?} 侧栏背景必须区别于编辑区");
        }
    }

    #[gpui::test]
    fn dark_pill_tab_hover_uses_the_application_hover_color(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        cx.update(|cx| {
            let secondary = gpui::component::Theme::global(cx).tokens.secondary.color;
            assert_eq!(secondary, gpui::Hsla::from(gpui::rgb(0x2b2f2f)));
        });
    }

    #[gpui::test]
    fn sidebar_hover_tracks_scrolled_configuration_rows(cx: &mut TestAppContext) {
        use crate::state::{CommandTab, TabData};
        use crate::tokens::{
            SIDEBAR_HIGHLIGHT_SCROLL_GAP, SIDEBAR_SCROLL_LANE_RIGHT, SIDEBAR_SCROLL_THUMB_INSET,
            SIDEBAR_SCROLL_THUMB_MAX_WIDTH,
        };
        use gpui::{PlatformInput, ScrollDelta, ScrollWheelEvent};
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut workspace = None;
        let handle = cx.open_window(size(px(760.), px(700.)), |w, cx| {
            let view = cx.new(|cx| {
                let mut view = CommandWorkspace::new(w, cx);
                view.reduced_motion = true;
                for id in 1..28 {
                    view.tabs.push(CommandTab::new(
                        id,
                        TabData {
                            name: format!("Tab {id}"),
                            ..TabData::example()
                        },
                        w,
                        cx,
                    ));
                }
                view
            });
            workspace = Some(view.clone());
            Root::new(view, w, cx)
        });
        let view = workspace.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            view.update(cx, |v, cx| v.select_tab(26, w, cx));
            w.render_frame(cx);
            let viewport = w.find("sidebar-items").bounds();
            let lane = w.find("sidebar-scrollbar-lane").bounds();
            let resize_handle = w.find("sidebar-resize-handle").bounds();
            assert_eq!(
                lane.right(),
                viewport.right() - px(SIDEBAR_SCROLL_LANE_RIGHT)
            );
            assert_eq!(lane.size.height, viewport.size.height);
            let widest_thumb_left =
                lane.right() - px(SIDEBAR_SCROLL_THUMB_INSET + SIDEBAR_SCROLL_THUMB_MAX_WIDTH);
            for id in [25usize, 26usize] {
                assert!(
                    widest_thumb_left - w.find(("sidebar-tab", id)).bounds().right()
                        >= px(SIDEBAR_HIGHLIGHT_SCROLL_GAP),
                    "配置高亮与最宽滚动滑块之间至少保留 3px"
                );
            }
            assert!(
                lane.right() <= resize_handle.left(),
                "滚动条不能与侧栏宽度拖动区重叠：{lane:?} / {resize_handle:?}"
            );
            for id in [25usize, 26usize] {
                w.hover(("sidebar-tab", id), cx);
                let row = w.find(("sidebar-tab", id)).bounds();
                let indicator = w.find(("sidebar-hover-row-indicator", id)).bounds();
                assert!(
                    row.top() >= viewport.top() && row.bottom() <= viewport.bottom(),
                    "测试行须在滚动视口内：{row:?} / {viewport:?}"
                );
                assert_eq!(
                    indicator, row,
                    "长列表 hover 背景必须绑定到实际行：配置 {id}"
                );
            }
            let selected_row = w.find(("sidebar-tab", 26usize)).bounds();
            assert_eq!(
                w.find(("sidebar-selection-row-indicator", 26usize))
                    .bounds(),
                selected_row,
                "长列表选中背景必须绑定到实际行"
            );

            // Wheel scrolling moves rows under a stationary pointer but may not
            // produce a MouseMove. The old row marker must disappear instead of
            // remaining attached to its newly displaced screen position.
            w.hover(("sidebar-tab", 25usize), cx);
            let pointer = w.find(("sidebar-tab", 25usize)).bounds().center();
            w.dispatch_event(
                PlatformInput::ScrollWheel(ScrollWheelEvent {
                    position: pointer,
                    delta: ScrollDelta::Pixels(gpui::point(px(0.), px(44.))),
                    ..Default::default()
                }),
                cx,
            );
            w.render_frame(cx);
            assert!(
                w.try_find(("sidebar-hover-row-indicator", 25usize))
                    .is_none(),
                "滚动必须清除指针静止时过期的 hover 行"
            );
            w.hover(("sidebar-tab", 24usize), cx);
            assert_eq!(
                w.find(("sidebar-hover-row-indicator", 24usize)).bounds(),
                w.find(("sidebar-tab", 24usize)).bounds(),
                "滚动后重新 hover 应精确贴合当前行"
            );
            w.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn sidebar_hover_and_selection_slide_and_reduced_motion_snaps(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Light, cx);
        });
        let mut workspace = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            let view = cx.new(|cx| CommandWorkspace::new(w, cx));
            workspace = Some(view.clone());
            Root::new(view, w, cx)
        });
        let view = workspace.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            view.update(cx, |v, _| v.reduced_motion = true);
            w.click("add-tab-title", cx);
            w.click("add-tab-title", cx);
            w.click(("sidebar-tab", 0usize), cx);
            view.update(cx, |v, cx| {
                v.reduced_motion = false;
                cx.notify();
            });
            w.render_frame(cx);
            w.hover(("sidebar-tab", 2usize), cx);
            let first = w.find(("sidebar-tab", 0usize)).bounds();
            assert_eq!(
                w.find("sidebar-hover-indicator").bounds().top(),
                first.top(),
                "悬停高亮不应瞬移"
            );
            w.click(("sidebar-tab", 2usize), cx);
            assert_eq!(view.read(cx).active, 2);
            assert_eq!(
                w.find("sidebar-selection-indicator").bounds().top(),
                first.top(),
                "点击高亮不应瞬移"
            );
        })
        .unwrap();
        cx.background_executor
            .advance_clock(std::time::Duration::from_millis(60));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            let first = w.find(("sidebar-tab", 0usize)).bounds();
            let last = w.find(("sidebar-tab", 2usize)).bounds();
            for id in ["sidebar-selection-indicator", "sidebar-hover-indicator"] {
                let indicator = w.find(id).bounds();
                assert!(
                    indicator.top() > first.top() && indicator.top() < last.top(),
                    "{id} 应在两行之间滑动: {indicator:?}"
                );
            }
            w.click(("sidebar-tab", 0usize), cx); // reverse while moving
            view.update(cx, |v, cx| {
                v.reduced_motion = true;
                cx.notify();
            });
            w.render_frame(cx);
            assert_eq!(
                w.find("sidebar-selection-indicator").bounds().top(),
                first.top()
            );
            w.hover(("sidebar-tab", 1usize), cx);
            assert_eq!(
                w.find("sidebar-hover-indicator").bounds().top(),
                w.find(("sidebar-tab", 1usize)).bounds().top()
            );
            assert_eq!(view.read(cx).active, 0, "hover 不应切换页面");
            w.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn sidebar_icon_picker_and_defaults_preserve_the_current_page(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut workspace = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            let entity = cx.new(|cx| CommandWorkspace::new(w, cx));
            workspace = Some(entity.clone());
            Root::new(entity, w, cx)
        });
        let view = workspace.unwrap();
        let initial = cx.update(|cx| view.read(cx).preview.read(cx).value().to_string());
        cx.update_window(handle.into(), |_, w, cx| {
            assert!(w.try_find("tab-selector").is_none());
            w.click(("sidebar-icon", 0usize), cx);
        })
        .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(250));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            assert!(w.try_find("icon-choice-rust").is_some());
            w.click("icon-choice-python", cx);
            assert_eq!(
                view.read(cx).tabs[0].icon_source.as_deref(),
                Some("builtin:python")
            );
            assert_eq!(view.read(cx).preview.read(cx).value().as_ref(), initial);
        })
        .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(250));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            w.click(("sidebar-icon", 0usize), cx)
        })
        .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(250));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            assert!(w.has_active_dialog(cx));
            w.click("icon-choice-default", cx);
            assert!(view.read(cx).tabs[0].icon_source.is_none());
            w.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn expanded_sidebar_icon_opens_picker_without_switching_tabs(cx: &mut TestAppContext) {
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
        cx.update_window(handle.into(), |_, w, cx| {
            w.click("add-tab-title", cx);
            assert_eq!(view.read(cx).tabs[view.read(cx).active].id, 1);
            assert!(w.try_find(("sidebar-edit-icon", 0usize)).is_none());
            w.click(("sidebar-icon", 0usize), cx);
            assert_eq!(
                view.read(cx).tabs[view.read(cx).active].id,
                1,
                "编辑背景配置的图标不应切换页面"
            );
            assert!(w.has_active_dialog(cx));
        })
        .unwrap();
        // Wait for the dialog's entry animation before testing its clickable grid.
        cx.background_executor
            .advance_clock(std::time::Duration::from_millis(350));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            w.click("icon-choice-rust", cx);
            assert_eq!(
                view.read(cx).tabs[0].icon_source.as_deref(),
                Some("builtin:rust")
            );
            assert_eq!(view.read(cx).tabs[view.read(cx).active].id, 1);
            w.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn collapsed_sidebar_can_open_icon_picker_from_context_menu(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            Root::new(
                cx.new(|cx| {
                    let mut view = CommandWorkspace::new(w, cx);
                    view.reduced_motion = true;
                    view
                }),
                w,
                cx,
            )
        });
        cx.update_window(handle.into(), |_, w, cx| {
            w.click("collapse-sidebar", cx);
            w.right_click(("sidebar-tab", 0usize), cx);
            w.render_frame(cx);
            assert!(w.try_find("context-icon").is_some());
            w.click("context-icon", cx);
            w.render_frame(cx);
            assert!(w.try_find("icon-choice-rust").is_some());
            w.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn https_svg_download_is_bounded_cached_and_does_not_follow_redirects(cx: &mut TestAppContext) {
        use gpui::http_client::{AsyncBody, FakeHttpClient, RedirectPolicy, Response};
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
            cx.set_http_client(FakeHttpClient::create(|request| async move {
                assert_eq!(request.extensions().get::<RedirectPolicy>(), Some(&RedirectPolicy::NoFollow));
                let (status, body) = match request.uri().path() {
                    "/ok.svg" => (200, b"<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 20 20'><circle cx='10' cy='10' r='8'/></svg>".to_vec()),
                    "/large.svg" => (200, vec![b'x'; crate::tab_icons::MAX_SVG_BYTES + 1]),
                    _ => (302, Vec::new()),
                };
                Ok(Response::builder().status(status).body(AsyncBody::from(body)).unwrap())
            }));
        });
        let mut workspace = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            let view = cx.new(|cx| CommandWorkspace::new(w, cx));
            workspace = Some(view.clone());
            Root::new(view, w, cx)
        });
        let view = workspace.unwrap();
        let source = "https://icons.example.com/ok.svg";
        cx.update_window(handle.into(), |_, _, cx| {
            view.update(cx, |v, cx| v.set_tab_icon_from_source(0, source.into(), cx));
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(view.read(cx).tabs[0].icon_source.as_deref(), Some(source));
            assert!(view.read(cx).tabs[0]
                .icon_svg
                .as_deref()
                .unwrap()
                .contains("<circle"));
        });
        for path in ["/large.svg", "/redirect.svg"] {
            cx.update_window(handle.into(), |_, _, cx| {
                view.update(cx, |v, cx| {
                    v.set_tab_icon_from_source(0, format!("https://icons.example.com{path}"), cx)
                });
            })
            .unwrap();
            cx.run_until_parked();
            cx.update(|cx| {
                assert_eq!(
                    view.read(cx).tabs[0].icon_source.as_deref(),
                    Some(source),
                    "失败不得替换先前有效图标"
                )
            });
        }
        cx.update_window(handle.into(), |_, w, _| w.remove_window())
            .unwrap();
    }

    #[gpui::test]
    fn sidebar_layout_and_builtin_icon_restore_after_save(cx: &mut TestAppContext) {
        use crate::backend::ConfigStore;
        use std::time::{SystemTime, UNIX_EPOCH};
        let dir = std::env::temp_dir().join(format!(
            "ecr-sidebar-config-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::at(dir.join("config.json"), dir.join("backup"));
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut workspace = None;
        let window = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            let view = cx.new(|cx| {
                CommandWorkspace::new_with_backend(w, cx, Some(store.clone()), None, None)
            });
            workspace = Some(view.clone());
            Root::new(view, w, cx)
        });
        let view = workspace.unwrap();
        cx.update_window(window.into(), |_, w, cx| {
            view.update(cx, |v, cx| {
                v.set_tab_icon_from_source(0, "builtin:rust".into(), cx)
            });
            w.click("collapse-sidebar", cx);
            w.click("save", cx);
            assert!(!view.read(cx).has_unsaved_edits());
            w.remove_window();
        })
        .unwrap();
        let saved = store.load().unwrap().unwrap();
        assert_eq!(saved["gpui"]["sidebar_collapsed"], true);
        assert_eq!(saved["tabs"][0]["gpui_icon"], "builtin:rust");
        let second = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            Root::new(
                cx.new(|cx| {
                    CommandWorkspace::new_with_backend(
                        w,
                        cx,
                        Some(store.clone()),
                        Some(saved),
                        None,
                    )
                }),
                w,
                cx,
            )
        });
        cx.update_window(second.into(), |root, w, cx| {
            w.render_frame(cx);
            assert_eq!(w.find("tab-sidebar").bounds().size.width, px(56.));
            let view = root
                .downcast::<Root>()
                .unwrap()
                .read(cx)
                .view()
                .clone()
                .downcast::<CommandWorkspace>()
                .unwrap();
            assert_eq!(
                view.read(cx).tabs[0].icon_source.as_deref(),
                Some("builtin:rust")
            );
            w.remove_window();
        })
        .unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[gpui::test]
    fn local_svg_icon_survives_save_roundtrip_and_bad_svg_cannot_replace_it(
        cx: &mut TestAppContext,
    ) {
        use std::time::{SystemTime, UNIX_EPOCH};
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let path = std::env::temp_dir().join(format!(
            "ecr-tab-icon-{}.svg",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, b"<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'><circle cx='12' cy='12' r='8'/></svg>").unwrap();
        let mut workspace = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            let view = cx.new(|cx| CommandWorkspace::new(w, cx));
            workspace = Some(view.clone());
            Root::new(view, w, cx)
        });
        let view = workspace.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            w.click(("sidebar-icon", 0usize), cx)
        })
        .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(250));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            w.click("icon-source", cx);
            w.input(path.to_str().unwrap(), cx);
            w.click("icon-source-apply", cx);
            w.render_frame(cx);
            assert_eq!(view.read(cx).tabs[0].icon_source.as_deref(), path.to_str());
            assert!(view.read(cx).tabs[0]
                .icon_svg
                .as_deref()
                .unwrap()
                .contains("<circle"));
            let data = view.read(cx).tabs[0].data(cx);
            let value = serde_json::to_value(&data).unwrap();
            assert_eq!(value["gpui_icon"], path.to_str().unwrap());
            let restored: crate::state::TabData = serde_json::from_value(value).unwrap();
            assert_eq!(restored.icon_svg, data.icon_svg);
            std::fs::write(&path, b"<svg><script>alert(1)</script></svg>").unwrap();
            view.update(cx, |v, cx| {
                v.set_tab_icon_from_source(0, path.to_string_lossy().into_owned(), cx)
            });
            assert_eq!(
                view.read(cx).tabs[0].icon_svg,
                data.icon_svg,
                "非法 SVG 不得替换先前选择"
            );
            w.remove_window();
        })
        .unwrap();
        std::fs::remove_file(&path).unwrap();
    }

    #[gpui::test]
    fn closing_without_tray_still_protects_unsaved_edits(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            Root::new(cx.new(|cx| CommandWorkspace::new(w, cx)), w, cx)
        });
        cx.update_window(handle.into(), |_, w, cx| {
            assert!(crate::tray::can_close_without_tray(w, cx));
            w.click("add-tab-title", cx);
            assert!(!crate::tray::can_close_without_tray(w, cx));
            w.render_frame(cx);
            assert!(w.has_active_dialog(cx));
            assert!(w.try_find("confirm-exit").is_some());
            w.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn clean_exit_removes_the_main_window_without_a_confirmation(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut view = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            let entity = cx.new(|cx| CommandWorkspace::new(w, cx));
            view = Some(entity.clone());
            Root::new(entity, w, cx)
        });
        let view = view.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            assert!(!view.read(cx).has_unsaved_edits());
            assert!(!view.read(cx).has_running_commands());
            assert!(view.update(cx, |v, cx| v.request_exit(w, cx)));
        })
        .unwrap();
        assert!(cx.update(|cx| cx.windows().is_empty()));
    }

    #[gpui::test]
    fn adding_blank_parameters_requires_exit_confirmation(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut view = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            let entity = cx.new(|cx| CommandWorkspace::new(w, cx));
            view = Some(entity.clone());
            Root::new(entity, w, cx)
        });
        let view = view.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            w.click("add-parameter", cx);
            w.click("add-parameter", cx);
            assert!(view.read(cx).tabs[0].dirty);
            assert!(view.read(cx).has_unsaved_edits_now(cx));
        })
        .unwrap();

        // Match the tray path: update the window without leasing Root, then
        // update the workspace; request_exit opens its modal through WindowExt.
        let window: gpui::AnyWindowHandle = handle.into();
        let stop_polling = cx.update(|cx| {
            window
                .update(cx, |_, w, cx| {
                    view.update(cx, |v, cx| v.request_exit(w, cx))
                })
                .unwrap()
        });
        assert!(!stop_polling, "unsaved rows must keep the tray loop alive");
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            assert!(w.has_active_dialog(cx));
            assert!(w.try_find("cancel-exit").is_some());
            assert!(w.try_find("confirm-exit").is_some());
            w.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn exit_with_unsaved_edits_requires_explicit_confirmation(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut view = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            let entity = cx.new(|cx| CommandWorkspace::new(w, cx));
            view = Some(entity.clone());
            Root::new(entity, w, cx)
        });
        let view = view.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            assert!(!view.read(cx).has_unsaved_edits());
            w.click("program", cx);
            w.press(
                if cfg!(target_os = "macos") {
                    "cmd-a"
                } else {
                    "ctrl-a"
                },
                cx,
            );
            w.input("unsaved-command", cx);
            assert_eq!(w.find("program").value(), Some("unsaved-command"));
            assert!(
                view.read(cx).has_unsaved_edits_now(cx),
                "退出判定必须检查最新输入值，不依赖延迟送达的 Change 订阅"
            );
            view.update(cx, |v, cx| v.request_exit(w, cx));
            w.render_frame(cx);
            assert!(w.try_find("cancel-exit").is_some());
            assert!(w.try_find("confirm-exit").is_some());
            assert!(w.has_active_dialog(cx));
            assert!(
                !view.update(cx, |v, cx| v.request_exit(w, cx)),
                "已有确认框时退出请求不能转成立即退出"
            ); // 连续点击托盘退出不能叠加对话框。
        })
        .unwrap();
        // GPUI Kit 的对话框入场动画在下一轮测试时钟后才完成，
        // 未完成时即使按钮已可观测到，其点击区域仍可能被遮罩层接管。
        cx.background_executor
            .advance_clock(std::time::Duration::from_millis(350));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| w.render_frame(cx))
            .unwrap();
        cx.background_executor
            .advance_clock(std::time::Duration::from_millis(350));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            w.click("cancel-exit", cx);
            assert!(!w.has_active_dialog(cx));
            view.update(cx, |v, cx| v.request_exit(w, cx));
            w.render_frame(cx);
            assert!(w.has_active_dialog(cx), "取消后应允许重新请求退出");
            w.press("escape", cx);
            assert!(!w.has_active_dialog(cx));
            assert_eq!(view.read(cx).tabs.len(), 1);
            w.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn closing_open_editor_keeps_draft_and_sidebar_can_reopen_it(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut view = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            let entity = cx.new(|cx| CommandWorkspace::new(w, cx));
            view = Some(entity.clone());
            Root::new(entity, w, cx)
        });
        let view = view.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            w.click("add-tab-title", cx);
            let tab_id = view.read(cx).tabs[1].id;
            view.update(cx, |v, cx| {
                v.tabs[1]
                    .program
                    .update(cx, |state, cx| state.set_value("draft-not-saved", w, cx))
            });
            w.click(("close-tab", tab_id), cx);
            assert_eq!(view.read(cx).tabs.len(), 2, "关闭编辑器不删除配置");
            assert_eq!(view.read(cx).open_tab_ids, vec![0]);
            assert!(
                w.try_find("cancel-close-tab").is_none(),
                "关闭视图不应丢弃草稿或弹出删除确认"
            );
            assert!(w.try_find(("sidebar-tab", tab_id)).is_some());
            w.click(("sidebar-tab", tab_id), cx);
            assert_eq!(view.read(cx).active, 1);
            assert_eq!(w.find("program").value(), Some("draft-not-saved"));
            w.click(("close-tab", tab_id), cx);
            w.click(("close-tab", 0usize), cx);
            assert!(view.read(cx).open_tab_ids.is_empty());
            assert!(w.try_find("empty-editor").is_some());
            w.click(("sidebar-tab", 0usize), cx);
            assert_eq!(view.read(cx).open_tab_ids, vec![0]);
            w.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn sidebar_delete_button_only_reveals_on_its_row_without_layout_shift(cx: &mut TestAppContext) {
        use gpui::Focusable;
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
        cx.update_window(handle.into(), |_, w, cx| {
            w.click("add-tab-title", cx);
            w.click("program", cx);
            let editor = view.read(cx).tabs[1].program.clone();
            let before = w.find(("sidebar-tab", 0usize)).bounds();
            let icon_before = w.find(("sidebar-icon", 0usize)).bounds();
            for id in 0..2usize {
                assert!(!w
                    .try_find(("sidebar-close-tab", id))
                    .is_some_and(|button| button.visible()));
            }
            w.hover(("sidebar-tab", 0usize), cx);
            assert!(w.find(("sidebar-close-tab", 0usize)).visible());
            assert!(!w
                .try_find(("sidebar-close-tab", 1usize))
                .is_some_and(|button| button.visible()));
            assert_eq!(w.find(("sidebar-tab", 0usize)).bounds(), before);
            assert_eq!(w.find(("sidebar-icon", 0usize)).bounds(), icon_before);
            assert!(editor.focus_handle(cx).is_focused(w));
            w.hover("program", cx);
            assert!(!w
                .try_find(("sidebar-close-tab", 0usize))
                .is_some_and(|button| button.visible()));
            assert_eq!(w.find(("sidebar-tab", 0usize)).bounds(), before);
            assert!(!w.has_active_dialog(cx));
            w.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn removing_sidebar_configuration_always_requires_confirmation(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut view = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            let entity = cx.new(|cx| CommandWorkspace::new(w, cx));
            view = Some(entity.clone());
            Root::new(entity, w, cx)
        });
        let view = view.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            w.click("add-tab-title", cx);
            w.hover(("sidebar-tab", 1usize), cx);
            w.click(("sidebar-close-tab", 1usize), cx);
            assert!(w.try_find("confirm-delete-tab").is_some());
            assert_eq!(view.read(cx).tabs.len(), 2);
            w.render_frame(cx);
        })
        .unwrap();
        cx.background_executor
            .advance_clock(std::time::Duration::from_millis(350));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| w.render_frame(cx))
            .unwrap();
        cx.background_executor
            .advance_clock(std::time::Duration::from_millis(350));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            w.click("cancel-delete-tab", cx);
            assert_eq!(view.read(cx).tabs.len(), 2);
            w.hover(("sidebar-tab", 1usize), cx);
            w.click(("sidebar-close-tab", 1usize), cx);
            w.render_frame(cx);
        })
        .unwrap();
        cx.background_executor
            .advance_clock(std::time::Duration::from_millis(700));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            w.click("confirm-delete-tab", cx);
            assert_eq!(view.read(cx).tabs.len(), 1);
            assert_eq!(view.read(cx).open_tab_ids, vec![0]);
            assert!(w.try_find(("sidebar-tab", 1usize)).is_none());
            w.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn sidebar_and_open_tab_strips_reorder_independently_by_drag(cx: &mut TestAppContext) {
        use gpui::point;
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut view = None;
        let handle = cx.open_window(size(px(1200.), px(900.)), |w, cx| {
            let entity = cx.new(|cx| CommandWorkspace::new(w, cx));
            view = Some(entity.clone());
            Root::new(entity, w, cx)
        });
        let view = view.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            for _ in 0..3 {
                w.click("add-tab-title", cx);
            }
            let input_id = view.read(cx).tabs[3].program.entity_id();
            let from = w.find(("sidebar-tab", 3usize)).bounds();
            let to = w.find(("sidebar-tab", 1usize)).bounds();
            w.drag(from.center(), point(to.center().x, to.top() + px(3.)), cx);
            assert_eq!(
                view.read(cx)
                    .tabs
                    .iter()
                    .map(|tab| tab.id)
                    .collect::<Vec<_>>(),
                vec![0, 3, 1, 2]
            );
            assert_eq!(view.read(cx).tabs[view.read(cx).active].id, 3);
            assert_eq!(view.read(cx).tabs[1].program.entity_id(), input_id);
            assert_eq!(
                view.read(cx).open_tab_ids,
                vec![0, 1, 2, 3],
                "侧栏排序不应偷改已打开标签顺序"
            );
            let from = w.find(("tab-context", 2usize)).bounds();
            let to = w.find(("tab-context", 0usize)).bounds();
            w.drag(from.center(), point(to.left() + px(3.), to.center().y), cx);
            assert_eq!(view.read(cx).open_tab_ids, vec![2, 0, 1, 3]);
            assert_eq!(
                view.read(cx)
                    .tabs
                    .iter()
                    .map(|tab| tab.id)
                    .collect::<Vec<_>>(),
                vec![0, 3, 1, 2],
                "上部排序不改配置列表顺序"
            );
            w.render_frame(cx);
            let from = w.find(("sidebar-tab", 0usize)).bounds();
            let to = w.find(("sidebar-tab", 1usize)).bounds();
            w.drag(
                from.center(),
                point(to.center().x, to.bottom() - px(3.)),
                cx,
            );
            assert_eq!(
                view.read(cx)
                    .tabs
                    .iter()
                    .map(|tab| tab.id)
                    .collect::<Vec<_>>(),
                vec![3, 1, 0, 2],
                "落在目标下半部应插到后面"
            );
            w.render_frame(cx);
            let from = w.find(("tab-context", 2usize)).bounds();
            let to = w.find(("tab-context", 1usize)).bounds();
            w.drag(from.center(), point(to.right() - px(3.), to.center().y), cx);
            assert_eq!(
                view.read(cx).open_tab_ids,
                vec![0, 1, 2, 3],
                "落在标签右半部应插到后面"
            );
            w.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn sidebar_second_config_can_move_before_first(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut view = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            let entity = cx.new(|cx| CommandWorkspace::new(w, cx));
            view = Some(entity.clone());
            Root::new(entity, w, cx)
        });
        let view = view.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            w.click("add-tab-title", cx);
            // Ordinary center-to-center dragging must work in both directions; the old
            // >= midpoint rule made the reverse adjacent move a no-op.
            w.drag_to(("sidebar-tab", 1usize), ("sidebar-tab", 0usize), cx);
            assert_eq!(
                view.read(cx).tabs.iter().map(|t| t.id).collect::<Vec<_>>(),
                vec![1, 0],
                "第二行应可拖到第一行之前"
            );
            assert_eq!(view.read(cx).tabs[view.read(cx).active].id, 1);
            w.drag_to(("sidebar-tab", 1usize), ("sidebar-tab", 0usize), cx);
            assert_eq!(
                view.read(cx).tabs.iter().map(|t| t.id).collect::<Vec<_>>(),
                vec![0, 1],
                "第一行拖向第二行中点也应交换"
            );
            w.drag_to(("tab-context", 1usize), ("tab-context", 0usize), cx);
            assert_eq!(
                view.read(cx).open_tab_ids,
                vec![1, 0],
                "顶部第二枚拖向第一枚中点也应交换"
            );
            w.render_frame(cx);
            w.drag_to(("tab-context", 1usize), ("tab-context", 0usize), cx);
            assert_eq!(
                view.read(cx).open_tab_ids,
                vec![0, 1],
                "顶部正反两方向应对称"
            );
            w.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn top_tabs_drop_on_close_side_without_closing_or_reordering_sidebar(cx: &mut TestAppContext) {
        use gpui::point;
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut view = None;
        let handle = cx.open_window(size(px(1200.), px(900.)), |w, cx| {
            let entity = cx.new(|cx| CommandWorkspace::new(w, cx));
            view = Some(entity.clone());
            Root::new(entity, w, cx)
        });
        let view = view.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            for _ in 0..2 {
                w.click("add-tab-title", cx);
            }
            let before = view
                .read(cx)
                .tabs
                .iter()
                .map(|tab| tab.id)
                .collect::<Vec<_>>();
            let from = w.find(("tab-context", 0usize)).bounds().center();
            let close = w.find(("close-tab", 2usize)).bounds();
            // The X side belongs to the whole pill as a drop target, but must not click X.
            w.drag(from, point(close.center().x, close.center().y), cx);
            assert_eq!(view.read(cx).open_tab_ids, vec![1, 2, 0]);
            assert_eq!(
                view.read(cx)
                    .tabs
                    .iter()
                    .map(|tab| tab.id)
                    .collect::<Vec<_>>(),
                before
            );
            assert_eq!(view.read(cx).tabs.len(), 3);
            // A click on X still closes the editor view, without deleting its configuration.
            w.click(("close-tab", 0usize), cx);
            assert_eq!(view.read(cx).open_tab_ids, vec![1, 2]);
            assert_eq!(view.read(cx).tabs.len(), 3);
            w.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn reorder_by_drag_handle_changes_preview_order(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut view = None;
        let handle = cx.open_window(size(px(850.), px(900.)), |w, cx| {
            let entity = cx.new(|cx| CommandWorkspace::new(w, cx));
            view = Some(entity.clone());
            Root::new(entity, w, cx)
        });
        let view = view.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            assert_eq!(
                view.read(cx).tabs[0]
                    .rows
                    .iter()
                    .map(|r| r.id)
                    .collect::<Vec<_>>(),
                vec![0, 1, 2]
            );
            w.drag_to(("move-row", 0usize), ("parameter-row", 2usize), cx);
            assert_eq!(
                view.read(cx).tabs[0]
                    .rows
                    .iter()
                    .map(|r| r.id)
                    .collect::<Vec<_>>(),
                vec![1, 2, 0]
            );
            assert!(view.read(cx).preview.read(cx).value().contains("-i"));
        })
        .unwrap();
    }

    #[gpui::test]
    fn row_drag_handles_gaps_groups_and_cancel(cx: &mut TestAppContext) {
        use gpui::point;
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut view = None;
        let handle = cx.open_window(size(px(900.), px(1000.)), |w, cx| {
            let entity = cx.new(|cx| super::fixture(6, w, cx));
            view = Some(entity.clone());
            Root::new(entity, w, cx)
        });
        let view = view.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            let from = w.find(("move-row", 0usize)).bounds().center();
            let target = w.find(("parameter-row", 2usize)).bounds();
            w.drag(
                from,
                point(target.left() + px(10.), target.top() - px(3.)),
                cx,
            );
            assert_eq!(
                view.read(cx).tabs[0]
                    .rows
                    .iter()
                    .map(|r| r.id)
                    .collect::<Vec<_>>(),
                vec![1, 0, 2, 3, 4, 5]
            );
            let from = w.find(("move-row", 5usize)).bounds().center();
            let first = w.find(("parameter-row", 1usize)).bounds();
            w.drag(
                from,
                point(first.left() + px(10.), first.top() + px(3.)),
                cx,
            );
            assert_eq!(
                view.read(cx).tabs[0]
                    .rows
                    .iter()
                    .map(|r| r.id)
                    .collect::<Vec<_>>(),
                vec![5, 1, 0, 2, 3, 4]
            );
            let from = w.find(("move-row", 5usize)).bounds().center();
            let outside = w.find("preview-header").bounds().center();
            w.drag(from, outside, cx);
            for id in 0usize..6 {
                assert!(w.try_find(("insert-indicator", id)).is_none());
            }
            assert_eq!(view.read(cx).tabs[0].rows[0].id, 5);
            let add = w.find("add-parameter").bounds();
            let enabled_first = w.find("enabled-first");
            assert_eq!(enabled_first.label(), Some("勾选参数置顶"));
            assert_eq!(enabled_first.checked(), Some(false));
            let priority = enabled_first.bounds();
            let all = w.find("select-all").bounds();
            let none = w.find("select-none").bounds();
            assert!(
                add.right() < priority.left()
                    && priority.right() < all.left()
                    && all.right() < none.left(),
                "添加/参数排序应在左侧，全选/全不选在右侧"
            );
            let before_sort = priority.size;
            w.click("enabled-first", cx);
            assert_eq!(w.find("enabled-first").bounds().size, before_sort);
            assert_eq!(w.find("enabled-first").checked(), Some(true));
            assert!(view.read(cx).enabled_first);
            assert_eq!(
                view.read(cx).tabs[0]
                    .rows
                    .iter()
                    .map(|r| r.enabled)
                    .collect::<Vec<_>>(),
                vec![true, true, true, true, false, false]
            );
            let before = view.read(cx).tabs[0]
                .rows
                .iter()
                .map(|r| r.id)
                .collect::<Vec<_>>();
            w.drag_to(("move-row", before[0]), ("parameter-row", before[4]), cx);
            assert_eq!(
                view.read(cx).tabs[0]
                    .rows
                    .iter()
                    .map(|r| r.id)
                    .collect::<Vec<_>>(),
                before,
                "勾选参数置顶时不能越过未勾选组"
            );
            let disabled = before[4];
            w.click(("toggle-row", disabled), cx);
            assert_eq!(view.read(cx).tabs[0].rows[4].id, disabled);
            assert!(view.read(cx).tabs[0].rows[4].enabled);
            assert!(view.read(cx).preview.read(cx).value().contains("-i"));
        })
        .unwrap();
    }

    #[gpui::test]
    fn parameter_priority_button_has_fixed_action_label_and_active_accent(cx: &mut TestAppContext) {
        cx.update(|cx| gpui::init(cx));
        for (mode, accent) in [
            (Theme::Dark, theme::Accent::Blue),
            (Theme::Light, theme::Accent::Teal),
        ] {
            for font_size in [12u8, 14, 16] {
                cx.update(|cx| {
                    cx.set_global(accent);
                    theme::apply(mode, cx);
                    theme::set_font_size(font_size, cx);
                });
                let stored = serde_json::json!({
                    "theme": if mode == Theme::Dark { "dark" } else { "light" },
                    "gpui": { "accent": if accent == theme::Accent::Blue { "blue" } else { "teal" }, "font_size": font_size }
                });
                let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
                    Root::new(
                        cx.new(|cx| {
                            CommandWorkspace::new_with_backend(w, cx, None, Some(stored), None)
                        }),
                        w,
                        cx,
                    )
                });
                let old_size = cx
                    .update_window(handle.into(), |_, w, cx| {
                        w.render_frame(cx);
                        let button = w.find("enabled-first");
                        assert_eq!(button.label(), Some("勾选参数置顶"));
                        assert_eq!(button.checked(), Some(false));
                        assert_eq!(button.bounds().size.width, px(font_size as f32 * 6. + 28.));
                        button.bounds().size
                    })
                    .unwrap();
                cx.update_window(handle.into(), |_, w, cx| {
                    w.click("enabled-first", cx);
                    w.render_frame(cx);
                    assert_eq!(w.find("enabled-first").label(), Some("勾选参数置顶"));
                    assert_eq!(w.find("enabled-first").checked(), Some(true));
                    assert_eq!(w.find("enabled-first").bounds().size, old_size);
                })
                .unwrap();
                cx.update_window(handle.into(), |_, w, _| w.remove_window())
                    .unwrap();
            }
        }
    }

    #[gpui::test]
    fn saving_reordered_configurations_keeps_independent_open_order(cx: &mut TestAppContext) {
        use crate::backend::ConfigStore;
        use gpui::point;
        use std::time::{SystemTime, UNIX_EPOCH};
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let dir = std::env::temp_dir().join(format!(
            "ecr-reordered-configs-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = ConfigStore::at(dir.join("config.json"), dir.join("backup"));
        store
            .save(&serde_json::json!({"tabs":[{"name":"a","program":"echo a"},
            {"name":"b","program":"echo b"},{"name":"c","program":"echo c"}]}))
            .unwrap();
        let mut view = None;
        let handle = cx.open_window(size(px(1200.), px(900.)), |w, cx| {
            let entity = cx.new(|cx| {
                CommandWorkspace::new_with_backend(
                    w,
                    cx,
                    Some(store.clone()),
                    store.load().unwrap(),
                    None,
                )
            });
            view = Some(entity.clone());
            Root::new(entity, w, cx)
        });
        let view = view.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            let from = w.find(("sidebar-tab", 2usize)).bounds();
            let to = w.find(("sidebar-tab", 0usize)).bounds();
            w.drag(from.center(), point(to.center().x, to.top() + px(3.)), cx);
            assert_eq!(
                view.read(cx)
                    .tabs
                    .iter()
                    .map(|t| t.label(cx))
                    .collect::<Vec<_>>(),
                vec!["c", "a", "b"]
            );
            let from = w.find(("tab-context", 1usize)).bounds();
            let to = w.find(("tab-context", 0usize)).bounds();
            w.drag(from.center(), point(to.left() + px(3.), to.center().y), cx);
            assert_eq!(view.read(cx).open_tab_ids, vec![1, 0, 2]);
            w.click(("sidebar-tab", 0usize), cx);
            // Before explicit save, the session still refers to original disk positions.
            assert_eq!(store.load_tab_session(), Some((vec![1, 0, 2], 0)));
            w.click("save", cx);
            assert!(!view.read(cx).has_unsaved_edits());
            w.remove_window();
        })
        .unwrap();
        let saved = store.load().unwrap().unwrap();
        assert_eq!(saved["tabs"][0]["name"], "c");
        assert_eq!(saved["gpui"]["open_tabs"], serde_json::json!([2, 1, 0]));
        assert_eq!(store.load_tab_session(), Some((vec![2, 1, 0], 1)));
        let mut restored = None;
        let handle = cx.open_window(size(px(1200.), px(900.)), |w, cx| {
            let entity = cx.new(|cx| {
                CommandWorkspace::new_with_backend(
                    w,
                    cx,
                    Some(store.clone()),
                    store.load().unwrap(),
                    None,
                )
            });
            restored = Some(entity.clone());
            Root::new(entity, w, cx)
        });
        cx.update_window(handle.into(), |_, w, cx| {
            let view = restored.as_ref().unwrap().read(cx);
            assert_eq!(
                view.tabs.iter().map(|t| t.label(cx)).collect::<Vec<_>>(),
                vec!["c", "a", "b"]
            );
            assert_eq!(view.open_tab_ids, vec![2, 1, 0]);
            assert_eq!(view.tabs[view.active].label(cx), "a");
            w.remove_window();
        })
        .unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[gpui::test]
    fn open_editors_are_remembered_without_saving_draft_commands(cx: &mut TestAppContext) {
        use crate::backend::ConfigStore;
        use std::time::{SystemTime, UNIX_EPOCH};
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let dir = std::env::temp_dir().join(format!(
            "ecr-open-editors-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = ConfigStore::at(dir.join("config.json"), dir.join("backup"));
        store
            .save(&serde_json::json!({"tabs":[
            {"name":"one","program":"echo one"},
            {"name":"two","program":"echo two"},
            {"name":"three","program":"echo three"}
        ],"current_tab_index":2}))
            .unwrap();
        let original = std::fs::read(store.config_path()).unwrap();
        let mut view = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            let entity = cx.new(|cx| {
                CommandWorkspace::new_with_backend(
                    w,
                    cx,
                    Some(store.clone()),
                    store.load().unwrap(),
                    None,
                )
            });
            view = Some(entity.clone());
            Root::new(entity, w, cx)
        });
        let view = view.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            w.click(("close-tab", 0usize), cx);
            w.click(("close-tab", 2usize), cx);
            assert_eq!(view.read(cx).open_tab_ids, vec![1]);
            assert_eq!(view.read(cx).tabs.len(), 3);
            assert!(
                !view.read(cx).has_unsaved_edits(),
                "关闭视图不应标为命令未保存"
            );
            assert_eq!(store.load_tab_session(), Some((vec![1], 1)));
            assert_eq!(
                std::fs::read(store.config_path()).unwrap(),
                original,
                "记忆视图不能顺带写入未保存的命令"
            );
            assert!(!dir.join("backup").exists(), "记忆视图不应制造配置备份");
            w.remove_window();
        })
        .unwrap();
        let mut restored = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            let entity = cx.new(|cx| {
                CommandWorkspace::new_with_backend(
                    w,
                    cx,
                    Some(store.clone()),
                    store.load().unwrap(),
                    None,
                )
            });
            restored = Some(entity.clone());
            Root::new(entity, w, cx)
        });
        let restored = restored.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            assert_eq!(restored.read(cx).open_tab_ids, vec![1]);
            assert_eq!(restored.read(cx).active, 1);
            assert!(w.try_find(("sidebar-tab", 0usize)).is_some());
            assert!(w.try_find(("tab-context", 0usize)).is_none());
            restored.update(cx, |view, cx| {
                view.tabs[1]
                    .program
                    .update(cx, |state, cx| state.set_value("unsaved", w, cx))
            });
            w.click(("close-tab", 1usize), cx);
            assert!(restored.read(cx).open_tab_ids.is_empty());
            assert!(w.try_find("empty-editor").is_some());
            assert_eq!(store.load_tab_session(), Some((vec![], 1)));
            w.remove_window();
        })
        .unwrap();
        let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            let entity = cx.new(|cx| {
                CommandWorkspace::new_with_backend(
                    w,
                    cx,
                    Some(store.clone()),
                    store.load().unwrap(),
                    None,
                )
            });
            Root::new(entity, w, cx)
        });
        cx.update_window(handle.into(), |_, w, cx| {
            assert!(w.try_find("empty-editor").is_some());
            w.click(("sidebar-tab", 0usize), cx);
            assert_eq!(w.find("program").value(), Some("echo one"));
            w.remove_window();
        })
        .unwrap();
        std::fs::write(
            store.config_path(),
            b"{\"tabs\":[{\"name\":\"replacement\"}]}",
        )
        .unwrap();
        assert!(
            store.load_tab_session().is_none(),
            "配置被外部改写后不可使用旧索引"
        );
        std::fs::write(
            store.config_path(),
            serde_json::to_vec(&serde_json::json!({
                "tabs":[{"name":"new one"},{"name":"new two"},{"name":"new three"}],
                "gpui":{"open_tabs":[2]}
            }))
            .unwrap(),
        )
        .unwrap();
        let mut restored = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            let entity = cx.new(|cx| {
                CommandWorkspace::new_with_backend(
                    w,
                    cx,
                    Some(store.clone()),
                    store.load().unwrap(),
                    None,
                )
            });
            restored = Some(entity.clone());
            Root::new(entity, w, cx)
        });
        cx.update_window(handle.into(), |_, w, cx| {
            let view = restored.as_ref().unwrap().read(cx);
            assert_eq!(
                view.open_tab_ids,
                vec![0, 1, 2],
                "旧会话失效时不可回退到配置里可能过时的打开索引"
            );
            assert_eq!(view.active, 0);
            w.remove_window();
        })
        .unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[gpui::test]
    fn backend_restores_last_tab_and_saves_qt_compatible_configuration(cx: &mut TestAppContext) {
        use crate::backend::ConfigStore;
        use std::time::{SystemTime, UNIX_EPOCH};
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let dir = std::env::temp_dir().join(format!(
            "ecr-gpui-ui-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::at(dir.join("config.json"), dir.join("backup"));
        store.save(&serde_json::json!({
            "theme":"light", "current_tab_index":1,
            "tabs":[
                {"name":"第一条","program":"echo first","working_dir":"","other_args":"","description":"one",
                 "functions":[{"function":"-n","parameter":"first","comment":"first row","enabled":true}],
                 "gpui_row_height":3},
                {"name":"最后打开的页面","program":"echo second","working_dir":"","other_args":"","description":"restored",
                 "functions":[{"function":"-n","parameter":"second","comment":"second row","enabled":true}],
                 "gpui_row_height":3}
            ],
            "gpui":{"accent":"teal","font_size":16,"font_weight":500,"reduced_motion":true,
                    "enabled_first":true,"show_log":true,"log_height":100,"description_width":280}
        })).unwrap();

        let mut workspace = None;
        let handle = cx.open_window(size(px(1200.), px(1000.)), |w, cx| {
            let config = store.load().unwrap();
            let view = cx.new(|cx| {
                CommandWorkspace::new_with_backend(w, cx, Some(store.clone()), config, None)
            });
            workspace = Some(view.clone());
            Root::new(view, w, cx)
        });
        let workspace = workspace.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            let view = workspace.read(cx);
            assert_eq!(view.active, 1);
            assert_eq!(view.tabs[1].label(cx), "最后打开的页面");
            assert_eq!(view.tabs[1].row_height_override, Some(3));
            assert!(view.enabled_first && view.show_log && view.reduced_motion);
            assert!(!view.has_unsaved_edits());
            assert_eq!(*cx.global::<Theme>(), Theme::Light);
            assert_eq!(*cx.global::<theme::Accent>(), theme::Accent::Teal);
            assert_eq!(view.font_size, 16);
            assert_eq!(view.font_weight, 500);
            assert_eq!(w.find("description-frame").bounds().size.width, px(280.));
            assert!(w.try_find("log-dock").is_some());

            workspace.update(cx, |view, cx| {
                let id = view.tabs[1].id;
                view.tabs[1]
                    .program
                    .update(cx, |state, cx| state.set_value("printf saved", w, cx));
                view.changed(id, w, cx);
            });
            w.click("save", cx);
            assert!(!workspace.read(cx).has_unsaved_edits());
            assert!(workspace.read(cx).status.contains("配置已保存"));
            w.remove_window();
        })
        .unwrap();

        let saved = store.load().unwrap().unwrap();
        assert_eq!(saved["current_tab_index"], 1);
        assert_eq!(saved["tabs"][1]["program"], "printf saved");
        assert_eq!(saved["tabs"][1]["functions"][0]["function"], "-n");
        assert_eq!(saved["tabs"][1]["gpui_row_height"], 3);
        assert_eq!(saved["gpui"]["description_width"], 280.0);
        assert!(std::fs::read_dir(dir.join("backup")).unwrap().count() == 1);

        let mut restored = None;
        let handle = cx.open_window(size(px(1200.), px(1000.)), |w, cx| {
            let config = store.load().unwrap();
            let view = cx.new(|cx| {
                CommandWorkspace::new_with_backend(w, cx, Some(store.clone()), config, None)
            });
            restored = Some(view.clone());
            Root::new(view, w, cx)
        });
        let restored = restored.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            assert_eq!(restored.read(cx).active, 1);
            assert_eq!(
                restored.read(cx).tabs[1].program.read(cx).value(),
                "printf saved"
            );
            assert_eq!(w.find("description-frame").bounds().size.width, px(280.));
            restored.update(cx, |v, cx| {
                let id = v.tabs[1].id;
                v.tabs[1]
                    .program
                    .update(cx, |s, cx| s.set_value("unsaved command", w, cx));
                v.changed(id, w, cx);
                v.apply_preferences(
                    crate::settings::Preferences {
                        theme: Theme::Dark,
                        accent: theme::Accent::Violet,
                        font_size: 24,
                        font_weight: 800,
                        language: crate::i18n::Language::English,
                        reduced_motion: true,
                    },
                    w,
                    cx,
                )
                .unwrap();
                assert!(
                    v.has_unsaved_edits(),
                    "applying settings must not mark command edits saved"
                );
            });
            w.remove_window();
        })
        .unwrap();
        let saved = store.load().unwrap().unwrap();
        assert_eq!(saved["tabs"][1]["program"], "printf saved");
        let handle = cx.open_window(size(px(1200.), px(1000.)), |w, cx| {
            let v = cx.new(|cx| {
                CommandWorkspace::new_with_backend(
                    w,
                    cx,
                    Some(store.clone()),
                    Some(saved.clone()),
                    None,
                )
            });
            assert_eq!(v.read(cx).font_size, 24);
            assert_eq!(v.read(cx).font_weight, 800);
            assert_eq!(
                *cx.global::<crate::i18n::Language>(),
                crate::i18n::Language::English
            );
            assert_eq!(*cx.global::<Theme>(), Theme::Dark);
            assert_eq!(*cx.global::<theme::Accent>(), theme::Accent::Violet);
            Root::new(v, w, cx)
        });
        cx.update_window(handle.into(), |_, w, _| w.remove_window())
            .unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[gpui::test]
    fn backup_restore_asks_before_replacing_the_workspace(cx: &mut TestAppContext) {
        use crate::backend::ConfigStore;
        use std::time::{SystemTime, UNIX_EPOCH};
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let dir = std::env::temp_dir().join(format!(
            "ecr-gpui-restore-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::at(dir.join("config.json"), dir.join("backup"));
        store.save(&serde_json::json!({"tabs":[{"name":"旧备份","program":"echo old","functions":[]}]})).unwrap();
        store.save(&serde_json::json!({"tabs":[{"name":"当前配置","program":"echo current","functions":[]}]})).unwrap();
        let mut workspace = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            let config = store.load().unwrap();
            let view = cx.new(|cx| {
                CommandWorkspace::new_with_backend(w, cx, Some(store.clone()), config, None)
            });
            workspace = Some(view.clone());
            Root::new(view, w, cx)
        });
        let workspace = workspace.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            assert_eq!(workspace.read(cx).tabs[0].label(cx), "当前配置");
            w.click("application-menu", cx);
            w.click("恢复配置备份", cx);
            assert!(w.try_find(("backup-choice", 0usize)).is_some());
        })
        .unwrap();
        cx.run_until_parked();
        // Kit dialog geometry is animated with wall-clock time, not the test executor clock.
        // Wait for the entrance before choosing a row so its hitbox does not move mid-click.
        std::thread::sleep(std::time::Duration::from_millis(300));
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            assert!(w.try_find(("backup-choice", 0usize)).is_some());
            w.click_at(("backup-choice", 0usize), gpui::point(px(24.), px(16.)), cx);
            assert_eq!(
                workspace.read(cx).tabs[0].label(cx),
                "当前配置",
                "selecting a backup must not restore it"
            );
        })
        .unwrap();
        cx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(300));
        cx.update_window(handle.into(), |_, w, cx| {
            w.click("confirm-restore-backup", cx);
            assert_eq!(
                workspace.read(cx).tabs[0].label(cx),
                "旧备份",
                "backup choice did not restore; status={}",
                workspace.read(cx).status
            );
            assert!(workspace.read(cx).status.starts_with("已恢复备份："));
            w.remove_window();
        })
        .unwrap();
        assert_eq!(
            store.load().unwrap().unwrap()["tabs"][0]["program"],
            "echo old"
        );
        assert!(
            store.backups().unwrap().len() >= 2,
            "restore backs up the replaced config"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[gpui::test]
    fn corrupt_backend_configuration_is_never_overwritten(cx: &mut TestAppContext) {
        use crate::backend::ConfigStore;
        use std::time::{SystemTime, UNIX_EPOCH};
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let dir = std::env::temp_dir().join(format!(
            "ecr-gpui-broken-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, "broken json").unwrap();
        let store = ConfigStore::at(&path, dir.join("backup"));
        let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            Root::new(
                cx.new(|cx| {
                    CommandWorkspace::new_with_backend(
                        w,
                        cx,
                        Some(store.clone()),
                        None,
                        Some("读取失败，已禁止覆盖".into()),
                    )
                }),
                w,
                cx,
            )
        });
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            w.click("save", cx);
            w.render_frame(cx);
            w.remove_window();
        })
        .unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "broken json");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[gpui::test]
    fn run_button_streams_real_shell_output_and_stop_control(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut view = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            let entity = cx.new(|cx| CommandWorkspace::new(w, cx));
            view = Some(entity.clone());
            Root::new(entity, w, cx)
        });
        let view = view.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            view.update(cx, |v, cx| {
                let tab_id = {
                    let tab = &mut v.tabs[v.active];
                    tab.program.update(cx, |state, cx| {
                        state.set_value(
                            "printf 'stream-out\\n'; printf 'stream-error\\n' >&2; exit 7",
                            w,
                            cx,
                        )
                    });
                    tab.other.update(cx, |state, cx| state.set_value("", w, cx));
                    for row in &mut tab.rows {
                        row.enabled = false;
                    }
                    tab.id
                };
                v.changed(tab_id, w, cx);
            });
            w.click("run-command", cx);
            assert!(view.read(cx).show_log);
            assert!(w.try_find("stop-log").is_some());
        })
        .unwrap();
        let mut completed = false;
        for _ in 0..100 {
            std::thread::sleep(std::time::Duration::from_millis(20));
            cx.background_executor
                .advance_clock(std::time::Duration::from_millis(24));
            cx.run_until_parked();
            cx.update_window(handle.into(), |_, w, cx| {
                w.render_frame(cx);
                completed = view.read(cx).status.contains("退出代码：7");
            })
            .unwrap();
            if completed {
                break;
            }
        }
        assert!(
            completed,
            "shell process should finish and report exit status"
        );
        cx.update_window(handle.into(), |_, w, cx| {
            let output = view.read(cx).log_output.read(cx).value().to_string();
            assert!(
                output.contains("stream-out"),
                "stdout is streamed into selected run log: {output}"
            );
            assert!(
                output.contains("stream-error"),
                "stderr is streamed into selected run log: {output}"
            );
            assert!(
                w.try_find("stop-log").is_none(),
                "completed runs no longer show Stop"
            );
            view.update(cx, |v, cx| {
                let tab_id = v.tabs[v.active].id;
                v.tabs[v.active].program.update(cx, |state, cx| {
                    state.set_value("while :; do sleep 0.05; done", w, cx)
                });
                v.changed(tab_id, w, cx);
            });
            w.click("run-command", cx); // Start a long command and stop it from the log dock.
            w.render_frame(cx);
            assert!(w.try_find("stop-log").is_some());
            w.click("stop-log", cx);
        })
        .unwrap();
        let mut stopped = false;
        for _ in 0..100 {
            std::thread::sleep(std::time::Duration::from_millis(20));
            cx.background_executor
                .advance_clock(std::time::Duration::from_millis(24));
            cx.run_until_parked();
            stopped = cx
                .update_window(handle.into(), |_, w, cx| {
                    w.render_frame(cx);
                    view.read(cx).status == "命令已停止"
                })
                .unwrap();
            if stopped {
                break;
            }
        }
        assert!(
            stopped,
            "the stop action should kill the shell process group"
        );
        cx.update_window(handle.into(), |_, w, _| w.remove_window())
            .unwrap();
    }

    #[gpui::test]
    fn detached_log_shares_selection_output_zoom_and_reattaches_on_close(cx: &mut TestAppContext) {
        use gpui::Focusable;
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut workspace = None;
        let main = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            let view = cx.new(|cx| CommandWorkspace::new(w, cx));
            workspace = Some(view.clone());
            Root::new(view, w, cx)
        });
        let view = workspace.unwrap();
        let (first, second) = cx
            .update_window(main.into(), |_, w, cx| {
                let ids = view.update(cx, |v, cx| {
                    v.show_log = true;
                    v.log_height_override = Some(180.);
                    let a = v.add_finished_log_for_test("第一次", "旧输出", w, cx);
                    let b = v.add_finished_log_for_test("第二次", "新输出", w, cx);
                    (a, b)
                });
                w.render_frame(cx);
                w.click("detach-log", cx);
                ids
            })
            .unwrap();
        cx.run_until_parked();
        let detached = cx
            .update(|cx| view.read(cx).detached_log)
            .expect("成功打开第二个窗口");
        assert_eq!(cx.windows().len(), 2, "不能重复创建日志窗口");
        cx.update_window(main.into(), |_, w, cx| {
            w.render_frame(cx);
            assert!(w.try_find("log-dock").is_none());
            w.click("toggle-log", cx); // Already detached: focus the same window.
        })
        .unwrap();
        cx.run_until_parked();
        assert_eq!(cx.windows().len(), 2);
        cx.update_window(detached.into(), |_, w, cx| {
            w.render_frame(cx);
            assert!(w.find("detached-log-output").bounds().size.height > px(100.));
            assert_eq!(view.read(cx).selected_log_id, Some(second));
            w.click("detached-log-selector", cx);
            assert!(w.try_find(("log-menu-content", 0usize)).is_some());
        })
        .unwrap();
        cx.background_executor
            .advance_clock(std::time::Duration::from_millis(250));
        cx.run_until_parked();
        cx.update_window(detached.into(), |_, w, cx| {
            w.render_frame(cx);
            w.click(("log-menu-content", 0usize), cx);
        })
        .unwrap();
        cx.run_until_parked();
        assert_eq!(cx.update(|cx| view.read(cx).selected_log_id), Some(first));
        cx.update_window(detached.into(), |root, w, cx| {
            w.render_frame(cx);
            let pane = root
                .downcast::<Root>()
                .unwrap()
                .read(cx)
                .view()
                .clone()
                .downcast::<crate::log_window::DetachedLogWindow>()
                .unwrap();
            assert_eq!(pane.read(cx).output.read(cx).value().as_ref(), "旧输出");
            let focus = pane.read(cx).output.focus_handle(cx);
            w.focus(&focus, cx);
            w.press("ctrl-=", cx);
            assert_eq!(view.read(cx).log_font_size, 15);
            w.click("detached-delete-log", cx);
            assert_eq!(view.read(cx).selected_log_id, Some(second));
            assert_eq!(view.read(cx).log_snapshot().items.len(), 1);
            w.click("reattach-log", cx);
        })
        .unwrap();
        cx.run_until_parked();
        assert!(cx.update(|cx| view.read(cx).detached_log.is_none()));
        assert!(cx.update(|cx| view.read(cx).show_log));
        assert_eq!(cx.windows().len(), 1);
        cx.update_window(main.into(), |_, w, cx| {
            w.render_frame(cx);
            assert!(w.try_find("log-dock").is_some());
            assert_eq!(view.read(cx).log_font_size, 15);
            w.click("detach-log", cx);
        })
        .unwrap();
        cx.run_until_parked();
        let detached = cx
            .update(|cx| view.read(cx).detached_log)
            .expect("再次打开");
        cx.update_window(detached.into(), |_, w, _| w.remove_window())
            .unwrap();
        cx.run_until_parked();
        assert!(
            cx.update(|cx| view.read(cx).detached_log.is_none()),
            "系统关闭须重新停靠"
        );
        assert_eq!(cx.windows().len(), 1);
        cx.update_window(main.into(), |_, w, cx| {
            w.render_frame(cx);
            assert!(w.try_find("log-dock").is_some());
            w.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn reloading_configuration_keeps_detached_window_bound_to_current_session(
        cx: &mut TestAppContext,
    ) {
        use crate::backend::ConfigStore;
        use std::time::{SystemTime, UNIX_EPOCH};
        let dir = std::env::temp_dir().join(format!(
            "ecr-detached-reload-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::at(dir.join("config.json"), dir.join("backup"));
        store
            .save(&serde_json::json!({"tabs":[{"name":"saved","program":"echo saved"}]}))
            .unwrap();
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut workspace = None;
        let main = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            let view = cx.new(|cx| {
                CommandWorkspace::new_with_backend(
                    w,
                    cx,
                    Some(store.clone()),
                    store.load().unwrap(),
                    None,
                )
            });
            workspace = Some(view.clone());
            Root::new(view, w, cx)
        });
        let view = workspace.unwrap();
        cx.update_window(main.into(), |_, w, cx| {
            view.update(cx, |v, cx| {
                v.show_log = true;
                v.add_finished_log_for_test("session", "temporary", w, cx);
            });
            w.click("detach-log", cx);
        })
        .unwrap();
        cx.run_until_parked();
        let detached = cx.update(|cx| view.read(cx).detached_log).unwrap();
        cx.update_window(main.into(), |_, w, cx| w.click("reload", cx))
            .unwrap();
        cx.run_until_parked();
        assert_eq!(
            cx.update(|cx| view.read(cx).detached_log.map(|h| h.window_id())),
            Some(detached.window_id())
        );
        assert_eq!(
            cx.update(|cx| view.read(cx).log_snapshot().items.len()),
            0,
            "重载清除旧会话记录"
        );
        cx.update_window(detached.into(), |root, w, cx| {
            w.render_frame(cx);
            let pane = root
                .downcast::<Root>()
                .unwrap()
                .read(cx)
                .view()
                .clone()
                .downcast::<crate::log_window::DetachedLogWindow>()
                .unwrap();
            assert!(pane.read(cx).output.read(cx).value().is_empty());
            w.click("reattach-log", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(main.into(), |_, w, cx| {
            w.render_frame(cx);
            assert!(w.try_find("log-dock").is_some());
            w.remove_window();
        })
        .unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[gpui::test]
    fn detached_window_receives_live_output_and_closing_it_does_not_stop_the_run(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut workspace = None;
        let main = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            let view = cx.new(|cx| CommandWorkspace::new(w, cx));
            workspace = Some(view.clone());
            Root::new(view, w, cx)
        });
        let view = workspace.unwrap();
        cx.update_window(main.into(), |_, w, cx| {
            view.update(cx, |v, cx| {
                let tab = &mut v.tabs[v.active];
                let id = tab.id;
                tab.program.update(cx, |s, cx| {
                    s.set_value(
                        "printf 'detached-start\\n'; while :; do sleep 0.05; done",
                        w,
                        cx,
                    )
                });
                tab.other.update(cx, |s, cx| s.set_value("", w, cx));
                for row in &mut tab.rows {
                    row.enabled = false;
                }
                v.changed(id, w, cx);
            });
            w.click("run-command", cx);
            w.click("detach-log", cx);
        })
        .unwrap();
        cx.run_until_parked();
        let detached = cx.update(|cx| view.read(cx).detached_log).unwrap();
        let mut streamed = false;
        for _ in 0..80 {
            std::thread::sleep(std::time::Duration::from_millis(20));
            cx.background_executor
                .advance_clock(std::time::Duration::from_millis(25));
            cx.run_until_parked();
            streamed = cx
                .update_window(detached.into(), |root, w, cx| {
                    w.render_frame(cx);
                    let pane = root
                        .downcast::<Root>()
                        .unwrap()
                        .read(cx)
                        .view()
                        .clone()
                        .downcast::<crate::log_window::DetachedLogWindow>()
                        .unwrap();
                    pane.read(cx)
                        .output
                        .read(cx)
                        .value()
                        .contains("detached-start")
                })
                .unwrap();
            if streamed {
                break;
            }
        }
        assert!(streamed, "独立窗口应收到实时输出");
        cx.update_window(detached.into(), |_, w, _| {
            assert!(w.try_find("detached-stop-log").is_some());
            w.remove_window(); // Native X does not stop the worker.
        })
        .unwrap();
        cx.run_until_parked();
        assert!(cx.update(|cx| view.read(cx).has_running_commands()));
        cx.update_window(main.into(), |_, w, cx| {
            w.render_frame(cx);
            assert!(w.try_find("log-dock").is_some());
            assert!(w.try_find("stop-log").is_some());
            w.click("detach-log", cx);
        })
        .unwrap();
        cx.run_until_parked();
        let detached = cx.update(|cx| view.read(cx).detached_log).unwrap();
        cx.update_window(detached.into(), |_, w, cx| w.click("detached-stop-log", cx))
            .unwrap();
        let mut stopped = false;
        for _ in 0..80 {
            std::thread::sleep(std::time::Duration::from_millis(20));
            cx.background_executor
                .advance_clock(std::time::Duration::from_millis(25));
            cx.run_until_parked();
            stopped = cx.update(|cx| !view.read(cx).has_running_commands());
            if stopped {
                break;
            }
        }
        assert!(stopped, "独立窗口停止按钮须结束同一个进程组");
        cx.update_window(detached.into(), |_, w, cx| w.click("reattach-log", cx))
            .unwrap();
        cx.run_until_parked();
        cx.update_window(main.into(), |_, w, cx| {
            assert!(view
                .read(cx)
                .log_output
                .read(cx)
                .value()
                .contains("detached-start"));
            w.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn log_history_selects_and_deletes_only_real_records(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut view = None;
        let handle = cx.open_window(size(px(900.), px(1000.)), |w, cx| {
            let entity = cx.new(|cx| super::fixture(3, w, cx));
            view = Some(entity.clone());
            Root::new(entity, w, cx)
        });
        let view = view.unwrap();
        let mut ids = None;
        cx.update_window(handle.into(), |_, w, cx| {
            view.update(cx, |v, cx| {
                v.show_log = true;
                v.log_height_override = Some(180.);
                cx.notify();
            });
            w.render_frame(cx);
            assert!(w.try_find("log-selector").is_some());
            assert!(w.try_find("delete-log").is_some());
            w.click("delete-log", cx); // 空日志禁止操作，不能伪造一次运行。
            assert_eq!(view.read(cx).selected_log_id, None);
            let (first, second) = view.update(cx, |v, cx| {
                let first =
                    v.add_finished_log_for_test("00:01 转换  [退出 0]", "第一条输出", w, cx);
                let second =
                    v.add_finished_log_for_test("00:02 转换  [退出 1]", "第二条输出", w, cx);
                (first, second)
            });
            ids = Some((first, second));
            assert_eq!(view.read(cx).selected_log_id, Some(second));
            assert_eq!(
                view.read(cx).log_output.read(cx).value().as_ref(),
                "第二条输出"
            );
            w.click("log-selector", cx);
            let first_row = w.find(("log-menu-content", 0usize)).bounds();
            let second_row = w.find(("log-menu-content", 1usize)).bounds();
            assert_eq!(first_row.size.height, px(24.));
            assert_eq!(second_row.top() - first_row.top(), px(32.));
            assert!(first_row.top() >= w.find("log-selector").bounds().bottom());
        })
        .unwrap();
        let (first, second) = ids.unwrap();
        cx.background_executor
            .advance_clock(std::time::Duration::from_millis(200));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            w.click(("log-menu-content", 0usize), cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            assert_eq!(view.read(cx).selected_log_id, Some(first));
            assert_eq!(
                view.read(cx).log_output.read(cx).value().as_ref(),
                "第一条输出"
            );
            w.click("delete-log", cx);
            assert_eq!(view.read(cx).selected_log_id, Some(second));
            assert_eq!(
                view.read(cx).log_output.read(cx).value().as_ref(),
                "第二条输出"
            );
            w.click("delete-log", cx);
            assert_eq!(view.read(cx).selected_log_id, None);
            assert!(view.read(cx).log_output.read(cx).value().is_empty());
            w.click("log-selector", cx);
            assert!(w.try_find(("log-menu-content", 0usize)).is_none());
        })
        .unwrap();
    }

    #[gpui::test]
    fn panel_resize_obeys_three_to_nine_rows_and_preserves_preview(cx: &mut TestAppContext) {
        use crate::tokens::{parameter_height, PREVIEW_MIN};
        use gpui::point;
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let handle = cx.open_window(size(px(900.), px(1000.)), |w, cx| {
            Root::new(cx.new(|cx| super::fixture(12, w, cx)), w, cx)
        });
        cx.update_window(handle.into(), |_, w, cx| {
            let from = w.find("parameter-resize").bounds().center();
            w.drag(from, point(from.x, from.y + px(200.)), cx);
            assert_eq!(
                w.find("parameter-list").bounds().size.height,
                px(parameter_height(9))
            );
            let from = w.find("parameter-resize").bounds().center();
            w.drag(from, point(from.x, from.y - px(400.)), cx);
            assert_eq!(
                w.find("parameter-list").bounds().size.height,
                px(parameter_height(3))
            );
            assert!(w.try_find("preview-resize").is_none());
            assert!(w.find("preview-frame").bounds().size.height >= px(PREVIEW_MIN));
            assert_eq!(
                w.find("description-frame").bounds().size.height,
                w.find("preview-frame").bounds().size.height
            );
            let before = w.find("description-frame").bounds().size.width;
            let grip = w.find("preview-splitter").bounds().center();
            w.drag(grip, point(grip.x + px(80.), grip.y), cx);
            assert!(w.find("description-frame").bounds().size.width < before);
            assert!(w.find("preview-frame").bounds().size.width >= px(PREVIEW_MIN));
            let grip = w.find("preview-splitter").bounds().center();
            w.drag(grip, point(grip.x - px(900.), grip.y), cx);
            assert!(w.find("preview-frame").bounds().size.width >= px(PREVIEW_MIN));
            let grip = w.find("preview-splitter").bounds().center();
            w.drag(grip, point(grip.x + px(900.), grip.y), cx);
            assert!(w.find("description-frame").bounds().size.width >= px(120.));
            assert!(
                w.find("run-actions").bounds().bottom()
                    <= w.find("command-editor").bounds().bottom()
            );
            w.click("toggle-log", cx);
            let from = w.find("log-resize").bounds().center();
            let before = w.find("log-dock").bounds().size.height;
            w.drag(from, point(from.x, from.y - px(80.)), cx);
            assert!(w.find("log-dock").bounds().size.height > before);
            assert!(w.find("preview-frame").bounds().size.height >= px(PREVIEW_MIN));
            let list = w.find("parameter-list").bounds();
            let row_grip = w.find("parameter-resize").bounds();
            assert_eq!(
                row_grip.center().y,
                list.bottom(),
                "参数拖柄不应盖住最后一行"
            );
            assert!(w.find("parameter-actions").bounds().top() - row_grip.center().y >= px(8.));
            let dock = w.find("log-dock").bounds();
            let log_grip = w.find("log-resize").bounds();
            assert!(
                log_grip.center().y - dock.top() >= px(6.),
                "日志拖柄不贴顶边框"
            );
            assert!(
                w.find("log-select-frame").bounds().top() - log_grip.center().y >= px(4.),
                "日志拖柄与控件留白"
            );
        })
        .unwrap();
    }

    #[gpui::test]
    fn splitters_remember_global_sizes_and_independent_tab_row_heights(cx: &mut TestAppContext) {
        use crate::tokens::{parameter_height, PREVIEW_MIN};
        use gpui::point;
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut view = None;
        let handle = cx.open_window(size(px(1200.), px(1200.)), |w, cx| {
            let entity = cx.new(|cx| super::fixture(12, w, cx));
            view = Some(entity.clone());
            Root::new(entity, w, cx)
        });
        let view = view.unwrap();
        let (description_width, log_height) = cx
            .update_window(handle.into(), |_, w, cx| {
                let grip = w.find("parameter-resize").bounds().center();
                w.drag(grip, point(grip.x, grip.y + px(120.)), cx);
                assert_eq!(view.read(cx).tabs[0].row_height_override, Some(9));
                assert!(
                    !view.read(cx).tabs[0].dirty,
                    "调整布局不应伪装成命令内容修改"
                );

                let grip = w.find("preview-splitter").bounds().center();
                w.drag(grip, point(grip.x - px(100.), grip.y), cx);
                let description_width = w.find("description-frame").bounds().size.width;
                w.click("toggle-log", cx);
                let grip = w.find("log-resize").bounds().center();
                w.drag(grip, point(grip.x, grip.y - px(80.)), cx);
                let log_height = w.find("log-dock").bounds().size.height;

                // 副本从来源页继承一次，然后单独调到三行，不反向影响来源页。
                w.right_click(("tab-context", 0usize), cx);
                w.click("context-copy", cx);
                assert_eq!(view.read(cx).tabs[1].row_height_override, Some(9));
                let grip = w.find("parameter-resize").bounds().center();
                w.drag(grip, point(grip.x, grip.y - px(240.)), cx);
                assert_eq!(view.read(cx).tabs[1].row_height_override, Some(3));
                assert_eq!(view.read(cx).tabs[0].row_height_override, Some(9));

                w.click("add-tab-title", cx);
                assert_eq!(
                    view.read(cx).tabs[2].row_height_override,
                    None,
                    "新页使用默认布局"
                );
                for (index, rows) in [(0, 9), (1, 3), (0, 9), (1, 3)] {
                    view.update(cx, |v, cx| v.select_tab(index, w, cx));
                    w.render_frame(cx);
                    assert_eq!(
                        w.find("parameter-list").bounds().size.height,
                        px(parameter_height(rows))
                    );
                    assert_eq!(
                        w.find("description-frame").bounds().size.width,
                        description_width
                    );
                    assert_eq!(w.find("log-dock").bounds().size.height, log_height);
                }
                w.click("toggle-log", cx);
                assert!(w.try_find("log-dock").is_none());
                w.click("toggle-log", cx);
                assert_eq!(w.find("log-dock").bounds().size.height, log_height);
                (description_width, log_height)
            })
            .unwrap();

        // 小窗口只临时限制实际尺寸，不能把记忆的偏好写小。
        cx.simulate_window_resize(handle.into(), size(px(640.), px(660.)));
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            assert!(w.find("description-frame").bounds().size.width < description_width);
            assert!(w.find("log-dock").bounds().size.height < log_height);
            assert!(w.find("preview-frame").bounds().size.height >= px(PREVIEW_MIN));
            view.update(cx, |v, cx| v.select_tab(0, w, cx));
            w.render_frame(cx);
            assert!(w.find("parameter-list").bounds().size.height < px(parameter_height(9)));
            assert_eq!(view.read(cx).tabs[0].row_height_override, Some(9));
        })
        .unwrap();
        cx.simulate_window_resize(handle.into(), size(px(1200.), px(1200.)));
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            assert_eq!(
                w.find("parameter-list").bounds().size.height,
                px(parameter_height(9))
            );
            assert_eq!(
                w.find("description-frame").bounds().size.width,
                description_width
            );
            assert_eq!(w.find("log-dock").bounds().size.height, log_height);

            view.update(cx, |v, cx| v.select_tab(1, w, cx));
            w.render_frame(cx);
            w.right_click(("tab-context", 0usize), cx);
            w.click("context-close", cx);
            assert_eq!(view.read(cx).active, 1);
            assert_eq!(view.read(cx).tabs[1].id, 1, "关闭前一页后仍是同一个副本页");
            assert_eq!(view.read(cx).open_tab_ids, vec![1, 2]);
            assert_eq!(
                w.find("parameter-list").bounds().size.height,
                px(parameter_height(3))
            );
            assert_eq!(
                w.find("description-frame").bounds().size.width,
                description_width
            );
            assert_eq!(w.find("log-dock").bounds().size.height, log_height);
            w.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn dragging_scrolled_parameter_rows_uses_visible_drop_position(cx: &mut TestAppContext) {
        use gpui::{point, ScrollDelta};
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut view = None;
        let handle = cx.open_window(size(px(900.), px(1000.)), |w, cx| {
            let entity = cx.new(|cx| super::fixture(12, w, cx));
            view = Some(entity.clone());
            Root::new(entity, w, cx)
        });
        let view = view.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            w.scroll(
                "parameter-items",
                ScrollDelta::Pixels(point(px(0.), px(-250.))),
                cx,
            );
            assert!(w.find(("move-row", 11usize)).visible());
            let from = w.find(("move-row", 11usize)).bounds().center();
            let row = w.find(("parameter-row", 9usize)).bounds();
            w.drag(from, point(row.left() + px(10.), row.top() + px(3.)), cx);
            assert_eq!(view.read(cx).tabs[0].rows[9].id, 11);
            assert_eq!(view.read(cx).tabs[0].rows[10].id, 9);
        })
        .unwrap();
    }

    #[gpui::test]
    fn selected_tab_dot_and_close_fit_and_contrast_in_all_accents(cx: &mut TestAppContext) {
        use crate::theme::Accent;
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut baseline = None;
        for mode in [Theme::Dark, Theme::Light] {
            for accent in Accent::ALL {
                cx.update(|cx| {
                    cx.set_global(accent);
                    theme::apply(mode, cx);
                });
                let mut view = None;
                let handle = cx.open_window(size(px(1200.), px(800.)), |w, cx| {
                    let entity = cx.new(|cx| CommandWorkspace::new_with_backend(w, cx, None,
                        Some(serde_json::json!({"theme": if mode == Theme::Dark { "dark" } else { "light" }, "gpui": {"accent": accent.key()}})), None));
                    view = Some(entity.clone());
                    Root::new(entity, w, cx)
                });
                cx.update_window(handle.into(), |_, w, cx| {
                    view.as_ref().unwrap().update(cx, |v, cx| {
                        v.tabs[0].dirty = true;
                        cx.notify();
                    });
                    w.click("add-tab-title", cx);
                    for id in 0usize..=1 {
                        let tab = w.find(("tab-context", id)).bounds();
                        let close = w.find(("close-tab", id)).bounds();
                        let dot = w.find(("dirty-dot", id)).bounds();
                        assert_eq!(close.size, size(px(20.), px(20.)));
                        assert_eq!(dot.size, size(px(6.), px(6.)));
                        assert!(close.top() >= tab.top() && close.bottom() <= tab.bottom());
                        assert!(dot.left() >= tab.left() && dot.right() < close.left());
                    }
                    let current = super::geometry(w);
                    if let Some(ref previous) = baseline {
                        assert_eq!(previous, &current, "切换主题/主色不能改变控件几何");
                    } else {
                        baseline = Some(current);
                    }
                    w.remove_window();
                })
                .unwrap();
                // 选中背景和状态点必须不同；主要按钮和标签必须同主色。
                let selected = cx.update(|cx| {
                    assert_eq!(*cx.global::<Accent>(), accent);
                    theme::palette(cx)
                });
                fn luminance(color: u32) -> f64 {
                    let channel = |shift| {
                        let v = ((color >> shift) & 255u32) as f64 / 255.;
                        if v <= 0.04045 {
                            v / 12.92
                        } else {
                            ((v + 0.055) / 1.055).powf(2.4)
                        }
                    };
                    0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
                }
                let contrast =
                    (luminance(selected.on_primary) + 0.05) / (luminance(selected.focus) + 0.05);
                assert!(contrast >= 4.5, "未保存点与选中背景对比度不足: {contrast}");
                assert_eq!(selected.primary, selected.focus);
            }
        }
    }

    #[gpui::test]
    fn settings_draft_apply_cancel_and_english_dropdown(cx: &mut TestAppContext) {
        use crate::i18n::Language;
        use gpui::component::IndexPath;
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut view = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            let v = cx.new(|cx| CommandWorkspace::new(w, cx));
            view = Some(v.clone());
            Root::new(v, w, cx)
        });
        let view = view.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            w.click("settings", cx);
        })
        .unwrap();
        cx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(300));
        cx.update_window(handle.into(), |_, w, cx| {
            let panel = view.read(cx).settings_panel.clone().unwrap();
            panel.update(cx, |p, cx| {
                for (state, index) in [
                    (&p.theme, 1),
                    (&p.accent, 1),
                    (&p.size, 14),
                    (&p.weight, 5),
                    (&p.language, 1),
                    (&p.motion, 1),
                ] {
                    state.update(cx, |s, cx| {
                        s.set_selected_index(Some(IndexPath::new(index)), w, cx)
                    });
                }
            });
            assert_eq!(
                view.read(cx).font_size,
                14,
                "draft must not change workspace"
            );
            assert_eq!(*cx.global::<Language>(), Language::Chinese);
            w.click("settings-apply", cx);
            assert_eq!(view.read(cx).font_size, 24);
            assert_eq!(view.read(cx).font_weight, 800);
            assert_eq!(*cx.global::<Language>(), Language::English);
            assert_eq!(*cx.global::<theme::Accent>(), theme::Accent::Teal);
            assert_eq!(*cx.global::<Theme>(), Theme::Light);
            assert!(view.read(cx).reduced_motion);
            assert_eq!(crate::i18n::tr(cx, "保存"), "Save");
            w.click("settings-copy-json", cx);
            let json: serde_json::Value =
                serde_json::from_str(&cx.read_from_clipboard().unwrap().text().unwrap()).unwrap();
            assert_eq!(json["gpui"]["font_size"], 24);
            assert_eq!(json["gpui"]["language"], "en_US");
            panel.update(cx, |p, cx| {
                p.size.update(cx, |s, cx| {
                    s.set_selected_index(Some(IndexPath::new(0)), w, cx)
                })
            });
            w.click("settings-close", cx);
            assert_eq!(
                view.read(cx).font_size,
                24,
                "cancel discards only changes since Apply"
            );
            w.click("settings", cx);
        })
        .unwrap();
        cx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(300));
        cx.update_window(handle.into(), |_, w, cx| {
            let panel = view.read(cx).settings_panel.clone().unwrap();
            assert_eq!(
                panel.read(cx).size.read(cx).selected_index(cx).unwrap().row,
                14
            );
            w.click("settings-accent", cx);
            w.press("down", cx);
            w.press("enter", cx);
            assert_eq!(
                panel
                    .read(cx)
                    .accent
                    .read(cx)
                    .selected_index(cx)
                    .unwrap()
                    .row,
                2
            );
        })
        .unwrap();
        cx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(300));
        cx.update_window(handle.into(), |_, w, cx| {
            w.click("settings-ok", cx);
            assert_eq!(
                *cx.global::<theme::Accent>(),
                theme::Accent::Violet,
                "keyboard selects accent from dropdown"
            );
            w.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn browse_directory_uses_native_directory_picker_and_updates_field(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut view = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            let entity = cx.new(|cx| CommandWorkspace::new(w, cx));
            view = Some(entity.clone());
            Root::new(entity, w, cx)
        });
        let view = view.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            w.click("browse-directory", cx);
        })
        .unwrap();
        assert!(cx.did_prompt_for_paths());
        let selected = std::env::temp_dir().join("ECR directory with spaces");
        cx.simulate_path_prompt_response(|options| {
            assert!(!options.files && options.directories && !options.multiple);
            Some(vec![selected.clone()])
        });
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            assert_eq!(
                w.find("working-directory").value(),
                Some(selected.to_string_lossy().as_ref())
            );
            assert!(view.read(cx).has_unsaved_edits_now(cx));
            w.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn native_configuration_dialogs_preserve_data_until_confirmed(cx: &mut TestAppContext) {
        use crate::backend::ConfigStore;
        use std::{
            fs,
            time::{SystemTime, UNIX_EPOCH},
        };
        let dir = std::env::temp_dir().join(format!(
            "ecr-file-dialogs-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::at(dir.join("config.json"), dir.join("backup"));
        store
            .save(&serde_json::json!({"tabs":[{"name":"original","program":"echo original"}]}))
            .unwrap();
        let source = dir.join("source.json");
        fs::write(
            &source,
            r#"{"tabs":[{"name":"imported","program":"echo imported"}]}"#,
        )
        .unwrap();
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut view = None;
        let handle = cx.open_window(size(px(850.), px(800.)), |w, cx| {
            let v = cx.new(|cx| {
                CommandWorkspace::new_with_backend(
                    w,
                    cx,
                    Some(store.clone()),
                    store.load().unwrap(),
                    None,
                )
            });
            view = Some(v.clone());
            Root::new(v, w, cx)
        });
        let view = view.unwrap();
        // Native picker cancellation does not dirty or replace anything.
        cx.update_window(handle.into(), |_, w, cx| {
            view.update(cx, |v, cx| v.prompt_import(false, w, cx))
        })
        .unwrap();
        assert!(cx.did_prompt_for_paths());
        cx.simulate_path_prompt_response(|options| {
            assert!(options.files && !options.directories && !options.multiple);
            None
        });
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            assert!(!view.read(cx).has_unsaved_edits());
            assert_eq!(view.read(cx).tabs[0].label(cx), "original");
            view.update(cx, |v, cx| v.prompt_import(true, w, cx));
        })
        .unwrap();
        cx.simulate_path_prompt_response(|_| Some(vec![source.clone()]));
        cx.run_until_parked();
        assert_eq!(
            store.load().unwrap().unwrap()["tabs"][0]["name"],
            "original"
        );
        assert_eq!(store.backups().unwrap().len(), 1);
        cx.update_window(handle.into(), |_, w, cx| {
            w.close_all_dialogs(cx);
            assert_eq!(view.read(cx).tabs[0].label(cx), "original");
            view.update(cx, |v, cx| v.prompt_import(false, w, cx));
        })
        .unwrap();
        cx.simulate_path_prompt_response(|_| Some(vec![source.clone()]));
        cx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(300));
        cx.update_window(handle.into(), |_, w, cx| {
            assert_eq!(view.read(cx).tabs[0].label(cx), "original");
            w.click("cancel-import", cx);
            assert_eq!(view.read(cx).tabs[0].label(cx), "original");
            view.update(cx, |v, cx| v.prompt_import(false, w, cx));
        })
        .unwrap();
        cx.simulate_path_prompt_response(|_| Some(vec![source.clone()]));
        cx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(300));
        cx.update_window(handle.into(), |_, w, cx| {
            w.click("confirm-import", cx);
            assert_eq!(view.read(cx).tabs[0].label(cx), "imported");
            view.update(cx, |v, cx| v.prompt_export(None, w, cx));
        })
        .unwrap();
        let exported = dir.join("export.json");
        cx.simulate_new_path_selection(|_| Some(exported.clone()));
        cx.run_until_parked();
        assert_eq!(
            crate::backend::read_configuration(&exported).unwrap()["tabs"][0]["program"],
            "echo imported"
        );
        assert!(store.backups().unwrap().len() >= 2);
        cx.update_window(handle.into(), |_, w, _| w.remove_window())
            .unwrap();
        fs::remove_dir_all(dir).unwrap();
    }

    #[gpui::test]
    fn qt_font_ranges_and_languages_fit_small_windows(cx: &mut TestAppContext) {
        use crate::{i18n::Language, settings::Preferences};
        cx.update(gpui::init);
        for language in [Language::Chinese, Language::English] {
            for size in [10, 14, 24] {
                cx.update(|cx| theme::apply(Theme::Dark, cx));
                let handle = cx.open_window(size_fn(640., 660.), |w, cx| {
                    let v = cx.new(|cx| {
                        let mut v = super::fixture(3, w, cx);
                        v.apply_preferences(
                            Preferences {
                                theme: Theme::Dark,
                                accent: theme::Accent::Blue,
                                font_size: size,
                                font_weight: 800,
                                language,
                                reduced_motion: true,
                            },
                            w,
                            cx,
                        )
                        .unwrap();
                        v
                    });
                    Root::new(v, w, cx)
                });
                cx.update_window(handle.into(), |_, w, cx| {
                    w.render_frame(cx);
                    super::validate(&super::geometry(w), 660., 3, false);
                    for id in [
                        "enabled-first",
                        "select-all",
                        "select-none",
                        "run-command",
                        "program",
                        "command-name",
                    ] {
                        let b = w.find(id).bounds();
                        assert!(
                            b.left() >= px(0.) && b.right() <= px(640.),
                            "{language:?} {size}px: {id} {b:?}"
                        );
                    }
                    w.click("settings", cx);
                    for id in ["settings-close", "settings-ok", "settings-accent"] {
                        let b = w.find(id).bounds();
                        assert!(
                            b.bottom() <= px(660.) && b.right() <= px(640.),
                            "{id} {b:?}"
                        );
                    }
                    w.remove_window();
                })
                .unwrap();
            }
        }
        fn size_fn(w: f32, h: f32) -> gpui::Size<gpui::Pixels> {
            gpui::size(px(w), px(h))
        }
    }

    #[gpui::test]
    fn log_zoom_shortcuts_are_focus_scoped_and_bounded(cx: &mut TestAppContext) {
        use gpui::Focusable;
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut view = None;
        let handle = cx.open_window(size(px(850.), px(1000.)), |w, cx| {
            let v = cx.new(|cx| {
                let mut v = CommandWorkspace::new(w, cx);
                v.show_log = true;
                v.log_height_override = Some(200.);
                v
            });
            view = Some(v.clone());
            Root::new(v, w, cx)
        });
        let view = view.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            let focus = view.read(cx).log_output.focus_handle(cx);
            w.focus(&focus, cx);
            w.press("ctrl-=", cx);
            assert_eq!(view.read(cx).log_font_size, 15);
            w.press("ctrl--", cx);
            assert_eq!(view.read(cx).log_font_size, 14);
            w.press("ctrl-+", cx);
            assert_eq!(view.read(cx).log_font_size, 15);
            w.press("ctrl-alt-=", cx);
            assert_eq!(view.read(cx).log_font_size, 15);
            if cfg!(target_os = "macos") {
                w.press("cmd-=", cx);
                assert_eq!(view.read(cx).log_font_size, 16);
            }
            for _ in 0..25 {
                w.press("ctrl-=", cx);
            }
            assert_eq!(view.read(cx).log_font_size, 32);
            for _ in 0..30 {
                w.press("ctrl--", cx);
            }
            assert_eq!(view.read(cx).log_font_size, 8);
            let input = view.read(cx).tabs[0].program.focus_handle(cx);
            w.focus(&input, cx);
            w.press("ctrl-=", cx);
            assert_eq!(view.read(cx).log_font_size, 8);
            assert_eq!(view.read(cx).font_size, 14);
            w.remove_window();
        })
        .unwrap();
    }

    #[gpui::test]
    fn tab_context_menu_duplicates_the_target_not_the_active_tab(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui::init(cx);
            theme::apply(Theme::Dark, cx);
        });
        let mut view = None;
        let handle = cx.open_window(size(px(1200.), px(800.)), |w, cx| {
            let entity = cx.new(|cx| CommandWorkspace::new(w, cx));
            view = Some(entity.clone());
            Root::new(entity, w, cx)
        });
        let view = view.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            w.click("add-tab-title", cx);
            assert_eq!(view.read(cx).active, 1);
            w.right_click(("tab-context", 0usize), cx);
            assert!(w.try_find("tab-context-menu").is_some());
            w.click("context-copy", cx);
            assert_eq!(view.read(cx).tabs.len(), 3);
            assert_eq!(view.read(cx).tabs[2].label(cx), "视频转换 副本");
            w.right_click(("tab-context", 0usize), cx);
            assert!(w.try_find("context-close").is_some());
            w.click("context-close", cx);
            assert_eq!(view.read(cx).tabs.len(), 3);
            assert!(!view.read(cx).open_tab_ids.contains(&0));
            assert!(view.read(cx).tabs.iter().any(|tab| tab.id == 0));
        })
        .unwrap();
    }
}
