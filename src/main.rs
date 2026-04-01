mod core;
mod ui;
mod utils;

use utils::Logger;

fn main() {
    // 初始化日志
    Logger::init();

    // Qt 应用初始化（将在 C++ 侧处理）
    // 这里只需要确保 Rust 运行时已初始化
    println!("EasyCommandRunner 已启动");

    // Qt 应用将从 C++ main() 启动
    // 我们在这里做最小的初始化
}
