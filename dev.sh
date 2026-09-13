#!/usr/bin/env bash
# Debug 构建后直接运行 Qt GUI；在 tmux 中调用本脚本即可保留构建/运行输出。
set -euo pipefail
cd "$(dirname "$0")"
cmake -S . -B build -DCMAKE_BUILD_TYPE=Debug
cmake --build build --parallel 4 --target EasyCommandRunner

# 与旧版配置、系统 QSettings 隔离，不覆盖现有用户数据。
export ECR_DATA_DIR="${ECR_DATA_DIR:-$PWD/build/dev-data}"
mkdir -p "$ECR_DATA_DIR"
for file in config.json settings.ini; do
    if [[ -f "$ECR_DATA_DIR/$file" ]]; then
        mkdir -p "$ECR_DATA_DIR/dev-backup"
        cp "$ECR_DATA_DIR/$file" "$ECR_DATA_DIR/dev-backup/${file}.$(date +%Y%m%d_%H%M%S)"
    fi
done
printf '\n开发数据目录：%s\n' "$ECR_DATA_DIR"
case "$(uname -s)" in
    Darwin) exec ./build/EasyCommandRunner.app/Contents/MacOS/EasyCommandRunner ;;
    MINGW*|MSYS*|CYGWIN*) exec ./build/EasyCommandRunner.exe ;;
    *) exec ./build/EasyCommandRunner ;;
esac
