//! DESIGN.md A.2–A.5 / B.4 / B.7：所有主题共享同一套几何。
pub const CONTROL: f32 = 32.;
pub const ICON: f32 = 16.;
pub const RADIUS: f32 = 6.;
pub const FIELD_PADDING: f32 = 10.;
pub const BUTTON_PADDING: f32 = 12.;
pub const GAP: f32 = 8.;
pub const INPUT_GAP: f32 = 12.;
pub const SECTION: f32 = 16.;
pub const PAGE_INSET: f32 = 24.;
pub const BODY: f32 = 14.;
pub const SMALL: f32 = 12.;
pub const CODE: f32 = 13.;
pub const LINE: f32 = 20.;
pub const LABEL: f32 = 64.;
pub const GRIP: f32 = 24.;
pub const TITLE_HEIGHT: f32 = 48.;
pub const STATUS_HEIGHT: f32 = 24.;
pub const PREVIEW_MIN: f32 = 72.;
pub const PREVIEW_HEADER_HEIGHT: f32 = LINE;
pub const TABLE_INSET: f32 = 4.;
pub const HEADER_HEIGHT: f32 = LINE;
pub const HEADER_GAP: f32 = 4.;
pub const DEFAULT_VISIBLE_ROWS: usize = 6;
pub const MAX_VISIBLE_ROWS: usize = 9;
pub const COMMAND_NAME_LINE_HEIGHT: f32 = 40.;
pub const COMMAND_NAME_HEIGHT: f32 = 48.;
pub const SIDEBAR_SCROLL_LANE_RIGHT: f32 = 2.;
pub const SIDEBAR_SCROLL_THUMB_INSET: f32 = 1.;
pub const SIDEBAR_SCROLL_THUMB_MAX_WIDTH: f32 = 8.;
pub const SIDEBAR_HIGHLIGHT_SCROLL_GAP: f32 = 3.;
pub const SIDEBAR_LIST_RIGHT_PADDING: f32 = SIDEBAR_SCROLL_LANE_RIGHT
    + SIDEBAR_SCROLL_THUMB_INSET
    + SIDEBAR_SCROLL_THUMB_MAX_WIDTH
    + SIDEBAR_HIGHLIGHT_SCROLL_GAP;
// Include the taller title in the fixed layout budget so small windows retain a usable preview.
// The divider is painted inside the existing section gap, without consuming another row.
pub const FORM_HEIGHT: f32 = COMMAND_NAME_HEIGHT + CONTROL * 2. + GAP * 2.;
pub const TABLE_CHROME: f32 = HEADER_HEIGHT + HEADER_GAP + GAP + CONTROL;
// 32px controls surrounded by 4px vertical padding and a 1px accent stroke.
pub const EXTRAS_HEIGHT: f32 = CONTROL + GAP + 2.;

/// 固定区域之和，不含参数视口、辅助字段、预览正文和日志。
/// 编辑器结构：表单 / 参数+追加字段 / 预览+运行操作，区块间 16px，组内 8px。
pub const FIXED_HEIGHT: f32 = TITLE_HEIGHT
    + STATUS_HEIGHT
    + SECTION * 2.
    + FORM_HEIGHT
    + SECTION
    + TABLE_CHROME
    + GAP
    + SECTION
    + GAP
    + CONTROL
    + GAP
    + PREVIEW_HEADER_HEIGHT;

pub fn parameter_height(rows: usize) -> f32 {
    let rows = rows.clamp(1, MAX_VISIBLE_ROWS) as f32;
    TABLE_INSET * 2. + rows * CONTROL + (rows - 1.) * GAP
}

#[derive(Clone, Copy, Debug)]
pub struct WorkspaceLayout {
    pub rows_height: f32,
    pub log_height: f32,
}

impl WorkspaceLayout {
    pub fn new(height: f32, row_count: usize, show_log: bool) -> Self {
        Self::with_overrides(height, row_count, show_log, None, None)
    }

    pub fn with_overrides(
        height: f32,
        row_count: usize,
        show_log: bool,
        rows_override: Option<usize>,
        log_override: Option<f32>,
    ) -> Self {
        let max_rows = row_count.clamp(1, MAX_VISIBLE_ROWS);
        let wanted = rows_override
            .unwrap_or(max_rows.min(DEFAULT_VISIBLE_ROWS))
            .clamp(1, max_rows);
        let budget = (height - FIXED_HEIGHT - EXTRAS_HEIGHT - PREVIEW_MIN).max(0.);
        // 窗口足够时保留三行及完整预览；小屏仍优先避免裁切预览和半行控件。
        let min_rows = if row_count > 3 && budget >= parameter_height(3) + 32. {
            3
        } else {
            1
        };
        let max_log = (budget - parameter_height(min_rows)).max(0.);
        let preferred_log = log_override.unwrap_or_else(|| {
            ((budget - parameter_height(wanted)) / 4.)
                .floor()
                .clamp(8., 40.)
                * 4.
        });
        let log_height = if show_log {
            preferred_log.clamp(0., max_log)
        } else {
            0.
        };
        let available = budget - log_height;
        let fit = ((available - TABLE_INSET * 2. + GAP) / (CONTROL + GAP))
            .floor()
            .max(1.) as usize;
        Self {
            rows_height: parameter_height(wanted.min(fit)),
            log_height,
        }
    }
}

/// 表头和每一行共用这个网格，绝不按文本内容分配列宽。
#[derive(Clone, Copy, Debug)]
pub struct ParameterGrid {
    pub option: f32,
    pub value: f32,
    pub note: Option<f32>,
}

impl ParameterGrid {
    pub fn new(width: f32, compact: bool) -> Self {
        let fixed = GRIP + CONTROL * 2. + INPUT_GAP * 5.;
        let available = (width - fixed - if compact { CONTROL } else { 0. }).max(0.);
        if compact {
            Self {
                option: available * 0.45,
                value: available * 0.55,
                note: None,
            }
        } else {
            Self {
                option: available * 28. / 88.,
                value: available * 32. / 88.,
                note: Some(available * 28. / 88.),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parameter_height_follows_content_not_window() {
        for rows in 0..=10 {
            let expected = parameter_height(rows.min(DEFAULT_VISIBLE_ROWS));
            for height in [1000., 1100., 1200.] {
                let layout = WorkspaceLayout::new(height, rows, false);
                assert_eq!(layout.rows_height, expected);
            }
        }
        assert_eq!(
            WorkspaceLayout::new(800., 3, false).rows_height,
            parameter_height(3)
        );
        assert_eq!(
            WorkspaceLayout::new(700., 3, false).rows_height,
            parameter_height(3)
        );
        assert_eq!(parameter_height(1), 40.);
        assert_eq!(parameter_height(3), 120.);
        assert_eq!(parameter_height(6), 240.);
        assert_eq!(parameter_height(7), 280.);
        assert_eq!(parameter_height(9), 360.);
    }

    #[test]
    fn compact_windows_reserve_preview_and_whole_rows() {
        for height in [660., 700., 800., 900.] {
            for rows in [0, 1, 3, 6, 12] {
                for log in [false, true] {
                    let layout = WorkspaceLayout::new(height, rows, log);
                    let used =
                        FIXED_HEIGHT + EXTRAS_HEIGHT + layout.rows_height + layout.log_height;
                    assert!(
                        height - used >= PREVIEW_MIN,
                        "{height}/{rows}/{log}: {layout:?}"
                    );
                    assert_eq!(
                        (layout.rows_height - TABLE_INSET * 2. + GAP) % (CONTROL + GAP),
                        0.
                    );
                }
            }
        }
    }

    #[test]
    fn resize_limits_follow_actual_row_count() {
        for count in 4..=14 {
            let max = count.min(9);
            assert_eq!(
                WorkspaceLayout::with_overrides(1100., count, false, Some(100), None).rows_height,
                parameter_height(max)
            );
            assert_eq!(
                WorkspaceLayout::with_overrides(1100., count, false, Some(3), None).rows_height,
                parameter_height(3)
            );
        }
        assert_eq!(
            WorkspaceLayout::with_overrides(800., 12, true, Some(9), Some(500.)).rows_height,
            parameter_height(3)
        );
    }

    #[test]
    fn columns_fill_available_width() {
        for width in [464., 584., 706., 1056.] {
            for compact in [false, true] {
                let grid = ParameterGrid::new(width, compact);
                let total = GRIP
                    + CONTROL * 2.
                    + INPUT_GAP * 5.
                    + grid.option
                    + grid.value
                    + grid.note.unwrap_or(CONTROL);
                assert!((total - width).abs() < 0.001);
                assert!(grid.option > 80. && grid.value > 96.);
            }
        }
    }
}
