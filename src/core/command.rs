use chrono::Local;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// 命令配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandConfig {
    pub tab_name: String,
    pub working_dir: String,
    pub program: String,
    pub functions: Vec<(String, String)>, // (功能, 参数)
    pub other_args: String,
    pub description: String,
    pub use_new_window: bool,
}

/// 命令执行结果
#[derive(Debug, Clone)]
pub struct CommandResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub timestamp: String,
}

pub struct CommandExecutor;

impl CommandExecutor {
    /// 执行命令
    pub fn execute(
        program: &str,
        working_dir: &str,
        command_str: &str,
        capture_output: bool,
    ) -> Result<CommandResult, String> {
        let working_path = if working_dir.is_empty() {
            std::env::current_dir().map_err(|e| format!("获取当前目录失败: {}", e))?
        } else {
            PathBuf::from(working_dir)
        };

        if !working_path.exists() {
            return Err(format!("工作目录不存在: {:?}", working_path));
        }

        let (stdout, stderr, exit_code) = if capture_output {
            let output = if cfg!(windows) {
                Command::new("cmd")
                    .args(&["/C", command_str])
                    .current_dir(&working_path)
                    .output()
                    .map_err(|e| format!("执行命令失败: {}", e))?
            } else {
                Command::new("sh")
                    .arg("-c")
                    .arg(command_str)
                    .current_dir(&working_path)
                    .output()
                    .map_err(|e| format!("执行命令失败: {}", e))?
            };

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code().unwrap_or(-1);

            (stdout, stderr, exit_code)
        } else {
            // 不捕获输出（用于需要交互的命令）
            if cfg!(windows) {
                Command::new("cmd")
                    .args(&["/K", command_str])
                    .current_dir(&working_path)
                    .spawn()
                    .map_err(|e| format!("启动命令失败: {}", e))?;
            } else {
                // Unix 系统
                #[cfg(target_os = "macos")]
                {
                    Command::new("open")
                        .args(&["-a", "Terminal"])
                        .arg(command_str)
                        .current_dir(&working_path)
                        .spawn()
                        .map_err(|e| format!("启动命令失败: {}", e))?;
                }

                #[cfg(not(target_os = "macos"))]
                {
                    // Linux
                    Command::new("sh")
                        .arg("-c")
                        .arg(&format!("x-terminal-emulator -e '{}'", command_str))
                        .current_dir(&working_path)
                        .spawn()
                        .map_err(|e| format!("启动命令失败: {}", e))?;
                }
            }
            (String::new(), String::new(), 0)
        };

        Ok(CommandResult {
            exit_code,
            stdout,
            stderr,
            timestamp: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_simple_command() {
        let result = if cfg!(windows) {
            CommandExecutor::execute("cmd", "", "echo hello", true)
        } else {
            CommandExecutor::execute("sh", "", "echo hello", true)
        };

        assert!(result.is_ok());
        let cmd_result = result.unwrap();
        assert_eq!(cmd_result.exit_code, 0);
        assert!(cmd_result.stdout.contains("hello"));
    }
}
