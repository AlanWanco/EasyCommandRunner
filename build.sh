#!/bin/bash
# build.sh - EasyCommandRunner Rust + Qt6 Build Script
# Supports: macOS, Linux, Windows (MSYS2/MinGW)

set -e

PROJECT_NAME="EasyCommandRunner"
BUILD_DIR="build"
INSTALL_DIR="dist"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Functions
print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

check_dependencies() {
    print_info "检查依赖..."

    # Check for Rust
    if ! command -v rustc &> /dev/null; then
        print_error "Rust 未安装。请从 https://rustup.rs 安装 Rust"
        exit 1
    fi
    print_info "✓ Rust $(rustc --version | cut -d' ' -f2)"

    # Check for CMake
    if ! command -v cmake &> /dev/null; then
        print_error "CMake 未安装。请安装 CMake 3.24+"
        exit 1
    fi
    print_info "✓ CMake $(cmake --version | grep version | cut -d' ' -f3)"

    # Check for Qt6
    if ! command -v qmake &> /dev/null && ! command -v qmake6 &> /dev/null; then
        print_error "Qt6 未安装。请安装 Qt6"
        echo "  macOS: brew install qt6"
        echo "  Ubuntu: sudo apt-get install qt6-base-dev"
        echo "  Windows: 从 https://www.qt.io 下载安装程序"
        exit 1
    fi
    print_info "✓ Qt6 已安装"

    # Check for C++ compiler
    if ! command -v clang++ &> /dev/null && ! command -v g++ &> /dev/null && ! command -v cl.exe &> /dev/null; then
        print_error "C++ 编译器未找到"
        exit 1
    fi
    print_info "✓ C++ 编译器已安装"
}

build_release() {
    print_info "构建 Release 版本..."

    # Create build directory
    mkdir -p "$BUILD_DIR"
    cd "$BUILD_DIR"

    # Run CMake
    cmake -DCMAKE_BUILD_TYPE=Release ..
    cmake --build . --config Release -j$(nproc || echo 4)

    cd ..
    print_info "✓ Release 版本构建完成"
}

build_debug() {
    print_info "构建 Debug 版本..."

    # Create build directory
    mkdir -p "$BUILD_DIR"
    cd "$BUILD_DIR"

    # Run CMake
    cmake -DCMAKE_BUILD_TYPE=Debug ..
    cmake --build . --config Debug

    cd ..
    print_info "✓ Debug 版本构建完成"
}

install_release() {
    print_info "安装 Release 版本..."

    mkdir -p "$INSTALL_DIR"
    cd "$BUILD_DIR"
    cmake --install . --prefix "../$INSTALL_DIR" --config Release
    cd ..

    print_info "✓ 安装完成到 $INSTALL_DIR"
}

run_tests() {
    print_info "运行测试..."
    cargo test --release
    print_info "✓ 测试完成"
}

clean() {
    print_info "清理构建文件..."
    rm -rf "$BUILD_DIR" "$INSTALL_DIR" target
    print_info "✓ 清理完成"
}

show_usage() {
    cat << EOF
用法: ./build.sh [命令]

命令:
  check       检查依赖（默认行为）
  debug       构建 Debug 版本
  release     构建 Release 版本（推荐用于发布）
  install     安装 Release 版本到 dist/ 目录
  test        运行单元测试
  clean       清理所有构建文件
  help        显示此帮助信息

示例:
  ./build.sh check           # 检查依赖是否满足
  ./build.sh release         # 构建 Release 版本
  ./build.sh install         # 安装到 dist/ 目录
  ./build.sh clean           # 清理所有构建文件
EOF
}

# Main
case "${1:-check}" in
    check)
        check_dependencies
        ;;
    debug)
        check_dependencies
        build_debug
        ;;
    release)
        check_dependencies
        build_release
        ;;
    install)
        check_dependencies
        build_release
        install_release
        ;;
    test)
        print_info "运行测试..."
        cargo test --release -- --test-threads=1
        ;;
    clean)
        clean
        ;;
    help)
        show_usage
        ;;
    *)
        print_error "未知命令: $1"
        show_usage
        exit 1
        ;;
esac

print_info "完成！"
