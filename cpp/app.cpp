#include "app.h"
#include <QApplication>
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QSplitter>
#include <QScrollArea>
#include <QGroupBox>
#include <QMessageBox>
#include <QFileDialog>
#include <QClipboard>
#include <QShortcut>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonArray>
#include <QFile>
#include <QDesktopServices>
#include <QUrl>
#include <QScreen>
#include <QStandardPaths>
#include <QThread>
#include <QDebug>
#include <QMenuBar>
#include <QCloseEvent>
#include <QEvent>

// ============================================================================
// AppWindow Implementation
// ============================================================================

AppWindow::AppWindow(QWidget *parent)
    : QMainWindow(parent)
    , tabWidget(nullptr)
    , tabCombo(nullptr)
    , addTabButton(nullptr)
    , saveButton(nullptr)
    , runButton(nullptr)
    , trayIcon(nullptr)
    , trayMenu(nullptr)
    , settings(nullptr)
    , currentTheme("dark")
    , currentTabIndex(0)
{
    setWindowTitle("EasyCommandRunner");
    setWindowIcon(QIcon(":/res/icon2.ico"));

    // 应用设置
    settings = new QSettings("SleepyKanata", "EasyCommandRunner", this);

    // 初始化UI
    setupUI();
    setupMenu();
    setupTrayIcon();
    setupConnections();

    // 加载配置
    loadApplicationSettings();
    loadConfiguration();
    applyTheme(currentTheme);

    // 窗口大小和位置
    if (settings->contains("window/geometry")) {
        restoreGeometry(settings->value("window/geometry").toByteArray());
    } else {
        resize(850, 700);
        move(QApplication::primaryScreen()->availableGeometry().center() - frameGeometry().center());
    }

    show();
}

AppWindow::~AppWindow() {
    saveApplicationSettings();
    saveConfiguration();
}

void AppWindow::setupUI() {
    QWidget *centralWidget = new QWidget(this);
    QVBoxLayout *mainLayout = new QVBoxLayout(centralWidget);
    mainLayout->setContentsMargins(0, 0, 0, 0);

    // 创建标签页widget
    tabWidget = new QTabWidget(this);
    tabWidget->setTabsClosable(true);
    tabWidget->setMovable(true);
    tabWidget->setDocumentMode(false);

    // 下拉菜单选择标签
    tabCombo = new QComboBox(this);
    tabWidget->setCornerWidget(tabCombo, Qt::TopRightCorner);
    connect(tabCombo, QOverload<int>::of(&QComboBox::activated), tabWidget, &QTabWidget::setCurrentIndex);

    mainLayout->addWidget(tabWidget);

    // 按钮布局
    QHBoxLayout *buttonLayout = new QHBoxLayout();

    addTabButton = new QPushButton("增加标签", this);
    addTabButton->setObjectName("addTabBtn");

    reloadButton = new QPushButton("重新加载", this);
    reloadButton->setObjectName("reloadBtn");

    copyTabButton = new QPushButton("复制标签", this);
    copyTabButton->setObjectName("copyTabBtn");

    previewButton = new QPushButton("预生成命令", this);
    previewButton->setObjectName("previewBtn");

    runButton = new QPushButton("运行(Ctrl+Enter)", this);
    runButton->setObjectName("runBtn");

    saveButton = new QPushButton("保存(Ctrl+S)", this);
    saveButton->setObjectName("saveBtn");

    settingsButton = new QPushButton("设置", this);
    settingsButton->setObjectName("settingsBtn");

    buttonLayout->addWidget(addTabButton);
    buttonLayout->addWidget(reloadButton);
    buttonLayout->addWidget(copyTabButton);
    buttonLayout->addStretch();
    buttonLayout->addWidget(previewButton);
    buttonLayout->addWidget(runButton);
    buttonLayout->addWidget(saveButton);
    buttonLayout->addWidget(settingsButton);

    mainLayout->addLayout(buttonLayout);

    setCentralWidget(centralWidget);
}

void AppWindow::setupMenu() {
    QMenuBar *menuBar = new QMenuBar(this);
    setMenuBar(menuBar);

    // 文件菜单
    QMenu *fileMenu = menuBar->addMenu("文件(&F)");
    QAction *newTabAction = fileMenu->addAction("新建标签页(&N)");
    newTabAction->setShortcut(Qt::CTRL | Qt::Key_T);
    connect(newTabAction, &QAction::triggered, this, &AppWindow::onAddTabClicked);

    fileMenu->addSeparator();

    QAction *saveAction = fileMenu->addAction("保存(&S)");
    saveAction->setShortcut(Qt::CTRL | Qt::Key_S);
    connect(saveAction, &QAction::triggered, this, &AppWindow::onSaveButtonClicked);

    QAction *reloadAction = fileMenu->addAction("重新加载(&R)");
    reloadAction->setShortcut(Qt::CTRL | Qt::Key_L);
    connect(reloadAction, &QAction::triggered, this, &AppWindow::onReloadConfigClicked);

    fileMenu->addSeparator();

    QAction *exitAction = fileMenu->addAction("退出(&Q)");
    exitAction->setShortcut(Qt::CTRL | Qt::Key_Q);
    connect(exitAction, &QAction::triggered, this, &AppWindow::onExitClicked);

    // 编辑菜单
    QMenu *editMenu = menuBar->addMenu("编辑(&E)");

    QAction *selectAllAction = editMenu->addAction("全选(&A)");
    selectAllAction->setShortcut(Qt::CTRL | Qt::Key_A);
    connect(selectAllAction, &QAction::triggered, [this]() {
        CommandTab *tab = qobject_cast<CommandTab*>(tabWidget->currentWidget());
        if (tab) {
            // 在标签页中实现全选
        }
    });

    // 视图菜单
    QMenu *viewMenu = menuBar->addMenu("视图(&V)");

    QAction *nextTabAction = viewMenu->addAction("下一个标签页(&N)");
    nextTabAction->setShortcut(Qt::CTRL | Qt::Key_Right);
    connect(nextTabAction, &QAction::triggered, this, &AppWindow::onNextTab);

    QAction *prevTabAction = viewMenu->addAction("上一个标签页(&P)");
    prevTabAction->setShortcut(Qt::CTRL | Qt::Key_Left);
    connect(prevTabAction, &QAction::triggered, this, &AppWindow::onPreviousTab);

    // 工具菜单
    QMenu *toolsMenu = menuBar->addMenu("工具(&T)");

    QAction *settingsAction = toolsMenu->addAction("设置(&S)");
    connect(settingsAction, &QAction::triggered, this, &AppWindow::onSettingsClicked);

    // 帮助菜单
    QMenu *helpMenu = menuBar->addMenu("帮助(&H)");

    QAction *aboutAction = helpMenu->addAction("关于(&A)");
    connect(aboutAction, &QAction::triggered, this, &AppWindow::onAboutClicked);
}

void AppWindow::setupTrayIcon() {
    trayIcon = new QSystemTrayIcon(this);
    trayIcon->setIcon(QIcon(":/res/icon2.ico"));
    trayIcon->setToolTip("EasyCommandRunner");

    trayMenu = new QMenu(this);

    minimizeAction = trayMenu->addAction("最小化");
    connect(minimizeAction, &QAction::triggered, this, &AppWindow::onMinimizeToTray);

    restoreAction = trayMenu->addAction("还原");
    connect(restoreAction, &QAction::triggered, this, &AppWindow::onRestoreFromTray);

    trayMenu->addSeparator();

    settingsAction = trayMenu->addAction("设置");
    connect(settingsAction, &QAction::triggered, this, &AppWindow::onSettingsClicked);

    trayMenu->addSeparator();

    exitAction = trayMenu->addAction("退出");
    connect(exitAction, &QAction::triggered, this, &AppWindow::onExitClicked);

    trayIcon->setContextMenu(trayMenu);
    trayIcon->show();

    connect(trayIcon, &QSystemTrayIcon::activated, this, &AppWindow::onTrayIconActivated);
}

void AppWindow::setupConnections() {
    connect(tabWidget, &QTabWidget::tabCloseRequested, this, &AppWindow::onCloseTab);
    connect(tabWidget, &QTabWidget::currentChanged, this, &AppWindow::onTabChanged);
    connect(tabWidget->tabBar(), &QTabBar::tabMoved, this, &AppWindow::onTabMoved);

    connect(addTabButton, &QPushButton::clicked, this, &AppWindow::onAddTabClicked);
    connect(saveButton, &QPushButton::clicked, this, &AppWindow::onSaveButtonClicked);
    connect(runButton, &QPushButton::clicked, this, &AppWindow::onRunButtonClicked);
    connect(reloadButton, &QPushButton::clicked, this, &AppWindow::onReloadConfigClicked);
    connect(copyTabButton, &QPushButton::clicked, this, &AppWindow::onCopyTabConfigClicked);
    connect(previewButton, &QPushButton::clicked, this, &AppWindow::onPreviewCommandClicked);
    connect(settingsButton, &QPushButton::clicked, this, &AppWindow::onSettingsClicked);

    // 快捷键
    new QShortcut(Qt::CTRL | Qt::Key_Return, this, SLOT(onRunButtonClicked()));
}

void AppWindow::closeEvent(QCloseEvent *event) {
    saveApplicationSettings();
    saveConfiguration();
    event->ignore();
    hide();
}

void AppWindow::changeEvent(QEvent *event) {
    if (event->type() == QEvent::WindowStateChange) {
        if (windowState() & Qt::WindowMinimized) {
            onMinimizeToTray();
        }
    }
    QMainWindow::changeEvent(event);
}

// ============================================================================
// Slots Implementation
// ============================================================================

void AppWindow::onAddTabClicked() {
    createTab(QString("标签%1").arg(tabWidget->count() + 1));
}

void AppWindow::onCloseTab(int index) {
    if (tabWidget->count() > 1) {
        QWidget *widget = tabWidget->widget(index);
        tabWidget->removeTab(index);
        delete widget;
        updateTabCombo();
    } else {
        QMessageBox::warning(this, "提示", "至少保留一个标签页");
    }
}

void AppWindow::onTabChanged(int index) {
    currentTabIndex = index;
    tabCombo->setCurrentIndex(index);
}

void AppWindow::onTabMoved(int from, int to) {
    updateTabCombo();
}

void AppWindow::onRunButtonClicked() {
    CommandTab *tab = qobject_cast<CommandTab*>(tabWidget->currentWidget());
    if (!tab) return;

    QString program = tab->getProgram();
    if (program.isEmpty()) {
        QMessageBox::warning(this, "提示", "请输入程序名称");
        return;
    }

    // 这里调用 Rust 函数执行命令
    // rust_execute_command(...)
}

void AppWindow::onSaveButtonClicked() {
    saveConfiguration();
    QMessageBox::information(this, "成功", "配置已保存！");
}

void AppWindow::onReloadConfigClicked() {
    loadConfiguration();
    QMessageBox::information(this, "成功", "配置已重新加载！");
}

void AppWindow::onCopyTabConfigClicked() {
    // 实现复制当前标签页配置到新标签页
}

void AppWindow::onPreviewCommandClicked() {
    CommandTab *tab = qobject_cast<CommandTab*>(tabWidget->currentWidget());
    if (tab) {
        tab->onPreviewCommandClicked();
    }
}

void AppWindow::onSettingsClicked() {
    SettingsDialog dialog(this);
    if (dialog.exec() == QDialog::Accepted) {
        saveApplicationSettings();
    }
}

void AppWindow::onThemeChanged(const QString &theme) {
    currentTheme = theme;
    applyTheme(theme);
    settings->setValue("theme", theme);
}

void AppWindow::onAboutClicked() {
    QMessageBox::information(this, "关于",
        "EasyCommandRunner v1.0.0\n\n"
        "现代化的跨平台命令运行器\n\n"
        "© 2024 SleepyKanata");
}

void AppWindow::onTrayIconActivated(QSystemTrayIcon::ActivationReason reason) {
    if (reason == QSystemTrayIcon::DoubleClick) {
        onRestoreFromTray();
    }
}

void AppWindow::onMinimizeToTray() {
    hide();
    minimizeAction->setText("还原");
    restoreAction->setText("显示");
}

void AppWindow::onRestoreFromTray() {
    showNormal();
    activateWindow();
}

void AppWindow::onExitClicked() {
    QApplication::quit();
}

void AppWindow::onNextTab() {
    int index = tabWidget->currentIndex();
    int count = tabWidget->count();
    tabWidget->setCurrentIndex((index + 1) % count);
}

void AppWindow::onPreviousTab() {
    int index = tabWidget->currentIndex();
    int count = tabWidget->count();
    tabWidget->setCurrentIndex((index - 1 + count) % count);
}

void AppWindow::onCommandExecuted(const QString &output) {
    // 显示运行结果
    QMessageBox::information(this, "运行结果", output);
}

void AppWindow::onCommandError(const QString &error) {
    QMessageBox::critical(this, "错误", error);
}

// ============================================================================
// Private Methods
// ============================================================================

void AppWindow::loadStylesheet(const QString &theme) {
    QString filename = (theme == "light") 
        ? ":/res/stylesheet_light.qss"
        : ":/res/stylesheet_dark.qss";

    QFile file(filename);
    if (file.open(QFile::ReadOnly)) {
        QString stylesheet = QLatin1String(file.readAll());
        qApp->setStyleSheet(stylesheet);
        file.close();
    }
}

void AppWindow::applyTheme(const QString &theme) {
    currentTheme = theme;
    loadStylesheet(theme);
    settings->setValue("ui/theme", theme);
}

void AppWindow::loadConfiguration() {
    // 从 Rust 加载配置
    // QString configJson = rust_load_config();
    // 解析并加载到 UI
}

void AppWindow::saveConfiguration() {
    // 收集 UI 数据并保存
    QJsonObject config;
    config["theme"] = currentTheme;
    config["window_index"] = currentTabIndex;

    // 保存所有标签页配置
    QJsonArray tabsArray;
    for (int i = 0; i < tabWidget->count(); ++i) {
        CommandTab *tab = qobject_cast<CommandTab*>(tabWidget->widget(i));
        if (tab) {
            tabsArray.append(tab->saveConfiguration());
        }
    }
    config["tabs"] = tabsArray;

    // 调用 Rust 保存
    // rust_save_config(QString(QJsonDocument(config).toJson()).toStdString().c_str());
}

void AppWindow::loadApplicationSettings() {
    if (settings->contains("ui/theme")) {
        currentTheme = settings->value("ui/theme", "dark").toString();
    }
    if (settings->contains("window/geometry")) {
        windowGeometry = settings->value("window/geometry").toByteArray();
    }
    if (settings->contains("window/index")) {
        currentTabIndex = settings->value("window/index", 0).toInt();
    }
}

void AppWindow::saveApplicationSettings() {
    settings->setValue("window/geometry", saveGeometry());
    settings->setValue("window/index", tabWidget->currentIndex());
    settings->setValue("ui/theme", currentTheme);
}

void AppWindow::createTab(const QString &name) {
    CommandTab *tab = new CommandTab(this);
    int index = tabWidget->addTab(tab, name);
    tabWidget->setCurrentIndex(index);
    updateTabCombo();
}

void AppWindow::updateTabCombo() {
    tabCombo->clear();
    for (int i = 0; i < tabWidget->count(); ++i) {
        tabCombo->addItem(tabWidget->tabText(i));
    }
}

// ============================================================================
// CommandTab Implementation
// ============================================================================

CommandTab::CommandTab(QWidget *parent)
    : QWidget(parent)
    , titleEdit(nullptr)
    , workingDirEdit(nullptr)
    , programEdit(nullptr)
    , parseButton(nullptr)
    , otherArgsEdit(nullptr)
    , useNewWindowCheckbox(nullptr)
    , descriptionEdit(nullptr)
    , commandPreviewEdit(nullptr)
    , addFunctionButton(nullptr)
    , previewButton(nullptr)
    , selectAllButton(nullptr)
    , deselectAllButton(nullptr)
    , functionCounter(1)
{
    setupUI();
    setupConnections();
}

QString CommandTab::getTabName() const {
    return titleEdit->text();
}

void CommandTab::setTabName(const QString &name) {
    titleEdit->setText(name);
}

QString CommandTab::getWorkingDirectory() const {
    return workingDirEdit->text();
}

QString CommandTab::getProgram() const {
    return programEdit->text();
}

QVector<QPair<QString, QString>> CommandTab::getFunctions() const {
    QVector<QPair<QString, QString>> result;
    for (int i = 0; i < functionEdits.size(); ++i) {
        result.append({functionEdits[i]->text(), parameterEdits[i]->text()});
    }
    return result;
}

QString CommandTab::getOtherArgs() const {
    return otherArgsEdit->text();
}

QString CommandTab::getDescription() const {
    return descriptionEdit->toPlainText();
}

bool CommandTab::getUseNewWindow() const {
    return useNewWindowCheckbox->isChecked();
}

void CommandTab::loadConfiguration(const QJsonObject &config) {
    titleEdit->setText(config.value("name").toString());
    workingDirEdit->setText(config.value("working_dir").toString());
    programEdit->setText(config.value("program").toString());
    otherArgsEdit->setText(config.value("other_args").toString());
    descriptionEdit->setText(config.value("description").toString());
    useNewWindowCheckbox->setChecked(config.value("use_new_window").toBool());

    // 加载函数列表
    QJsonArray functions = config.value("functions").toArray();
    for (const auto &func : functions) {
        QJsonObject funcObj = func.toObject();
        QLineEdit *funcEdit = new QLineEdit(funcObj.value("function").toString());
        QLineEdit *paramEdit = new QLineEdit(funcObj.value("parameter").toString());
        functionEdits.append(funcEdit);
        parameterEdits.append(paramEdit);
    }
}

QJsonObject CommandTab::saveConfiguration() const {
    QJsonObject config;
    config.insert("name", titleEdit->text());
    config.insert("working_dir", workingDirEdit->text());
    config.insert("program", programEdit->text());
    config.insert("other_args", otherArgsEdit->text());
    config.insert("description", descriptionEdit->toPlainText());
    config.insert("use_new_window", useNewWindowCheckbox->isChecked());

    QJsonArray functions;
    for (int i = 0; i < functionEdits.size(); ++i) {
        QJsonObject func;
        func.insert("function", functionEdits[i]->text());
        func.insert("parameter", parameterEdits[i]->text());
        functions.append(func);
    }
    config.insert("functions", functions);

    return config;
}

void CommandTab::clear() {
    titleEdit->clear();
    workingDirEdit->clear();
    programEdit->clear();
    otherArgsEdit->clear();
    descriptionEdit->clear();
    useNewWindowCheckbox->setChecked(false);
    functionEdits.clear();
    parameterEdits.clear();
    commentEdits.clear();
}

void CommandTab::onParseCommandClicked() {
    // 实现命令解析
}

void CommandTab::onAddFunctionClicked() {
    // 实现添加函数行
}

void CommandTab::onRemoveFunctionClicked() {
    // 实现删除函数行
}

void CommandTab::onPreviewCommandClicked() {
    updateCommandPreview();
}

void CommandTab::setupUI() {
    QVBoxLayout *layout = new QVBoxLayout(this);

    // 标题
    QHBoxLayout *titleLayout = new QHBoxLayout();
    titleLayout->addWidget(new QLabel("标题:"));
    titleEdit = new QLineEdit();
    titleLayout->addWidget(titleEdit);
    layout->addLayout(titleLayout);

    // 工作目录
    QHBoxLayout *dirLayout = new QHBoxLayout();
    dirLayout->addWidget(new QLabel("运行路径:"));
    workingDirEdit = new QLineEdit();
    workingDirEdit->setPlaceholderText("为空则使用程序当前目录");
    dirLayout->addWidget(workingDirEdit);
    layout->addLayout(dirLayout);

    // 程序
    QHBoxLayout *programLayout = new QHBoxLayout();
    programLayout->addWidget(new QLabel("程序本体:"));
    programEdit = new QLineEdit();
    programEdit->setPlaceholderText("粘贴命令可以解析");
    programLayout->addWidget(programEdit);
    parseButton = new QPushButton("解析");
    programLayout->addWidget(parseButton);
    layout->addLayout(programLayout);

    // 函数/参数区域（使用滚动区）
    QScrollArea *scrollArea = new QScrollArea();
    QWidget *scrollWidget = new QWidget();
    functionsLayout = new QVBoxLayout(scrollWidget);

    // 默认添加一行
    onAddFunctionClicked();

    scrollArea->setWidget(scrollWidget);
    scrollArea->setWidgetResizable(true);
    layout->addWidget(scrollArea);

    // 其他参数和新窗口checkbox
    QHBoxLayout *otherLayout = new QHBoxLayout();
    otherArgsEdit = new QLineEdit();
    otherArgsEdit->setPlaceholderText("其他参数");
    otherLayout->addWidget(otherArgsEdit);
    useNewWindowCheckbox = new QCheckBox("使用新窗口运行");
    otherLayout->addWidget(useNewWindowCheckbox);
    layout->addLayout(otherLayout);

    // 描述
    descriptionEdit = new QTextEdit();
    descriptionEdit->setPlaceholderText("描述");
    layout->addWidget(descriptionEdit);

    // 命令预览
    commandPreviewEdit = new QTextEdit();
    commandPreviewEdit->setReadOnly(true);
    commandPreviewEdit->setPlaceholderText("预生成命令");
    commandPreviewEdit->setMaximumHeight(100);
    layout->addWidget(commandPreviewEdit);

    // 按钮
    QHBoxLayout *buttonLayout = new QHBoxLayout();

    selectAllButton = new QPushButton("全选");
    deselectAllButton = new QPushButton("取消全选");
    addFunctionButton = new QPushButton("增加参数行");
    previewButton = new QPushButton("预生成命令");

    buttonLayout->addWidget(selectAllButton);
    buttonLayout->addWidget(deselectAllButton);
    buttonLayout->addWidget(addFunctionButton);
    buttonLayout->addStretch();
    buttonLayout->addWidget(previewButton);

    layout->addLayout(buttonLayout);
}

void CommandTab::setupConnections() {
    connect(parseButton, &QPushButton::clicked, this, &CommandTab::onParseCommandClicked);
    connect(addFunctionButton, &QPushButton::clicked, this, &CommandTab::onAddFunctionClicked);
    connect(previewButton, &QPushButton::clicked, this, &CommandTab::onPreviewCommandClicked);
}

void CommandTab::updateCommandPreview() {
    // 调用 Rust 函数构建命令预览
    QString program = programEdit->text();
    // QString preview = rust_build_command(...);
    // commandPreviewEdit->setText(preview);
}

// ============================================================================
// SettingsDialog Implementation
// ============================================================================

SettingsDialog::SettingsDialog(QWidget *parent)
    : QDialog(parent)
{
    setWindowTitle("设置");
    setModal(true);
    setupUI();
    loadSettings();
}

void SettingsDialog::setupUI() {
    QVBoxLayout *layout = new QVBoxLayout(this);

    // 主题设置
    QGroupBox *themeGroup = new QGroupBox("外观", this);
    QHBoxLayout *themeLayout = new QHBoxLayout(themeGroup);
    themeLayout->addWidget(new QLabel("主题:"));
    themeCombo = new QComboBox();
    themeCombo->addItem("深色", "dark");
    themeCombo->addItem("浅色", "light");
    themeLayout->addWidget(themeCombo);
    layout->addWidget(themeGroup);

    // 语言设置
    QGroupBox *langGroup = new QGroupBox("语言", this);
    QHBoxLayout *langLayout = new QHBoxLayout(langGroup);
    langLayout->addWidget(new QLabel("语言:"));
    languageCombo = new QComboBox();
    languageCombo->addItem("中文", "zh_CN");
    languageCombo->addItem("English", "en_US");
    langLayout->addWidget(languageCombo);
    layout->addWidget(langGroup);

    // 备份恢复
    QGroupBox *backupGroup = new QGroupBox("备份", this);
    QVBoxLayout *backupLayout = new QVBoxLayout(backupGroup);
    QHBoxLayout *backupSelectLayout = new QHBoxLayout();
    backupSelectLayout->addWidget(new QLabel("选择备份:"));
    backupCombo = new QComboBox();
    backupSelectLayout->addWidget(backupCombo);
    restoreButton = new QPushButton("恢复");
    backupSelectLayout->addWidget(restoreButton);
    backupLayout->addLayout(backupSelectLayout);
    layout->addWidget(backupGroup);

    layout->addStretch();

    // 按钮
    QHBoxLayout *buttonLayout = new QHBoxLayout();
    okButton = new QPushButton("确定");
    cancelButton = new QPushButton("取消");
    applyButton = new QPushButton("应用");

    buttonLayout->addStretch();
    buttonLayout->addWidget(okButton);
    buttonLayout->addWidget(cancelButton);
    buttonLayout->addWidget(applyButton);

    layout->addLayout(buttonLayout);

    // 连接信号
    connect(okButton, &QPushButton::clicked, this, &QDialog::accept);
    connect(cancelButton, &QPushButton::clicked, this, &QDialog::reject);
    connect(applyButton, &QPushButton::clicked, this, &SettingsDialog::onApplyClicked);
    connect(themeCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), this, &SettingsDialog::onThemeComboChanged);
    connect(restoreButton, &QPushButton::clicked, this, &SettingsDialog::onRestoreBackupClicked);

    resize(400, 300);
}

void SettingsDialog::loadSettings() {
    // 从设置中加载当前值
}

void SettingsDialog::onThemeComboChanged(int index) {
    // 主题改变
}

void SettingsDialog::onRestoreBackupClicked() {
    // 恢复备份
}

void SettingsDialog::onOkClicked() {
    accept();
}

void SettingsDialog::onCancelClicked() {
    reject();
}

void SettingsDialog::onApplyClicked() {
    // 应用设置
}

// ============================================================================
// OutputWindow Implementation
// ============================================================================

OutputWindow::OutputWindow(QWidget *parent)
    : QWidget(parent)
    , commandEdit(nullptr)
    , outputEdit(nullptr)
    , executeButton(nullptr)
    , clearButton(nullptr)
    , stopButton(nullptr)
    , copyButton(nullptr)
    , saveButton(nullptr)
    , statusLabel(nullptr)
    , isRunning(false)
{
    setWindowTitle("运行输出 - EasyCommandRunner");
    setupUI();
    setupConnections();
    resize(800, 600);
}

void OutputWindow::setCommand(const QString &cmd) {
    currentCommand = cmd;
    commandEdit->setText(cmd);
}

void OutputWindow::appendOutput(const QString &text) {
    outputEdit->append(text);
}

void OutputWindow::clearOutput() {
    outputEdit->clear();
}

void OutputWindow::onExecuteClicked() {
    // 执行命令
    isRunning = true;
    executeButton->setEnabled(false);
    stopButton->setEnabled(true);
    statusLabel->setText("运行中...");
}

void OutputWindow::onClearClicked() {
    clearOutput();
}

void OutputWindow::onStopClicked() {
    // 停止命令
    isRunning = false;
    executeButton->setEnabled(true);
    stopButton->setEnabled(false);
    statusLabel->setText("已停止");
}

void OutputWindow::onCopyClicked() {
    QApplication::clipboard()->setText(outputEdit->toPlainText());
}

void OutputWindow::onSaveClicked() {
    QString fileName = QFileDialog::getSaveFileName(this, "保存输出", "", "Text Files (*.txt)");
    if (!fileName.isEmpty()) {
        QFile file(fileName);
        if (file.open(QIODevice::WriteOnly | QIODevice::Text)) {
            file.write(outputEdit->toPlainText().toUtf8());
            file.close();
        }
    }
}

void OutputWindow::setupUI() {
    QVBoxLayout *layout = new QVBoxLayout(this);

    // 命令行
    QHBoxLayout *commandLayout = new QHBoxLayout();
    commandLayout->addWidget(new QLabel("命令:"));
    commandEdit = new QLineEdit();
    commandEdit->setReadOnly(true);
    commandLayout->addWidget(commandEdit);
    layout->addLayout(commandLayout);

    // 输出区域
    outputEdit = new QTextEdit();
    outputEdit->setReadOnly(true);
    layout->addWidget(outputEdit);

    // 状态栏
    QHBoxLayout *statusLayout = new QHBoxLayout();
    statusLabel = new QLabel("就绪");
    statusLayout->addWidget(statusLabel);
    statusLayout->addStretch();
    layout->addLayout(statusLayout);

    // 按钮
    QHBoxLayout *buttonLayout = new QHBoxLayout();

    executeButton = new QPushButton("执行");
    clearButton = new QPushButton("清除");
    stopButton = new QPushButton("停止");
    stopButton->setEnabled(false);
    copyButton = new QPushButton("复制");
    saveButton = new QPushButton("保存");

    buttonLayout->addWidget(executeButton);
    buttonLayout->addWidget(stopButton);
    buttonLayout->addWidget(clearButton);
    buttonLayout->addStretch();
    buttonLayout->addWidget(copyButton);
    buttonLayout->addWidget(saveButton);

    layout->addLayout(buttonLayout);
}

void OutputWindow::setupConnections() {
    connect(executeButton, &QPushButton::clicked, this, &OutputWindow::onExecuteClicked);
    connect(clearButton, &QPushButton::clicked, this, &OutputWindow::onClearClicked);
    connect(stopButton, &QPushButton::clicked, this, &OutputWindow::onStopClicked);
    connect(copyButton, &QPushButton::clicked, this, &OutputWindow::onCopyClicked);
    connect(saveButton, &QPushButton::clicked, this, &OutputWindow::onSaveClicked);
}

// ============================================================================
// main() function
// ============================================================================

int main(int argc, char *argv[])
{
    QApplication app(argc, argv);
    app.setApplicationName("EasyCommandRunner");
    app.setApplicationVersion("1.0.0");

    AppWindow window;
    return app.exec();
}
