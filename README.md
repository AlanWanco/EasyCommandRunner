# EasyCommandRunner · GPUI

![应用图标](resources/app_icon.png)

用图形界面保存、编辑和运行可复用的命令模板。当前默认分支 **`gpui`** 使用 **Rust + GPUI Kit**；Qt 版保留在 [`qt-legacy`](https://github.com/AlanWanco/EasyCommandRunner/tree/qt-legacy) / [`rust`](https://github.com/AlanWanco/EasyCommandRunner/tree/rust)，PyQt5 历史版保留在 [`legacy`](https://github.com/AlanWanco/EasyCommandRunner/tree/legacy)。

## 下载与平台

正式包以 [GitHub Releases](https://github.com/AlanWanco/EasyCommandRunner/releases) 中明确标注 **GPUI** 的版本为准；GPUI 版本线从 **`v1.0.0`** 开始。若页面尚无 GPUI Release，请勿将 Qt `v0.9.x` 或 CI 调试产物当作正式 GPUI 包。只有独立手动发布流程会创建 Release。

| 平台 | GPUI 发布包命名（以 v1.0.0 为例） | 使用方法 |
| --- | --- | --- |
| Windows x86_64 | `EasyCommandRunner-GPUI-v1.0.0-windows-x86_64.zip` | 完整解压，运行 `EasyCommandRunner.exe` |
| Windows ARM64 | `EasyCommandRunner-GPUI-v1.0.0-windows-arm64.zip` | 原生 ARM64 Windows，完整解压运行 |
| macOS ARM64 | `EasyCommandRunner-GPUI-v1.0.0-macos-arm64.dmg` | 将 `.app` 拖到 Applications |
| Linux x86_64 | `EasyCommandRunner-GPUI-v1.0.0-linux-x86_64.AppImage` | `chmod +x` 后运行 |
| Linux ARM64 | `EasyCommandRunner-GPUI-v1.0.0-linux-arm64.AppImage` | 原生 ARM64 Linux，`chmod +x` 后运行 |

- Windows 包是免安装 Portable ZIP，不会安装 Qt；配置仍默认写入用户 APPDATA，不是自动写入 EXE 旁边。
- macOS 当前测试基线为 Apple Silicon / macOS 14+。`.app` 使用一致的 bundle identifier 和原生图标，DMG 包含 `/Applications` 链接。没有 Developer ID 签名或公证，Gatekeeper 可能要求用户明确允许。
- Linux 当前构建基线为 Ubuntu 24.04 / glibc 2.39+；AppImage 不等于兼容所有发行版，不包含系统 GPU 驱动或 glibc。无 FUSE 时可用 `APPIMAGE_EXTRACT_AND_RUN=1`。托盘依赖桌面状态区支持。
- 每次 Release 附带 `SHA256SUMS.txt` 与各包 `build-info.json`，记录源码提交、release 编译输入及包校验和。
- CI 原始调试二进制不是正式发行包，尤其不能分发 Windows debug EXE。Windows release EXE 的 shader 已内嵌。

程序不附带 ffmpeg、yt-dlp、Python 等命令行工具；请自行安装，并保证 PATH／配置中的程序路径可用。

## 主要功能

- 顶部已打开标签与左侧全部配置分离，支持拖放排序、新建、复制、关闭视图及确认删除；关闭视图不会删除未保存的配置草稿。
- 参数编辑、勾选、拖放、命令解析／追加解析、实时命令预览、工作目录选择和备注；描述不参与命令执行。
- 深浅主题、主色、字号、字重、中英文与减少动效；约 260ms 的整页滑动／渐隐渐现不替换真实输入实体或 IME 状态。
- 离线 SVG 图标库、随机图标及自定义 SVG；**点击保存／Ctrl+S 后才持久化图标和命令编辑**。
- 真实 Shell 子进程、连续 stdout/stderr、中文／emoji 增量解码、停止进程组、会话运行历史与系统完成通知。
- 日志默认跟随底部，上滚暂停，点击“回到底部”恢复；支持分离窗口、重新停靠和日志字号快捷键。
- 系统托盘、未保存退出确认、Qt 兼容配置读写、备份轮换、确认恢复和配置导入／导出。

系统通知受 OS 权限／勿扰模式控制；macOS 需从 `.app` 启动。Linux/Windows ARM64 的 CI 通过不代表所有物理设备、桌面和 IME 场景已验收。日志复制／保存文件仍未接入。

## 配置与使用

1. 新建配置，填写程序、工作目录及参数，核对实时预览。
2. 点击运行，在日志区查看输出；向上滚动查看历史时不会被新输出拉回末尾。
3. 点击保存（Windows/Linux `Ctrl+S`，macOS `Cmd+S`）保存全部当前配置编辑、图标和布局。

默认配置目录：

- Windows：`%APPDATA%\SleepyKanata\EasyCommandRunner`
- macOS：`~/Library/Application Support/SleepyKanata/EasyCommandRunner`
- Linux：`${XDG_CONFIG_HOME:-~/.config}/easy-command-runner`

设置 `ECR_DATA_DIR` 可使用指定目录；配置与备份留在该目录，不写入 AppImage 挂载目录或 `.app` 资源。升级／切换 Qt 版前请备份 `config.json`；从 Qt 再次保存可能丢弃 GPUI 专用设置与图标缓存。

## 从源码运行

```sh
cd gpui-prototype
cargo run --locked
cargo test --all-targets --locked --features ui-test -- --test-threads=1
```

必须从 `gpui-prototype/` 执行 Cargo，以选择固定的 Rust **1.95.0**。不要在仓库根目录执行 Qt/CMake 或使用根目录的旧 Rust 工具链构建 GPUI。Linux 构建依赖见 [GPUI 发布说明](gpui-prototype/packaging/README.md)，开发与界面说明见 [GPUI README](gpui-prototype/README.md)。

## 发布与历史版

- `.github/workflows/gpui.yml`：独立五架构构建、测试及可选正式打包；普通 push 不发布 Release。
- `.github/workflows/gpui-release.yml`：仅手动运行 **GPUI Release**，输入 tag 和可选源码 ref。先固定源码提交，五个平台全部打包校验成功后，再完整发布。
- 旧 `.github/workflows/build.yml` / `release.yml` 属于 Qt，**不要用旧的 Release 工作流发布 GPUI**。旧分支及已发布的 Qt Release 不覆盖、不删除。

主程序 MIT 许可证见 [LICENSE](LICENSE)；图标和 Rust／Linux 依赖保留各自许可证，发行包包含相应通知。
