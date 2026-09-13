use chrono::Local;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

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

fn shell_command(
    command: &str,
    working_path: &PathBuf,
    capture_output: bool,
) -> Result<(String, String, i32), String> {
    #[cfg(windows)]
    let shell = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());
    #[cfg(not(windows))]
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());

    let mut process = Command::new(shell);
    #[cfg(windows)]
    process.args(["/D", "/S", "/C", command]);
    #[cfg(not(windows))]
    process.args(["-c", command]);
    process.current_dir(working_path);

    if capture_output {
        let output = process
            .output()
            .map_err(|e| format!("执行命令失败: {}", e))?;
        Ok((
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
            output.status.code().unwrap_or(-1),
        ))
    } else {
        process
            .spawn()
            .map_err(|e| format!("启动命令失败: {}", e))?;
        Ok((String::new(), String::new(), 0))
    }
}

impl CommandExecutor {
    /// 执行命令
    pub fn execute(
        _program: &str,
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

        let (stdout, stderr, exit_code) =
            shell_command(command_str, &working_path, capture_output)?;

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
