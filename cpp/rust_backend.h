#include "ffi_bindings.h"
#include <QString>
#include <QByteArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonArray>
#include <QPair>
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
        const QString& commandStr)
    {
        const QByteArray programUtf8 = program.toUtf8();
        const QByteArray workingDirUtf8 = workingDir.toUtf8();
        const QByteArray commandUtf8 = commandStr.toUtf8();

        const char* prog_c = programUtf8.constData();
        const char* dir_c = workingDir.isEmpty() ? nullptr : workingDirUtf8.constData();
        const char* cmd_c = commandUtf8.constData();

        return rust_execute_command(
            prog_c,
            dir_c,
            cmd_c
        );
    }

    /**
     * 加载配置
     */
    static QString loadConfig(const QString& configPath, const QString& backupDir) {
        const QByteArray pathUtf8 = configPath.toUtf8();
        const QByteArray backupUtf8 = backupDir.toUtf8();
        const char* path_c = pathUtf8.constData();
        const char* backup_c = backupUtf8.constData();
        ConfigData config = rust_load_config(path_c, backup_c);

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
    static bool saveConfig(const QString& configPath, const QString& backupDir, const QString& jsonData) {
        const QByteArray pathUtf8 = configPath.toUtf8();
        const QByteArray backupUtf8 = backupDir.toUtf8();
        const QByteArray jsonUtf8 = jsonData.toUtf8();
        const char* path_c = pathUtf8.constData();
        const char* backup_c = backupUtf8.constData();
        const char* json_c = jsonUtf8.constData();
        return rust_save_config(path_c, backup_c, json_c);
    }

    /**
     * 创建备份
     */
    static bool createBackup(const QString& configPath, const QString& backupDir) {
        const QByteArray pathUtf8 = configPath.toUtf8();
        const QByteArray backupUtf8 = backupDir.toUtf8();
        const char* path_c = pathUtf8.constData();
        const char* backup_c = backupUtf8.constData();
        return rust_create_backup(path_c, backup_c);
    }

    /**
     * 恢复备份
     */
    static bool restoreBackup(const QString& configPath, const QString& backupDir, const QString& backupPath) {
        const QByteArray configUtf8 = configPath.toUtf8();
        const QByteArray backupDirUtf8 = backupDir.toUtf8();
        const QByteArray backupUtf8 = backupPath.toUtf8();
        const char* config_c = configUtf8.constData();
        const char* backup_dir_c = backupDirUtf8.constData();
        const char* backup_c = backupUtf8.constData();
        return rust_restore_backup(config_c, backup_dir_c, backup_c);
    }

    /**
     * 获取备份列表
     */
    static QVector<QString> getBackups(const QString& configPath, const QString& backupDir) {
        QVector<QString> result;
        const QByteArray configUtf8 = configPath.toUtf8();
        const QByteArray backupUtf8 = backupDir.toUtf8();
        const char* config_c = configUtf8.constData();
        const char* backup_c = backupUtf8.constData();
        BackupList backups = rust_get_backups(config_c, backup_c);

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
    static QVector<QString> parseCommand(const QString& input, bool append = false) {
        QVector<QString> result;
        const QByteArray inputUtf8 = input.toUtf8();
        ParsedCommand parsed = rust_parse_command(inputUtf8.constData(), append);

        if (parsed.commands && parsed.count > 0) {
            for (int i = 0; i < parsed.count; ++i) {
                result.append(QString::fromUtf8(parsed.commands[i]));
            }
            rust_free_parsed_command(parsed);
        }

        return result;
    }

    /**
     * 按目标平台 shell 规则构建命令。参数行在这里以结构化数据传给 Rust，
     * 避免 C++/Qt 和 Rust 各自实现一套引号、空格、反斜杠规则。
     */
    static QString buildCommand(const QString& program,
                                const QVector<QPair<QString, QString>>& functions,
                                const QVector<bool>& enabled,
                                const QString& otherArgs) {
        QJsonObject input;
        input.insert("program", program);
        QJsonArray arguments;
        for (int i = 0; i < functions.size(); ++i) {
            QJsonObject item;
            item.insert("function", functions[i].first);
            item.insert("parameter", functions[i].second);
            item.insert("enabled", i >= enabled.size() || enabled[i]);
            arguments.append(item);
        }
        input.insert("arguments", arguments);
        input.insert("other_args", otherArgs);
        const QByteArray json = QJsonDocument(input).toJson(QJsonDocument::Compact);
        StringData result = rust_build_command(json.constData());
        QString command;
        if (result.data) {
            command = QString::fromUtf8(result.data, result.length);
            rust_free_string_data(result);
        }
        return command;
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
        const QByteArray levelUtf8 = level.toUtf8();
        const QByteArray messageUtf8 = message.toUtf8();
        const char* level_c = levelUtf8.constData();
        const char* msg_c = messageUtf8.constData();
        rust_log(level_c, msg_c);
    }

    /**
     * 释放命令结果
     */
    static void freeCommandResult(CommandResult& result) {
        rust_free_command_result(result);
    }
};
