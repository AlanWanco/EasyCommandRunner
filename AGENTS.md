# AGENTS.md

本文件记录 EasyCommandRunner 本次配置 GitHub Actions 时遇到的 CI、Qt、Rust/CMake 和打包问题。修改工作流或测试前先查阅本文件，避免重复踩坑。

## GitHub Actions 基线

- Qt 固定使用 **6.8.3**，不要随意升级或混用 Qt 版本。
- Linux/macOS 测试使用 `QT_QPA_PLATFORM=offscreen`；Windows 测试必须使用 `QT_QPA_PLATFORM=windows`。
  - Qt 6.8.3 的 MSVC 构建使用 `offscreen` 可能在 QTest 启动前崩溃，退出码表现为 `0xc0000409`。
- 所有 GUI 测试设置 `QT_SCALE_FACTOR=1`，避免不同 runner 的 DPI 缩放改变几何尺寸。
- Windows runner 可能不转发 QtTest/子进程的标准输出。Windows job 保留 `QT_FORCE_STDERR_LOGGING=1`，并直接运行测试程序，将结果写入文本文件后再用 PowerShell 输出：

  ```powershell
  $log = Join-Path $env:RUNNER_TEMP "ecr-ui-tests.txt"
  $test = Join-Path $env:GITHUB_WORKSPACE "build\Release\ecr_ui_tests.exe"
  & $test "-o" "$log,txt"
  $exitCode = $LASTEXITCODE
  Get-Content -LiteralPath $log
  if ($exitCode -ne 0) { exit $exitCode }
  ```

  仅使用 `ctest --output-on-failure` 时，Windows Actions 日志可能只显示失败退出码而没有 `FAIL!` 详情。
- Windows Hosted runner 使用 Hyper-V 虚拟显示器，工作区高度可能只有约 `815px`。GUI 测试不能无条件要求窗口成功调整到 `850x1000`，也不能在窗口已达到桌面最大高度时要求预览区域继续变大；这类断言必须对屏幕可用尺寸做兼容处理。
- `concurrency.cancel-in-progress: true` 会在同一分支的新提交到达时取消旧 run。看到旧 run 的其他 job 显示 `cancelled` 时，先确认是不是被新提交取消，不要直接判断为该架构构建失败。
- PowerShell 调用外部程序后要立即保存 `$LASTEXITCODE`。后续 `Get-Content`、文件操作可能改变该值。

## Qt 安装和架构名称

`jurplel/install-qt-action@v4` 的 `host`/`arch` 名称不能凭直觉填写。当前可用组合为：

| 平台 | runner | host | arch |
| --- | --- | --- | --- |
| Linux x86_64 | `ubuntu-24.04` | `linux` | `linux_gcc_64` |
| Linux ARM64 | `ubuntu-24.04-arm` | `linux_arm64` | `linux_gcc_arm64` |
| Windows x86_64 | `windows-2022` | `windows` | `win64_msvc2022_64` |
| Windows ARM64 | `windows-11-arm` | `windows_arm64` | `win64_msvc2022_arm64` |
| macOS ARM64 | `macos-14` | `mac` | `clang_64` |

已知错误：

- Linux Qt 6.8.3 的 x86_64 架构名是 `linux_gcc_64`，不是常见但错误的 `gcc_64`。
- Qt 6.8.3 传入 `modules: qtsvg` 会触发：
  `The packages ['qtsvg'] were not found while parsing XML of package information!`
  当前工作流不要传 `modules: qtsvg`；所有 job 都已移除该参数。
- macOS 的 `macdeployqt` 只会根据实际链接依赖部署模块。若代码通过 `QPixmap::loadFromData(..., "SVG")` 间接依赖 SVG 插件，打包后的 `.app` 可能所有 SVG 都失效；当前做法是显式加入 `Qt6::Svg`，并用 `QSvgRenderer` 直接渲染资源 SVG。
- 不要硬编码 Qt 安装目录。安装步骤之后使用 action 设置的 `QT_ROOT_DIR`，并将以下路径写入 `GITHUB_ENV`：
  `CMAKE_PREFIX_PATH`、`QT_PLUGIN_PATH`、`QT_QPA_PLATFORM_PLUGIN_PATH`。
- Windows 下建议通过 `Get-Command qmake.exe` 获取 qmake，再执行 `qmake -query QT_INSTALL_PLUGINS`，不要假设 Qt 路径或 shell 风格。
- ARM 使用原生 runner 和原生 Qt 包，不要在本工作流中追加复杂的交叉编译配置。Windows ARM CMake 配置必须使用 `-A ARM64`，x86_64 使用 `-A x64`。

## CMake、Rust 和 Windows 链接

- Visual Studio 是多配置生成器：
  - 配置：`cmake -S . -B build -G "Visual Studio 17 2022" -A x64`（ARM 使用 `-A ARM64`）。
  - 构建：`cmake --build build --config Release`。
  - Windows 可执行文件在 `build\Release\EasyCommandRunner.exe` 和 `build\Release\ecr_ui_tests.exe`，不是 `build\EasyCommandRunner.exe`。
- Ninja/Unix 是单配置生成器，可执行文件通常在 `build/EasyCommandRunner`。
- Rust 静态库是 CMake 的实际链接输入。仅添加一个 order-only 的自定义 target 不够：干净的 Ninja 构建会报：
  `ninja: error: ... libeasy_command_runner.a ... missing and no known rule to make it`。
  必须用 `add_custom_command(OUTPUT ${RUST_LIB_PATH} ...)` 显式声明 archive 输出，并让 `rust_build` 和应用/测试 target 依赖该输出；Windows 的 Rust 库扩展名为 `.lib`，Unix/macOS 为 `.a`。
- Rust 静态库中的 Windows 进程实现会导入 `NtCreateNamedPipeFile`。静态库不会自动把系统库依赖传播给最终目标，Windows 的应用和测试 target 都必须显式链接 `ntdll`，否则会出现：
  `unresolved external symbol __imp_NtCreateNamedPipeFile`。
- CI Rust 测试使用 `cargo test --all-targets --locked`；修改 `Cargo.toml` 时必须同步提交 `Cargo.lock`。
- 本地验证至少执行：

  ```text
  cmake -S . -B build -DECR_BUILD_TESTS=ON
  cmake --build build --config Release --parallel
  ctest --test-dir build -C Release --output-on-failure
  cargo test --all-targets --locked
  ```

## Windows Portable 打包

- 项目要求 Windows **免安装 Portable ZIP**，不要把 NSIS 安装器作为发布产物。虽然 `CMakeLists.txt` 仍保留 CPack 的 NSIS 配置，Actions 发布使用手动 staging + `Compress-Archive`。
- `windeployqt --dir dist build\Release\EasyCommandRunner.exe` 只会把 Qt DLL 和插件部署到 `dist`，**不会复制主程序本身**。必须先执行：

  ```powershell
  Copy-Item $exe dist\EasyCommandRunner.exe -Force
  & $windeployqt --release --no-translations --compiler-runtime --dir dist $exe
  ```

  否则最后的 `Test-Path dist\EasyCommandRunner.exe` 会失败：
  `windeployqt did not copy EasyCommandRunner.exe`。
- `windeployqt` 可能警告：
  `Cannot find Visual Studio installation directory, VCINSTALLDIR is not set.`
  这不是 windeployqt 的必然失败原因，但必须检查 ZIP 是否包含所需的 MSVC runtime（如 `vcruntime140.dll`、`vcruntime140_1.dll`、`msvcp140.dll`）。若目标 runner 没有自动提供，应通过 Visual Studio 安装路径或显式复制运行库解决。
- Qt SVG 依赖不能只看 CMake 的直接链接列表。当前程序使用 SVG 图标，Portable 目录需要确认存在 `Qt6Svg.dll`、`imageformats/qsvg.dll` 以及 `iconengines/qsvgicon.dll`；工作流保留了相应的 windeployqt/手动补齐逻辑。
- Qt 的 `setWindowIcon()` 只影响运行中的窗口，不会把图标嵌入 Windows PE 文件；Windows 目标必须额外编译 `resources/app_icon.rc` 和 `.ico`，否则 Explorer 中的 exe 可能显示默认图标。
- 打包前应检查主程序、Qt DLL、`platforms/qwindows.dll`、SVG 插件、嵌入式 exe 图标和 MSVC runtime 都在 ZIP 中，而不是只检查 ZIP 文件是否生成。
- x86_64 和 ARM64 必须分别使用对应 runner、Qt 包、ZIP 文件名和 artifact 名称，不能在 ARM job 中复用 x86_64 的 Qt 下载地址。

## Linux/macOS 打包

- Linux ELF 可执行文件本身通常不携带桌面图标；图标必须通过 `.desktop` 文件和 AppImage 的 icon theme 目录提供。`Icon=EasyCommandRunner` 必须和部署后的图标文件名一致。
- `linuxdeploy --icon-file resources/app_icon.png` 默认按输入文件名部署，不能只写 `app_icon.png` 再让 desktop 文件使用 `Icon=EasyCommandRunner`；当前工作流使用 `--icon-filename EasyCommandRunner`，并检查 `AppDir/usr/share/icons/hicolor/*/apps/EasyCommandRunner.png` 是否存在。
- linuxdeploy 会校验 PNG 的像素尺寸。当前 `app_icon.png` 为 `480x480`，属于当前 linuxdeploy 支持的尺寸；换图标时必须使用支持的正方形尺寸（例如 `256x256`），不能使用任意尺寸。
- Linux AppImage 使用与 runner 架构匹配的 linuxdeploy：x86_64 使用 `linuxdeploy-x86_64.AppImage`，ARM64 使用 `linuxdeploy-aarch64.AppImage`；不能混用。
- AppImage 打包需要 linuxdeploy Qt plugin，并设置 `APPIMAGE_EXTRACT_AND_RUN=1`；SVG 模块通过 `EXTRA_QT_MODULES="svg;"` 保留。
- macOS `.app` 的运行时 PNG 图标不会自动成为 Finder 的 bundle 图标；必须把原生 `app_icon.icns` 放进 `Contents/Resources`，并设置 `MACOSX_BUNDLE_ICON_FILE`。
- `macdeployqt build/EasyCommandRunner.app -dmg` 的快捷方式不会可靠地创建拖拽安装所需的 `/Applications` 链接。当前流程先运行 `macdeployqt` 部署依赖，再用 `hdiutil` 从 staging 目录生成 DMG，并显式创建 `ln -s /Applications Applications`。
- macOS ARM64 使用上述手动 staging 流程生成 DMG。
- 普通 push 的 `build.yml` 会构建、测试并上传各平台安装包 artifact，但不会创建 GitHub Release；PR 和普通手动运行默认只构建测试（Linux 可上传原始 CI 二进制）。`package_artifacts` 在可复用 workflow 中用于强制启用正式打包。

## Release 配置

- 构建和发布必须分开：`.github/workflows/build.yml` 负责 push/PR 的构建测试，同时作为可复用 workflow；`.github/workflows/release.yml` 只允许 `workflow_dispatch` 手动发布。
- 手动发布时输入 `tag`（例如 `v0.9.0`）。`release.yml` 会调用完整平台构建，所有平台成功后才执行发布；源码 ref 默认是手动运行时选择的分支。
- `source_ref` 可选，默认等于手动运行时选中的 workflow ref（通常是默认分支 `rust`），这样新 tag 不需要预先存在；如果要从已有 tag 做可复现构建，应明确填写该 tag。更新旧 tag 时也可保持 `source_ref` 为空，让当前分支重新打包并覆盖旧文件。
- 发布 job 必须等待可复用构建 job 完成，避免某个平台失败时仍发布不完整 Release。
- 全局权限可以是 `contents: read`；发布 workflow/job 单独声明 `permissions: contents: write`，否则 `softprops/action-gh-release` 无法创建或更新发布。
- `softprops/action-gh-release` 设置 `overwrite_files: true`，同一 tag 手动再次运行会覆盖旧 Release 文件；不要依赖 artifact 的旧版本。
- Release 文件列表必须和各平台实际产物名称完全一致：Linux x86_64/ARM64 AppImage、Windows x86_64/ARM64 ZIP、macOS ARM64 DMG。
- 修改 workflow 后先执行 `git diff --check`，再检查 YAML 语法；提交后确认每个架构 job 的配置、构建、测试、打包和 artifact 步骤都实际运行。

## 调试经验

- Actions API 或日志接口可能返回 `403`/rate limit，不能把日志 API 作为唯一诊断手段。测试程序应自行输出可读文本，必要时上传 `$RUNNER_TEMP` 中的测试日志 artifact。
- 首先区分三类问题：构建失败（CMake/Rust/链接）、测试失败（Qt 平台/屏幕几何/进程行为）、打包失败（部署目录缺文件）。不要看到 `windeployqt` 的 warning 就误判为失败，也不要只看到 CTest 的退出码就假设测试断言内容。
- 本地 Windows 通过不代表 Hosted runner 一定通过：窗口工作区尺寸、Qt 平台插件、PowerShell 进程输出和多配置路径都可能不同。修改 GUI 测试时优先使断言表达行为，而不是依赖 runner 的固定窗口尺寸。
