use crate::core::{CommandExecutor, CommandParser, ConfigManager};

/// 以下函数暴露给 C++ 调用
/// 它们是 FFI 的高级包装函数

/// 解析命令字符串
pub fn parse_command(input: &str) -> Vec<String> {
    CommandParser::parse(input, false)
}

/// 构建完整命令
pub fn build_command(
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

/// 执行命令
pub fn execute_command(program: &str, working_dir: &str, command_str: &str) -> String {
    match CommandExecutor::execute(program, working_dir, command_str, true) {
        Ok(result) => {
            format!(
                "Exit Code: {}\nOutput:\n{}\nError:\n{}",
                result.exit_code, result.stdout, result.stderr
            )
        }
        Err(e) => format!("Error: {}", e),
    }
}

/// 保存配置
pub fn save_config(config_path: &str, backup_dir: &str, config_json: &str) -> bool {
    match serde_json::from_str::<serde_json::Value>(config_json) {
        Ok(value) => {
            let manager = ConfigManager::new(config_path, backup_dir);
            manager.save(&value).is_ok()
        }
        Err(_) => false,
    }
}

/// 加载配置
pub fn load_config(config_path: &str, backup_dir: &str) -> String {
    let manager = ConfigManager::new(config_path, backup_dir);
    match manager.load() {
        Ok(config) => serde_json::to_string(&config).unwrap_or_else(|_| "{}".to_string()),
        Err(_) => "{}".to_string(),
    }
}

/// 创建备份
pub fn create_backup(config_path: &str, backup_dir: &str) -> bool {
    let manager = ConfigManager::new(config_path, backup_dir);
    // 备份是在 save() 时自动创建的，但我们可以手动触发
    if let Ok(config) = manager.load() {
        manager.save(&config).is_ok()
    } else {
        false
    }
}

/// 获取备份列表
pub fn get_backups(config_path: &str, backup_dir: &str) -> Vec<String> {
    let manager = ConfigManager::new(config_path, backup_dir);
    match manager.get_backups() {
        Ok(backups) => backups,
        Err(_) => Vec::new(),
    }
}

/// 恢复备份
pub fn restore_backup(config_path: &str, backup_dir: &str, backup_name: &str) -> bool {
    let manager = ConfigManager::new(config_path, backup_dir);
    manager.restore_backup(backup_name).is_ok()
}
