pub mod core;
pub mod ui;
pub mod utils;

// 重新导出常用的类型和函数
pub use core::{CommandConfig, CommandExecutor, CommandParser, CommandResult, ConfigManager};
pub use utils::Logger;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_command() {
        let result = CommandParser::parse("program -flag value", false);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_build_command() {
        let funcs = vec![("-c".to_string(), "4".to_string())];
        let cmd = CommandParser::build("ping", &funcs, "www.baidu.com");
        assert!(cmd.contains("ping"));
    }
}
