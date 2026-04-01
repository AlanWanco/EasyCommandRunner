#pragma once

#include <QMainWindow>
#include <QTabWidget>
#include <QPushButton>
#include <QLineEdit>
#include <QCheckBox>
#include <QTextEdit>
#include <QComboBox>
#include <QLabel>
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QSystemTrayIcon>
#include <QMenu>
#include <QString>
#include <QMap>
#include <QVector>
#include <QSettings>
#include <QThread>
#include <QTimer>
#include <QDialog>

/**
 * @brief 主应用窗口
 * 
 * EasyCommandRunner 主窗口，包含：
 * - 多标签页界面
 * - 命令编辑和参数管理
 * - 实时命令预览
 * - 系统托盘支持
 * - 日间/夜间主题切换
 */
class AppWindow : public QMainWindow {
    Q_OBJECT

public:
    explicit AppWindow(QWidget *parent = nullptr);
    ~AppWindow() override;

protected:
    void closeEvent(QCloseEvent *event) override;
    void changeEvent(QEvent *event) override;

private slots:
    // 主窗口操作
    void onAddTabClicked();
    void onCloseTab(int index);
    void onTabChanged(int index);
    void onTabMoved(int from, int to);

    // 命令操作
    void onRunButtonClicked();
    void onSaveButtonClicked();
    void onReloadConfigClicked();
    void onCopyTabConfigClicked();
    void onPreviewCommandClicked();

    // 设置相关
    void onSettingsClicked();
    void onThemeChanged(const QString &theme);
    void onAboutClicked();

    // 系统托盘相关
    void onTrayIconActivated(QSystemTrayIcon::ActivationReason reason);
    void onMinimizeToTray();
    void onRestoreFromTray();
    void onExitClicked();

    // 快捷键
    void onNextTab();
    void onPreviousTab();

    // 运行输出相关
    void onCommandExecuted(const QString &output);
    void onCommandError(const QString &error);

private:
    // 初始化UI
    void setupUI();
    void setupMenu();
    void setupTrayIcon();
    void setupConnections();
    void loadStylesheet(const QString &theme);
    void applyTheme(const QString &theme);
    
    // 图标管理
    QIcon createThemedIcon(const QString &svgPath, const QString &theme);
    void updateButtonIcons();

    // 配置管理
    void loadConfiguration();
    void saveConfiguration();
    void loadApplicationSettings();
    void saveApplicationSettings();

    // 其他
    void createTab(const QString &name = "");
    void updateTabCombo();

    // UI 组件
    QTabWidget *tabWidget;
    QComboBox *tabCombo;
    
    // 按钮
    QPushButton *addTabButton;
    QPushButton *saveButton;
    QPushButton *runButton;
    QPushButton *reloadButton;
    QPushButton *copyTabButton;
    QPushButton *settingsButton;
    QPushButton *previewButton;

    // 菜单和托盘
    QSystemTrayIcon *trayIcon;
    QMenu *trayMenu;
    QAction *minimizeAction;
    QAction *restoreAction;
    QAction *exitAction;
    QAction *settingsAction;
    QAction *themeAction;

    // 设置
    QSettings *settings;
    QString currentTheme;

    // 窗口几何信息保存
    QByteArray windowGeometry;
    int currentTabIndex;
};

/**
 * @brief 单个标签页widget
 */
class CommandTab : public QWidget {
    Q_OBJECT

public:
    explicit CommandTab(QWidget *parent = nullptr);

    // 获取/设置配置
    QString getTabName() const;
    void setTabName(const QString &name);

    QString getWorkingDirectory() const;
    QString getProgram() const;
    QVector<QPair<QString, QString>> getFunctions() const;
    QString getOtherArgs() const;
    QString getDescription() const;

    void loadConfiguration(const QJsonObject &config);
    QJsonObject saveConfiguration() const;

    void clear();

public slots:
    void onParseCommandClicked();
    void onAddFunctionClicked();
    void onRemoveFunctionClicked();
    void onPreviewCommandClicked();

private:
    void setupUI();
    void setupConnections();
    void updateCommandPreview();

    // 标题区域
    QLineEdit *titleEdit;
    
    // 运行路径
    QLineEdit *workingDirEdit;
    
    // 程序名称
    QLineEdit *programEdit;
    QPushButton *parseButton;
    
    // 函数/参数行（动态创建）
    QVBoxLayout *functionsLayout;
    QVector<QLineEdit*> functionEdits;
    QVector<QLineEdit*> parameterEdits;
    QVector<QLineEdit*> commentEdits;
    QVector<QPushButton*> removeButtons;
    
    // 其他参数
    QLineEdit *otherArgsEdit;
    
    // 描述
    QTextEdit *descriptionEdit;
    
    // 命令预览
    QTextEdit *commandPreviewEdit;
    QPushButton *addFunctionButton;
    QPushButton *previewButton;
    
    // 全选/取消全选按钮
    QPushButton *selectAllButton;
    QPushButton *deselectAllButton;
    
    int functionCounter;
};

/**
 * @brief 设置对话框
 */
class SettingsDialog : public QDialog {
    Q_OBJECT

public:
    explicit SettingsDialog(QWidget *parent = nullptr);

signals:
    void themeChanged(const QString &theme);
    void languageChanged(const QString &language);

private slots:
    void onThemeComboChanged(int index);
    void onRestoreBackupClicked();
    void onOkClicked();
    void onCancelClicked();
    void onApplyClicked();

private:
    void setupUI();
    void loadSettings();

    // UI 组件
    QComboBox *themeCombo;
    QComboBox *languageCombo;
    QComboBox *backupCombo;
    QPushButton *restoreButton;
    QPushButton *okButton;
    QPushButton *cancelButton;
    QPushButton *applyButton;

    QString currentTheme;
};

/**
 * @brief 运行输出窗口（替代 cmd/terminal）
 */
class OutputWindow : public QWidget {
    Q_OBJECT

public:
    explicit OutputWindow(QWidget *parent = nullptr);

    void setCommand(const QString &cmd);
    void appendOutput(const QString &text);
    void clearOutput();

public slots:
    void onExecuteClicked();
    void onClearClicked();
    void onStopClicked();
    void onCopyClicked();
    void onSaveClicked();

private:
    void setupUI();
    void setupConnections();

    // UI 组件
    QLineEdit *commandEdit;
    QTextEdit *outputEdit;
    QPushButton *executeButton;
    QPushButton *clearButton;
    QPushButton *stopButton;
    QPushButton *copyButton;
    QPushButton *saveButton;
    QLabel *statusLabel;

    QString currentCommand;
    bool isRunning;
};
