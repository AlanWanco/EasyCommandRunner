#pragma once

#include <QString>
#include <QVector>
#include <QPair>

/**
 * @brief Rust FFI 绑定接口
 * 
 * 这些函数将由 Rust 实现，从 C++ 调用
 */
extern "C" {
    // 命令执行 FFI
    typedef struct {
        int exit_code;
        const char* output;
        const char* error;
    } CommandResult;

    // 执行命令
    CommandResult rust_execute_command(
        const char* program,
        const char* working_dir,
        const char* command_str
    );

    // 释放命令结果内存
    void rust_free_command_result(CommandResult result);

    // 配置管理 FFI
    typedef struct {
        const char* json_data;
        int length;
    } ConfigData;

    // 加载配置
    ConfigData rust_load_config(const char* config_path, const char* backup_dir);

    // 保存配置
    bool rust_save_config(const char* config_path, const char* backup_dir, const char* json_data);

    // 释放配置数据内存
    void rust_free_config_data(ConfigData config);

    // 备份管理
    bool rust_create_backup(const char* config_path, const char* backup_dir);

    // 恢复备份
    bool rust_restore_backup(const char* config_path, const char* backup_dir, const char* backup_name);

    // 获取备份列表
    typedef struct {
        const char** paths;
        int count;
    } BackupList;

    BackupList rust_get_backups(const char* config_path, const char* backup_dir);

    void rust_free_backup_list(BackupList list);

    // 命令解析 FFI
    typedef struct {
        const char** commands;
        int count;
    } ParsedCommand;

    ParsedCommand rust_parse_command(const char* input, bool is_append);

    void rust_free_parsed_command(ParsedCommand cmd);

    // 跨平台命令构建 FFI
    typedef struct {
        const char* data;
        int length;
    } StringData;

    StringData rust_build_command(const char* input_json);
    void rust_free_string_data(StringData value);

    // 日志 FFI
    void rust_init_logger(void);
    void rust_log(const char* level, const char* message);
}
