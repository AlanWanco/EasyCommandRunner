//! Command completion notifications through the operating system, not in-app toasts.
use crate::i18n::{self, tr};
use gpui::{App, SystemNotification};

const APP_ID: &str = "com.sleepykanata.EasyCommandRunner.GPUI";
const APP_NAME: &str = "EasyCommandRunner · GPUI";

pub fn init(cx: &App) {
    // Windows needs an AppUserModelID for unpackaged EXE notifications.
    // GPUI handles the native Windows/macOS/Linux delivery and permission policy.
    cx.set_app_identity(APP_ID, APP_NAME);
}

pub fn run_completion(
    run_id: usize,
    command_name: &str,
    exit_code: i32,
    stopped: bool,
    cx: &App,
) -> Option<SystemNotification> {
    if stopped {
        return None;
    }
    Some(SystemNotification {
        tag: format!("ecr-gpui-run-{}-{run_id}", std::process::id()).into(),
        title: tr(
            cx,
            if exit_code == 0 {
                "正常结束"
            } else {
                "运行报错"
            },
        )
        .into(),
        body: i18n::format(
            cx,
            "「{}」· 退出代码：{}",
            &[command_name, &exit_code.to_string()],
        )
        .into(),
        actions: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{self, Language};

    #[gpui::test]
    fn completion_uses_native_notifications_with_localized_status_and_distinct_tags(
        cx: &mut gpui::TestAppContext,
    ) {
        cx.update(|cx| {
            gpui::init(cx);
            init(cx);
            i18n::apply(Language::Chinese, cx);
            cx.show_system_notification(run_completion(0, "中文任务", 0, false, cx).unwrap());
            cx.show_system_notification(run_completion(1, "中文任务", 1, false, cx).unwrap());
        });
        let shown = cx.shown_system_notifications();
        assert_eq!(shown.len(), 2);
        assert_eq!(shown[0].title.as_ref(), "正常结束");
        assert_eq!(shown[0].body.as_ref(), "「中文任务」· 退出代码：0");
        assert_eq!(shown[1].title.as_ref(), "运行报错");
        assert_eq!(shown[1].body.as_ref(), "「中文任务」· 退出代码：1");
        assert_ne!(shown[0].tag, shown[1].tag);
        assert!(shown
            .iter()
            .all(|notification| notification.actions.is_empty()));
        assert_eq!(cx.app_identity().unwrap().0.as_ref(), APP_ID);
        cx.update(|cx| {
            i18n::apply(Language::English, cx);
            let error = run_completion(2, "用户任务", 7, false, cx).unwrap();
            assert_eq!(error.title.as_ref(), "Run failed");
            assert_eq!(error.body.as_ref(), "‘用户任务’ · Exit code: 7");
            assert_eq!(
                run_completion(3, "用户任务", 0, false, cx)
                    .unwrap()
                    .title
                    .as_ref(),
                "Finished successfully"
            );
            for code in [0, 1, -1] {
                assert!(run_completion(4, "主动停止", code, true, cx).is_none());
            }
        });
    }
}
