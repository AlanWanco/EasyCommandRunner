use chrono::Local;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

/// 配置管理器
pub struct ConfigManager {
    config_path: String,
    backup_dir: String,
}

impl ConfigManager {
    pub fn new(config_path: &str, backup_dir: &str) -> Self {
        // 确保备份目录存在
        let _ = fs::create_dir_all(backup_dir);

        ConfigManager {
            config_path: config_path.to_string(),
            backup_dir: backup_dir.to_string(),
        }
    }

    /// 加载配置
    pub fn load(&self) -> Result<Value, String> {
        if !Path::new(&self.config_path).exists() {
            return Ok(json!({
                "tabs": [],
                "line_codes": [],
                "checkbox_statuses": [],
                "theme": "dark",
                "language": "zh_CN",
                "window_geometry": null,
                "current_tab_index": 0
            }));
        }

        let content = fs::read_to_string(&self.config_path)
            .map_err(|e| format!("读取配置文件失败: {}", e))?;

        serde_json::from_str(&content).map_err(|e| format!("解析配置文件失败: {}", e))
    }

    /// 保存配置
    pub fn save(&self, config: &Value) -> Result<(), String> {
        // 自动备份
        self.backup_if_changed(config)?;

        let json_str =
            serde_json::to_string_pretty(config).map_err(|e| format!("序列化配置失败: {}", e))?;

        fs::write(&self.config_path, json_str).map_err(|e| format!("保存配置文件失败: {}", e))
    }

    /// 自动备份
    fn backup_if_changed(&self, new_config: &Value) -> Result<(), String> {
        if !Path::new(&self.config_path).exists() {
            return Ok(());
        }

        let old_content =
            fs::read_to_string(&self.config_path).map_err(|e| format!("读取旧配置失败: {}", e))?;

        let new_content = serde_json::to_string_pretty(new_config)
            .map_err(|e| format!("序列化新配置失败: {}", e))?;

        if old_content != new_content {
            // 创建备份
            let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
            let backup_path = format!("{}/config_{}.json", self.backup_dir, timestamp);

            fs::write(&backup_path, &old_content).map_err(|e| format!("创建备份失败: {}", e))?;

            // 只保留最近 10 个备份
            self.cleanup_old_backups()?;
        }

        Ok(())
    }

    /// 清理旧备份
    fn cleanup_old_backups(&self) -> Result<(), String> {
        let entries =
            fs::read_dir(&self.backup_dir).map_err(|e| format!("读取备份目录失败: {}", e))?;

        let mut backup_files: Vec<_> = entries
            .filter_map(|entry| {
                entry
                    .ok()
                    .filter(|e| e.path().extension().map_or(false, |ext| ext == "json"))
            })
            .collect();

        if backup_files.len() > 10 {
            backup_files.sort_by_key(|e| {
                e.metadata()
                    .map(|m| {
                        m.modified()
                            .unwrap_or_else(|_| std::time::SystemTime::now())
                    })
                    .unwrap_or_else(|_| std::time::SystemTime::now())
            });

            // 删除最旧的备份
            for file in backup_files.iter().take(backup_files.len() - 10) {
                let _ = fs::remove_file(file.path());
            }
        }

        Ok(())
    }

    /// 获取所有备份文件
    pub fn get_backups(&self) -> Result<Vec<String>, String> {
        let entries =
            fs::read_dir(&self.backup_dir).map_err(|e| format!("读取备份目录失败: {}", e))?;

        let mut backups: Vec<String> = entries
            .filter_map(|entry| {
                entry
                    .ok()
                    .and_then(|e| e.file_name().into_string().ok())
                    .filter(|name| name.ends_with(".json"))
            })
            .collect();

        backups.sort();
        backups.reverse();

        Ok(backups)
    }

    /// 恢复备份
    pub fn restore_backup(&self, backup_name: &str) -> Result<(), String> {
        let backup_path = format!("{}/{}", self.backup_dir, backup_name);

        if !Path::new(&backup_path).exists() {
            return Err(format!("备份文件不存在: {}", backup_path));
        }

        let content =
            fs::read_to_string(&backup_path).map_err(|e| format!("读取备份文件失败: {}", e))?;

        fs::write(&self.config_path, content).map_err(|e| format!("恢复备份失败: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir
            .path()
            .join("config.json")
            .to_string_lossy()
            .to_string();
        let backup_dir = temp_dir.path().join("backup").to_string_lossy().to_string();

        let manager = ConfigManager::new(&config_path, &backup_dir);

        let config = json!({
            "tabs": ["tab1", "tab2"],
            "theme": "dark"
        });

        // 保存
        assert!(manager.save(&config).is_ok());

        // 加载
        let loaded = manager.load().unwrap();
        assert_eq!(loaded["tabs"], json!(["tab1", "tab2"]));
    }
}
