use crate::core::{CommandExecutor, CommandParser, ConfigManager};
use cxx_qt::prelude::*;
use serde_json::json;
use std::sync::Mutex;

/// CXX-Qt 应用桥接
#[cxx_qt::bridge(cxx_file = "cpp/app.cpp")]
pub mod qobject {
    unsafe extern "C++" {
        include!("cpp/app.h");

        /// Qt 应用主窗口
        type AppWindow = crate::ui::app::AppWindowRust;
    }

    unsafe extern "rust" {
        /// 暴露给 Qt 的方法

        /// 解析命令字符串
        fn rust_parse_command(input: &str) -> Vec<String>;

        /// 构建完整命令
        fn rust_build_command(
            program: &str,
            functions: &[String],
            params: &[String],
            other_args: &str,
        ) -> String;

        /// 执行命令（捕获输出）
        fn rust_execute_command(program: &str, working_dir: &str, command_str: &str) -> QString;

        /// 执行命令（新窗口）
        fn rust_execute_command_new_window(
            program: &str,
            working_dir: &str,
            command_str: &str,
        ) -> QString;

        /// 保存配置
        fn rust_save_config(config_json: &str) -> bool;

        /// 加载配置
        fn rust_load_config() -> QString;

        /// 获取备份列表
        fn rust_get_backups() -> Vec<QString>;

        /// 恢复备份
        fn rust_restore_backup(backup_name: &str) -> bool;

        /// 切换主题
        fn rust_set_theme(theme: &str) -> QString;

        /// 获取当前主题
        fn rust_get_current_theme() -> QString;
    }
}

pub struct AppWindowRust {
    config_manager: Mutex<ConfigManager>,
    current_theme: Mutex<String>,
}

impl AppWindowRust {
    pub fn new() -> Self {
        AppWindowRust {
            config_manager: Mutex::new(ConfigManager::new("config.json", "backup")),
            current_theme: Mutex::new("dark".to_string()),
        }
    }
}

impl Default for AppWindowRust {
    fn default() -> Self {
        Self::new()
    }
}

// 暴露给 Qt 的实现

pub fn rust_parse_command(input: &str) -> Vec<String> {
    CommandParser::parse(input, false)
}

pub fn rust_build_command(
    program: &str,
    functions: &[String],
    params: &[String],
    other_args: &str,
) -> String {
    let mut funcs = Vec::new();
    for i in 0..functions.len() {
        if i < params.len() {
            funcs.push((functions[i].clone(), params[i].clone()));
        }
    }

    CommandParser::build(program, &funcs, other_args)
}

pub fn rust_execute_command(program: &str, working_dir: &str, command_str: &str) -> QString {
    match CommandExecutor::execute(program, working_dir, command_str, true) {
        Ok(result) => {
            let output = format!(
                "[{}] 命令执行完成 (Exit Code: {})\n\n{}{}",
                result.timestamp, result.exit_code, result.stdout, result.stderr
            );
            QString::from(&output)
        }
        Err(e) => QString::from(&format!("错误: {}", e)),
    }
}

pub fn rust_execute_command_new_window(
    program: &str,
    working_dir: &str,
    command_str: &str,
) -> QString {
    match CommandExecutor::execute(program, working_dir, command_str, false) {
        Ok(_) => QString::from("命令已启动"),
        Err(e) => QString::from(&format!("错误: {}", e)),
    }
}

pub fn rust_save_config(config_json: &str) -> bool {
    match serde_json::from_str::<serde_json::Value>(config_json) {
        Ok(config) => {
            if let Ok(manager) = ConfigManager::new("config.json", "backup").save(&config) {
                true
            } else {
                false
            }
        }
        Err(_) => false,
    }
}

pub fn rust_load_config() -> QString {
    let manager = ConfigManager::new("config.json", "backup");
    match manager.load() {
        Ok(config) => {
            if let Ok(json_str) = serde_json::to_string(&config) {
                QString::from(&json_str)
            } else {
                QString::from("")
            }
        }
        Err(_) => QString::from(""),
    }
}

pub fn rust_get_backups() -> Vec<QString> {
    let manager = ConfigManager::new("config.json", "backup");
    match manager.get_backups() {
        Ok(backups) => backups.iter().map(|b| QString::from(b)).collect(),
        Err(_) => Vec::new(),
    }
}

pub fn rust_restore_backup(backup_name: &str) -> bool {
    let manager = ConfigManager::new("config.json", "backup");
    manager.restore_backup(backup_name).is_ok()
}

pub fn rust_set_theme(theme: &str) -> QString {
    let stylesheet = if theme == "light" {
        include_str!("../../resources/stylesheet_light.qss").to_string()
    } else {
        include_str!("../../resources/stylesheet_dark.qss").to_string()
    };

    QString::from(&stylesheet)
}

pub fn rust_get_current_theme() -> QString {
    QString::from("dark") // 默认深色主题
}
