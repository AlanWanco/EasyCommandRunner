# GPUI 原生打包与手动发布

本流程只打包 `gpui-prototype/` 的 **release** 二进制，不调用 CMake、Qt、macdeployqt 或 windeployqt。

## 发布入口

在 GitHub Actions 选择 **GPUI Release**（`gpui-release.yml`），手动输入 `v1.0.0`；`source_ref` 留空使用选中的 workflow 分支，或指定已有源码 ref。入口将 ref 固定为完整 Git SHA，检查 tag 等于 Cargo 版本，再调用独立 `gpui.yml` 五架构构建。

全部平台构建、测试、审计、打包及 artifact 上传成功后，发布 job 校验五个包的文件名、版本、源码 SHA 和 SHA256；先创建草稿并上传完整包集，再公开 Release。上传失败留下草稿，不公开部分产物。已发布 Release 永不覆盖，已有 tag 不允许指向另一份源码；同提交的中断草稿可手动重试。

普通 push／PR 只执行构建和测试，不打包、不发布。`GPUI build and test` 的手动运行可勾选 `package_artifacts` 做原生打包预检（**不会创建 Release**）。严禁使用旧 `Release`／`build.yml` 的 Qt 发布入口。

## 五架构产物

统一命名 `EasyCommandRunner-GPUI-v<版本>-<平台>-<架构>.<后缀>`：Windows x86_64/arm64 `.zip`，macOS arm64 `.dmg`，Linux x86_64/arm64 `.AppImage`。每个 artifact 同时包含 `<包名>.build-info.json`；发布额外汇总 `SHA256SUMS.txt`。版本来自 Cargo.toml，不从未验证的 tag 字符串拼接命令。

- Windows：复制主程序到 Portable 根目录，检查 GUI 子系统、原生架构、8 MiB 栈、嵌入图标；扫描实际导入的 MSVC DLL，仅复制对应架构的运行库。解压最终 ZIP 再验证主程序哈希和必备文件，不调用安装器。Windows 系统 UCRT 不随包复制。
- macOS：`.app/Contents/MacOS/EasyCommandRunner`、原生 ICNS、Info.plist、bundle ID `com.sleepykanata.EasyCommandRunner.GPUI`，并检查非系统 dylib。生成 ad-hoc 签名以保持 bundle 完整性；不是 Developer ID 签名／公证。DMG 加入 `/Applications` 链接，生成后只读挂载校验实际内容；不会打开或模拟操作 GUI。
- Linux：原生 Ubuntu 24.04 runner；linuxdeploy 部署 ELF 直接依赖，显式加入通过 dlopen 使用的 Ayatana AppIndicator 和 pixbuf loaders。自定义 AppRun 设置包内库／数据／schema 路径，并按当前挂载点重建 pixbuf cache。最终 AppImage 解包检查主程序、图标、AppIndicator 和动态依赖。系统 glibc／显卡驱动不打包，最低兼容基线必须明确写在发行说明。

Linux 打包工具来自 linuxdeploy 的 `continuous` Release：下载前通过 GitHub asset 元数据获取 SHA256，下载后验证摘要，缺失摘要立即失败。保存工具资产元数据以便审计；`continuous` 是移动入口，未来重复构建不保证使用相同工具，不能宣称字节级可复现。需要完全固定工具时，应另行审核并钉住工具版本／哈希。

## 本地准备与验证

```sh
cd gpui-prototype
cargo fmt -- --check
cargo test --all-targets --locked --features ui-test -- --test-threads=1
python3 scripts/test_release_support.py
cargo build --release --locked
export RELEASE_VERSION=1.0.0
export SOURCE_SHA="$(git rev-parse HEAD)"
# Apple Silicon/macOS:
bash scripts/package-macos.sh
# 原生 Linux x86_64 / ARM64:
bash scripts/package-linux.sh x86_64  # 或 arm64
# Windows PowerShell（相应原生架构）:
# $env:RELEASE_VERSION='1.0.0'; $env:SOURCE_SHA=(git rev-parse HEAD)
# pwsh -File scripts/package-windows.ps1 -Arch x86_64
```

Linux 构建／打包需要 `pkg-config libgtk-3-dev libayatana-appindicator3-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libx11-dev libfontconfig1-dev libfreetype6-dev binutils file dpkg-dev libgdk-pixbuf2.0-bin libglib2.0-bin`，以及 bash、python3、GitHub CLI、curl。打包脚本创建唯一临时目录，只清理自己的临时文件。

首次发布前必须在 Actions 原生 runner 实际通过 Linux 和 Windows 打包，不以本地 macOS 检查替代。Windows ARM64、Linux 各桌面／驱动和通知权限仍需要人工验收；所有签名／公证限制如实标注。修改打包配置先备份；提交和发布必须取得操作者审阅确认。
