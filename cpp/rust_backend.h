#include "ffi_bindings.h"
#include <QString>
#include <QJsonDocument>
#include <QJsonObject>
#include <memory>

/**
 * @brief C++ 包装类来调用 Rust FFI
 */
class RustBackend {
public:
    /**
     * 执行命令
     */
    static CommandResult executeCommand(
        const QString& program,
        const QString& workingDir,
        const QVector<QString>& args,
        bool useNewWindow = true)
    {
        const char* prog_c = program.toUtf8().constData();
        const char* dir_c = workingDir.isEmpty() ? nullptr : workingDir.toUtf8().constData();

        // 构建 C 字符串数组
        std::vector<const char*> args_c;
        std::vector<std::string> args_storage;
        for (const auto& arg : args) {
            args_storage.push_back(arg.toStdString());
            args_c.push_back(args_storage.back().c_str());
        }

        return rust_execute_command(
            prog_c,
            dir_c,
            args_c.data(),
            static_cast<int>(args_c.size()),
            useNewWindow
        );
    }

    /**
     * 加载配置
     */
    static QString loadConfig(const QString& configPath) {
        const char* path_c = configPath.toUtf8().constData();
        ConfigData config = rust_load_config(path_c);

        QString result;
        if (config.json_data) {
            result = QString::fromUtf8(config.json_data, config.length);
            rust_free_config_data(config);
        }
        return result;
    }

    /**
     * 保存配置
     */
    static bool saveConfig(const QString& configPath, const QString& jsonData) {
        const char* path_c = configPath.toUtf8().constData();
        const char* json_c = jsonData.toUtf8().constData();
        return rust_save_config(path_c, json_c);
    }

    /**
     * 创建备份
     */
    static bool createBackup(const QString& configPath) {
        const char* path_c = configPath.toUtf8().constData();
        return rust_create_backup(path_c);
    }

    /**
     * 恢复备份
     */
    static bool restoreBackup(const QString& configPath, const QString& backupPath) {
        const char* config_c = configPath.toUtf8().constData();
        const char* backup_c = backupPath.toUtf8().constData();
        return rust_restore_backup(config_c, backup_c);
    }

    /**
     * 获取备份列表
     */
    static QVector<QString> getBackups(const QString& configDir) {
        QVector<QString> result;
        const char* dir_c = configDir.toUtf8().constData();
        BackupList backups = rust_get_backups(dir_c);

        if (backups.paths && backups.count > 0) {
            for (int i = 0; i < backups.count; ++i) {
                result.append(QString::fromUtf8(backups.paths[i]));
            }
            rust_free_backup_list(backups);
        }

        return result;
    }

    /**
     * 解析命令
     */
    static QVector<QString> parseCommand(const QString& program, const QString& parameters) {
        QVector<QString> result;
        const char* prog_c = program.toUtf8().constData();
        const char* params_c = parameters.isEmpty() ? nullptr : parameters.toUtf8().constData();

        ParsedCommand parsed = rust_parse_command(prog_c, params_c);

        if (parsed.commands && parsed.count > 0) {
            for (int i = 0; i < parsed.count; ++i) {
                result.append(QString::fromUtf8(parsed.commands[i]));
            }
            rust_free_parsed_command(parsed);
        }

        return result;
    }

    /**
     * 初始化日志
     */
    static void initLogger() {
        rust_init_logger();
    }

    /**
     * 记录日志
     */
    static void log(const QString& level, const QString& message) {
        const char* level_c = level.toUtf8().constData();
        const char* msg_c = message.toUtf8().constData();
        rust_log(level_c, msg_c);
    }

    /**
     * 释放命令结果
     */
    static void freeCommandResult(CommandResult& result) {
        rust_free_command_result(result);
    }
};
