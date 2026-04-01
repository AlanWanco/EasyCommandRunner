use log::{debug, error, info, warn};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

pub struct Logger;

impl Logger {
    pub fn init() {
        let log_path = "easy_command_runner.log";

        // 初始化 env_logger
        env_logger::Builder::from_default_env()
            .format(|buf, record| {
                writeln!(
                    buf,
                    "[{}] {} - {}",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                    record.level(),
                    record.args()
                )
            })
            .init();

        info!("应用启动");
    }

    pub fn log_command(program: &str, command: &str) {
        info!("执行命令: {} {}", program, command);
    }

    pub fn log_error(error: &str) {
        error!("{}", error);
    }

    pub fn log_config_saved() {
        info!("配置已保存");
    }

    pub fn log_config_loaded() {
        info!("配置已加载");
    }
}
