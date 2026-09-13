#include "app.h"
#include "log_panel.h"
#include "rust_backend.h"
#include "widget_helpers.h"
#include <QInputDialog>
#include <QStatusBar>
#include <QDateTime>
#include <QSignalBlocker>
#include <algorithm>
#include <QApplication>
#include <QVBoxLayout>
#include <QHBoxLayout>
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
#include <QEnterEvent>
#include <QPixmap>
#include <QPainter>
#include <QSize>
#include <QAbstractButton>
#include <QTabBar>
#include <QListView>
#include <QStyleFactory>
#include <QDir>

namespace {

QString dataDirPath()
{
    return qEnvironmentVariable("ECR_DATA_DIR", QCoreApplication::applicationDirPath());
}

QString configFilePath()
{
    return dataDirPath() + "/config.json";
}

QString backupDirPath()
{
    return dataDirPath() + "/backup";
}

QString platformFontFamily()
{
#if defined(Q_OS_MACOS)
    return QStringLiteral("\".AppleSystemUIFont\", Helvetica, Arial");
#elif defined(Q_OS_WIN)
    return QStringLiteral("\"Segoe UI\", Roboto, Helvetica, Arial");
#else
    return QStringLiteral("Roboto, Noto Sans, Helvetica, Arial");
#endif
}

}

// ============================================================================
// CheckButton: fully custom-painted checkbox button (bypasses macOS native)
// ============================================================================

class CheckButton : public QPushButton {
public:
    explicit CheckButton(QWidget *parent = nullptr) : QPushButton(parent) {
        setCheckable(true);
        setChecked(true);
        setObjectName("paramCheckBox");
        setFixedSize(31, 23);
        setCursor(Qt::PointingHandCursor);
        // No text, no icon — all painting is manual
        setText(QString());
    }

    void setTheme(const QString &theme) {
        m_isDark = (theme != "light");
        update();
    }

protected:
    void paintEvent(QPaintEvent *) override {
        QPainter p(this);
        if (m_isDark) {
            const QPixmap image(isChecked() ? ":/classic/checked_image.png" : ":/classic/unchecked_image.png");
            p.drawPixmap(5, (height() - 13) / 2, 13, 13, image);
            if (hasFocus()) {
                p.setPen(QPen(QColor("#707070"), 1, Qt::DotLine));
                p.drawRect(QRect(3, 3, 17, 17));
            }
            return;
        }
        p.setRenderHint(QPainter::Antialiasing);

        const QRectF r(5.5, 4.5, 13, 13);
        const bool on = isChecked();
        const bool hov = underMouse();

        // Background
        if (on) {
            p.setBrush(QColor(hov ? "#2563EB" : "#3B82F6"));
            p.setPen(QPen(QColor(hov ? "#1D4ED8" : "#2563EB"), 1.5));
        } else {
            p.setBrush(QColor(hov
                ? (m_isDark ? "#25262B" : "#F9FAFB")
                : (m_isDark ? "#1A1B1E" : "#FFFFFF")));
            p.setPen(QPen(QColor(hov
                ? (m_isDark ? "#6B7280" : "#6B7280")
                : (m_isDark ? "#4B5563" : "#9CA3AF")), 1.5));
        }
        p.drawRoundedRect(r, 3.5, 3.5);

        // Checkmark
        if (on) {
            p.setPen(QPen(Qt::white, 1.8, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin));
            const float cx = r.center().x();
            const float cy = r.center().y();
            // polyline: left-bottom of check, bottom apex, top-right
            QPointF pts[3] = {
                { cx - 4.0f, cy },
                { cx - 1.0f, cy + 3.5f },
                { cx + 4.5f, cy - 3.5f }
            };
            p.drawPolyline(pts, 3);
        }
    }

    void enterEvent(QEnterEvent *e) override { QPushButton::enterEvent(e); update(); }
    void leaveEvent(QEvent *e) override  { QPushButton::leaveEvent(e);  update(); }

private:
    bool m_isDark = true;
};

static QIcon makeAppIcon() {
    // 统一使用带透明圆角的应用图标，避免窗口和托盘显示方形灰底。
    QIcon icon(":/res/app_icon.png");
    return icon;
}
// AppWindow Implementation
// ============================================================================

AppWindow::AppWindow(QWidget *parent)
    : QMainWindow(parent)
    , tabWidget(nullptr)
    , tabCombo(nullptr)
    , addTabButton(nullptr)
    , saveButton(nullptr)
    , trayIcon(nullptr)
    , trayMenu(nullptr)
    , settings(nullptr)
    , currentTheme("dark")
    , currentTabIndex(0)
{
    setWindowTitle("EasyCommandRunner");
    setWindowIcon(makeAppIcon());

    // 应用设置
    if (qEnvironmentVariableIsSet("ECR_DATA_DIR")) {
        QDir().mkpath(dataDirPath());
        settings = new QSettings(dataDirPath() + "/settings.ini", QSettings::IniFormat, this);
    } else {
        settings = new QSettings("SleepyKanata", "EasyCommandRunner", this);
    }

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
    mainLayout->setContentsMargins(10, 12, 10, 4);
    mainLayout->setSpacing(8);

    tabWidget = new QTabWidget(this);
    tabWidget->setTabsClosable(true);
    tabWidget->setMovable(true);
    tabCombo = new QComboBox(this);
    tabCombo->setObjectName("tabCombo");
    tabCombo->setView(new QListView());
    tabCombo->setMaxVisibleItems(30);
    tabWidget->setCornerWidget(tabCombo, Qt::TopRightCorner);
    connect(tabCombo, QOverload<int>::of(&QComboBox::activated), tabWidget, &QTabWidget::setCurrentIndex);
    tabWidget->setUsesScrollButtons(true);
    QHBoxLayout *pageLayout = new QHBoxLayout();
    pageLayout->setSpacing(4);
    previousTabButton = new QPushButton(this);
    previousTabButton->setObjectName("previousTabBtn");
    nextTabButton = new QPushButton(this);
    nextTabButton->setObjectName("nextTabBtn");
    for (auto *button : {previousTabButton, nextTabButton}) {
        button->setProperty("role", "nav");
        button->setFixedSize(30, 72);
    }
    previousTabButton->setAccessibleName("上一个标签页");
    previousTabButton->setToolTip("上一个标签页 · Ctrl+←");
    nextTabButton->setAccessibleName("下一个标签页");
    nextTabButton->setToolTip("下一个标签页 · Ctrl+→");
    setButtonIcon(previousTabButton, "icon_chevron_left");
    setButtonIcon(nextTabButton, "icon_chevron_right");
    connect(previousTabButton, &QPushButton::clicked, this, &AppWindow::onPreviousTab);
    connect(nextTabButton, &QPushButton::clicked, this, &AppWindow::onNextTab);
    pageLayout->addWidget(previousTabButton, 0, Qt::AlignVCenter);
    pageLayout->addWidget(tabWidget, 1);
    pageLayout->addWidget(nextTabButton, 0, Qt::AlignVCenter);
    mainLayout->addLayout(pageLayout, 1);

    // 配置操作与运行分组，不再占用四个等权重的大按钮。
    QWidget *buttonContainer = new QWidget(this);
    QHBoxLayout *buttonLayout = new QHBoxLayout(buttonContainer);
    buttonLayout->setContentsMargins(34, 4, 34, 4);
    buttonLayout->setSpacing(6);
    addTabButton = new QPushButton("新建标签", this);
    reloadButton = new QPushButton("重新加载", this);
    copyTabButton = new QPushButton("复制标签", this);
    saveButton = new QPushButton("保存", this);
    saveButton->setObjectName("saveBtn");
    saveButton->setToolTip("保存所有标签配置 · Ctrl+S");
    addTabButton->setToolTip("新建标签 · Ctrl+T");
    reloadButton->setToolTip("从磁盘重新加载 · Ctrl+L");
    setButtonIcon(addTabButton, "icon_add");
    setButtonIcon(reloadButton, "icon_refresh");
    setButtonIcon(copyTabButton, "icon_copy");
    setButtonIcon(saveButton, "icon_save");
    for (auto *button : {addTabButton, copyTabButton, reloadButton}) buttonLayout->addWidget(button);
    buttonLayout->addStretch();
    QPushButton *settingsButton = new QPushButton(this);
    settingsButton->setProperty("role", "icon");
    settingsButton->setToolTip("设置 / 备份恢复");
    settingsButton->setAccessibleName("设置");
    setButtonIcon(settingsButton, "icon_settings");
    connect(settingsButton, &QPushButton::clicked, this, &AppWindow::onSettingsClicked);
    buttonLayout->addWidget(settingsButton);
    buttonLayout->addWidget(saveButton);
    sessionLabel = new QLabel(this);
    sessionLabel->setProperty("role", "hint");
    statusBar()->addPermanentWidget(sessionLabel);
    mainLayout->addWidget(buttonContainer);
    setCentralWidget(centralWidget);

    logPanel = new LogPanel(this);
    addDockWidget(Qt::BottomDockWidgetArea, logPanel);
    logPanel->hide(); // 不挤占原版编辑区；首次运行或通过「视图」打开。
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

    // 文本框保留 Ctrl+A 的直觉语义；参数全选使用 Ctrl+Shift+A。
    QAction *selectAllAction = editMenu->addAction("启用全部参数");
    selectAllAction->setShortcut(Qt::CTRL | Qt::SHIFT | Qt::Key_A);
    connect(selectAllAction, &QAction::triggered, this, [this]() {
        if (auto *tab = qobject_cast<CommandTab*>(tabWidget->currentWidget())) tab->selectAllParameters(true);
    });
    QAction *deselectAllAction = editMenu->addAction("禁用全部参数");
    deselectAllAction->setShortcut(Qt::CTRL | Qt::Key_D);
    connect(deselectAllAction, &QAction::triggered, this, [this]() {
        if (auto *tab = qobject_cast<CommandTab*>(tabWidget->currentWidget())) tab->selectAllParameters(false);
    });

    // 视图菜单
    QMenu *viewMenu = menuBar->addMenu("视图(&V)");
    viewMenu->addAction(logPanel->toggleViewAction());
    viewMenu->addSeparator();

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
    trayIcon->setIcon(makeAppIcon());
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
    connect(reloadButton, &QPushButton::clicked, this, &AppWindow::onReloadConfigClicked);
    connect(copyTabButton, &QPushButton::clicked, this, &AppWindow::onCopyTabConfigClicked);

    // 快捷键
    new QShortcut(Qt::CTRL | Qt::Key_Return, this, SLOT(onRunButtonClicked()));
}

void AppWindow::closeEvent(QCloseEvent *event) {
    saveApplicationSettings();
    saveConfiguration();
    if (QSystemTrayIcon::isSystemTrayAvailable()) {
        event->ignore();
        hide();
    } else {
        event->ignore();
        onExitClicked();
    }
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
    sessionLabel->setText(QString("%1 / %2 个标签").arg(index + 1).arg(tabWidget->count()));
}

void AppWindow::onTabMoved(int from, int to) {
    updateTabCombo();
}

void AppWindow::onRunButtonClicked() {
    CommandTab *tab = qobject_cast<CommandTab*>(tabWidget->currentWidget());
    if (!tab) return;

    QString program = tab->getProgram();
    if (program.trimmed().isEmpty()) {
        QMessageBox::warning(this, "提示", "请输入程序名称");
        return;
    }

    logPanel->startCommand(tab->getTabName(), tab->getCommand(), tab->getWorkingDirectory());
}

void AppWindow::onSaveButtonClicked() {
    if (saveConfiguration()) statusBar()->showMessage("已保存所有标签配置", 3500);
}

void AppWindow::onReloadConfigClicked() {
    if (!confirmDiscard()) return;
    if (loadConfiguration()) {
        applyTheme(currentTheme);
        statusBar()->showMessage("已重新加载配置", 3500);
    }
}

void AppWindow::onCopyTabConfigClicked() {
    CommandTab *currentTab = qobject_cast<CommandTab*>(tabWidget->currentWidget());
    if (!currentTab) return;

    const QString sourceName = currentTab->getTabName().trimmed();
    const QString baseName = sourceName.isEmpty() ? "未命名" : sourceName;
    QString newName = baseName + " - 副本";
    int suffix = 2;
    while (tabCombo->findText(newName) >= 0) newName = baseName + QString(" - 副本 %1").arg(suffix++);

    createTab(newName);

    CommandTab *newTab = qobject_cast<CommandTab*>(tabWidget->currentWidget());
    if (!newTab) return;

    newTab->loadConfiguration(currentTab->saveConfiguration());
    newTab->setTabName(newName);
    updateTabCombo();
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
    if (theme != currentTheme) {
        currentTheme = theme;
        applyTheme(theme);
    }
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
    if (logPanel->runningCount() > 0 && QMessageBox::question(this, "退出",
        QString("还有 %1 个任务正在运行，退出将停止这些任务。是否继续？").arg(logPanel->runningCount()),
        QMessageBox::Yes | QMessageBox::No, QMessageBox::No) != QMessageBox::Yes) {
        return;
    }
    QApplication::quit();
}

void AppWindow::onNextTab() {
    int index = tabWidget->currentIndex();
    int count = tabWidget->count();
    if (count > 0) tabWidget->setCurrentIndex((index + 1) % count);
}

void AppWindow::onPreviousTab() {
    int index = tabWidget->currentIndex();
    int count = tabWidget->count();
    if (count > 0) tabWidget->setCurrentIndex((index - 1 + count) % count);
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
        QString stylesheet = QString::fromUtf8(file.readAll());
        stylesheet.replace(
            QStringLiteral("\".AppleSystemUIFont\", \"Segoe UI\", Roboto, Helvetica, Arial, sans-serif"),
            platformFontFamily());
        qApp->setStyleSheet(stylesheet);
        file.close();
    }
}

void AppWindow::applyTheme(const QString &theme) {
    currentTheme = theme;
    loadStylesheet(theme);
    updateButtonIcons();
    settings->setValue("ui/theme", theme);
}

void AppWindow::updateButtonIcons() {
    for (auto *button : findChildren<QPushButton*>()) {
        const QString icon = button->property("svgIcon").toString();
        if (!icon.isEmpty()) button->setIcon(themedIcon(icon, currentTheme));
    }
    for (int i = 0; i < tabWidget->count(); ++i) {
        CommandTab *tab = qobject_cast<CommandTab*>(tabWidget->widget(i));
        if (tab) tab->updateRemoveButtonIcons(currentTheme);
    }
}


bool AppWindow::loadConfiguration() {
    // 先验证磁盘内容，避免后端把读取错误吞成 {} 后在退出时覆盖损坏配置。
    QFile file(configFilePath());
    if (file.exists()) {
        QJsonParseError error;
        const bool readable = file.open(QIODevice::ReadOnly);
        const QJsonDocument original = QJsonDocument::fromJson(readable ? file.readAll() : QByteArray(), &error);
        if (!readable || error.error != QJsonParseError::NoError || !original.isObject()) {
            configWritable = false;
            if (tabWidget->count() == 0) createTab("标签1");
            statusBar()->showMessage("配置读取失败；已阻止自动覆盖。请修复文件或从设置恢复备份。");
            return false;
        }
    }
    const QString configJson = RustBackend::loadConfig(configFilePath(), backupDirPath());
    QJsonObject config = QJsonDocument::fromJson(configJson.toUtf8()).object();
    configWritable = true;

    // PyQt5 旧版把控件名称当字段名、行号/勾选状态分开保存；仅在内存迁移。
    QJsonArray normalizedTabs;
    const QJsonArray legacyRows = config.value("line_codes").toArray();
    const QJsonArray legacyChecks = config.value("checkbox_statuses").toArray();
    int tabIndex = 0;
    for (const QJsonValue &value : config.value("tabs").toArray()) {
        QJsonObject tab = value.toObject();
        if (tab.contains("name_edit2")) {
            const QJsonObject checks = tabIndex < legacyChecks.size() ? legacyChecks.at(tabIndex).toObject() : QJsonObject();
            QJsonArray rows;
            rows.append(QJsonObject{{"function", tab.value("name_edit3_1")},
                {"parameter", tab.value("name_edit3_2")}, {"comment", tab.value("name_edit3_3")},
                {"enabled", checks.value("chkbox1").toBool(true)}});
            QStringList codes = tabIndex < legacyRows.size() ? legacyRows.at(tabIndex).toObject().keys() : QStringList();
            std::sort(codes.begin(), codes.end(), [](const QString &a, const QString &b) { return a.toInt() < b.toInt(); });
            for (const QString &code : codes) {
                rows.append(QJsonObject{{"function", tab.value("function" + code)},
                    {"parameter", tab.value("parameter" + code)}, {"comment", tab.value("comment" + code)},
                    {"enabled", checks.value("chkbox" + code).toBool(true)}});
            }
            tab = QJsonObject{{"name", tab.value("name_edit_title")}, {"working_dir", tab.value("name_edit1")},
                {"program", tab.value("name_edit2")}, {"other_args", tab.value("name_editOther")},
                {"description", tab.value("editDescription")}, {"functions", rows}};
        }
        normalizedTabs.append(tab);
        ++tabIndex;
    }
    config["tabs"] = normalizedTabs;
    const int requestedIndex = config.value("current_tab_index").toInt(currentTabIndex);

    if (config.contains("theme") && config.value("theme").isString()) {
        currentTheme = config.value("theme").toString(currentTheme);
    }

    while (tabWidget->count() > 0) {
        QWidget *widget = tabWidget->widget(0);
        tabWidget->removeTab(0);
        delete widget;
    }

    const QJsonArray tabsArray = config.value("tabs").toArray();
    for (const QJsonValue &tabValue : tabsArray) {
        if (!tabValue.isObject()) continue;
        const QJsonObject tabConfig = tabValue.toObject();
        const QString tabName = tabConfig.value("name").toString().trimmed();
        createTab(tabName.isEmpty() ? QStringLiteral("未命名") : tabName);
        CommandTab *tab = qobject_cast<CommandTab*>(tabWidget->currentWidget());
        if (tab) {
            tab->loadConfiguration(tabConfig);
        }
    }

    if (tabWidget->count() == 0) {
        createTab(QStringLiteral("标签1"));
    }

    int savedIndex = requestedIndex;
    if (savedIndex < 0 || savedIndex >= tabWidget->count()) {
        savedIndex = 0;
    }
    tabWidget->setCurrentIndex(savedIndex);
    currentTabIndex = savedIndex;
    updateTabCombo();
    savedConfig = configuration();
    return true;
}

QJsonObject AppWindow::configuration() const {
    QJsonObject config;
    config["theme"] = currentTheme;
    config["current_tab_index"] = tabWidget->currentIndex();

    QJsonArray tabsArray;
    for (int i = 0; i < tabWidget->count(); ++i) {
        CommandTab *tab = qobject_cast<CommandTab*>(tabWidget->widget(i));
        if (tab) {
            tabsArray.append(tab->saveConfiguration());
        }
    }
    config["tabs"] = tabsArray;
    return config;
}

bool AppWindow::saveConfiguration() {
    if (!configWritable) return false;
    const QJsonObject config = configuration();
    const QJsonDocument doc(config);
    if (!RustBackend::saveConfig(configFilePath(), backupDirPath(), QString::fromUtf8(doc.toJson(QJsonDocument::Indented)))) {
        QMessageBox::warning(this, "错误", "保存配置失败");
        return false;
    }

    savedConfig = config;
    return true;
}

bool AppWindow::confirmDiscard() {
    if (configuration() == savedConfig) return true;
    const auto answer = QMessageBox::question(this, "未保存的修改", "当前配置已修改。先保存再继续？",
        QMessageBox::Save | QMessageBox::Discard | QMessageBox::Cancel, QMessageBox::Cancel);
    return answer == QMessageBox::Discard || (answer == QMessageBox::Save && saveConfiguration());
}

bool AppWindow::restoreBackup(const QString &name) {
    QFile candidate(backupDirPath() + "/" + name);
    if (!candidate.open(QIODevice::ReadOnly) || !QJsonDocument::fromJson(candidate.readAll()).isObject()) {
        QMessageBox::warning(this, "恢复失败", "备份文件不是有效的 JSON 对象。");
        return false;
    }
    if (!confirmDiscard()) return false;
    // 恢复前独立备份当前磁盘文件，包括损坏文件，以便撤销恢复操作。
    if (QFile::exists(configFilePath())) {
        const QString safety = backupDirPath() + "/before_restore_" + QDateTime::currentDateTime().toString("yyyyMMdd_HHmmss_zzz") + ".json";
        if (!QFile::copy(configFilePath(), safety)) return false;
    }
    if (!RustBackend::restoreBackup(configFilePath(), backupDirPath(), name)) return false;
    if (!loadConfiguration()) return false;
    applyTheme(currentTheme);
    return true;
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
    tab->setTabName(name);
    int index = tabWidget->addTab(tab, name);

    // Apply theme icons to the new tab's param rows immediately
    tab->updateRemoveButtonIcons(currentTheme);

    connect(tab, &CommandTab::runRequested, this, &AppWindow::onRunButtonClicked);
    connect(tab, &CommandTab::previousTabRequested, this, &AppWindow::onPreviousTab);
    connect(tab, &CommandTab::nextTabRequested, this, &AppWindow::onNextTab);
    connect(tab, &CommandTab::logRequested, this, [this]() { logPanel->show(); logPanel->raise(); });
    connect(tab, &CommandTab::configurationChanged, this, [this]() {
        if (configuration() != savedConfig) statusBar()->showMessage("有未保存的修改 · Ctrl+S 保存");
    });

    tabWidget->setCurrentIndex(index);
    updateTabCombo();

    // Sync titleEdit changes to the tab label and combo in real-time
    connect(tab, &CommandTab::titleChanged, [this, tab](const QString &newTitle) {
        int idx = tabWidget->indexOf(tab);
        if (idx >= 0) {
            tabWidget->setTabText(idx, newTitle.isEmpty() ? tr("未命名") : newTitle);
            updateTabCombo();
            tabCombo->setCurrentIndex(tabWidget->currentIndex());
        }
    });
}

void AppWindow::updateTabCombo() {
    tabCombo->blockSignals(true);
    tabCombo->clear();
    for (int i = 0; i < tabWidget->count(); ++i) {
        tabCombo->addItem(tabWidget->tabText(i));
    }
    tabCombo->setCurrentIndex(tabWidget->currentIndex());
    tabCombo->blockSignals(false);
    const bool multiple = tabWidget->count() > 1;
    previousTabButton->setEnabled(multiple);
    nextTabButton->setEnabled(multiple);
    sessionLabel->setText(QString("%1 / %2 个标签").arg(tabWidget->currentIndex() + 1).arg(tabWidget->count()));
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
    updateCommandPreview();
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

QString CommandTab::getCommand() const {
    return commandPreviewEdit->toPlainText();
}

QVector<QPair<QString, QString>> CommandTab::getFunctions() const {
    QVector<QPair<QString, QString>> result;
    for (int i = 0; i < functionEdits.size(); ++i) {
        result.append({functionEdits[i]->text(), parameterEdits[i]->text()});
    }
    return result;
}

QVector<bool> CommandTab::getEnabledStates() const {
    QVector<bool> result;
    result.reserve(rowCheckBoxes.size());
    for (auto *checkBox : rowCheckBoxes) result.append(checkBox && checkBox->isChecked());
    return result;
}

QString CommandTab::getOtherArgs() const {
    return otherArgsEdit->text();
}

QString CommandTab::getDescription() const {
    return descriptionEdit->toPlainText();
}

void CommandTab::loadConfiguration(const QJsonObject &config) {
    m_loading = true;
    clearRows();
    titleEdit->setText(config.value("name").toString());
    workingDirEdit->setText(config.value("working_dir").toString());
    programEdit->setText(config.value("program").toString());
    otherArgsEdit->setText(config.value("other_args").toString());
    descriptionEdit->setPlainText(config.value("description").toString());

    // 加载函数列表
    QJsonArray functions = config.value("functions").toArray();
    for (const auto &func : functions) {
        QJsonObject funcObj = func.toObject();
        if (func.isArray()) {
            const QJsonArray pair = func.toArray();
            funcObj["function"] = pair.size() > 0 ? pair.at(0) : QJsonValue("");
            funcObj["parameter"] = pair.size() > 1 ? pair.at(1) : QJsonValue("");
        }
        onAddFunctionClicked();
        int last = functionEdits.size() - 1;
        functionEdits[last]->setText(funcObj.value("function").toString());
        parameterEdits[last]->setText(funcObj.value("parameter").toString());
        rowCheckBoxes[last]->setChecked(funcObj.value("enabled").toBool(true));
        if (last < commentEdits.size()) {
            commentEdits[last]->setText(funcObj.value("comment").toString());
        }
    }

    // 若没有任何行则保留一个空行
    if (functionEdits.isEmpty()) {
        onAddFunctionClicked();
    }
    m_loading = false;
    updateCommandPreview();
}

QJsonObject CommandTab::saveConfiguration() const {
    QJsonObject config;
    config.insert("name", titleEdit->text());
    config.insert("working_dir", workingDirEdit->text());
    config.insert("program", programEdit->text());
    config.insert("other_args", otherArgsEdit->text());
    config.insert("description", descriptionEdit->toPlainText());

    QJsonArray functions;
    for (int i = 0; i < functionEdits.size(); ++i) {
        QJsonObject func;
        func.insert("function", functionEdits[i]->text());
        func.insert("parameter", parameterEdits[i]->text());
        func.insert("comment", i < commentEdits.size() ? commentEdits[i]->text() : QString());
        func.insert("enabled", rowCheckBoxes[i]->isChecked());
        functions.append(func);
    }
    config.insert("functions", functions);

    return config;
}

void CommandTab::clearRows() {
    while (QLayoutItem *item = functionsLayout->takeAt(0)) {
        delete item->widget();
        delete item;
    }
    rowCheckBoxes.clear();
    functionEdits.clear();
    parameterEdits.clear();
    commentEdits.clear();
    removeButtons.clear();
    functionCounter = 1;
}

void CommandTab::clear() {
    loadConfiguration(QJsonObject());
}

bool CommandTab::parseCommandText(const QString &text, bool append) {
    const auto tokens = RustBackend::parseCommand(text, append);
    if (tokens.isEmpty()) return false;
    m_loading = true;
    if (!append) {
        clearRows();
        programEdit->setText(tokens[0]);
        otherArgsEdit->clear();
    } else if (functionEdits.size() == 1 && functionEdits[0]->text().isEmpty()
        && parameterEdits[0]->text().isEmpty() && commentEdits[0]->text().isEmpty()) {
        clearRows();
    }
    for (int i = append ? 0 : 1; i < tokens.size(); i += 2) {
        onAddFunctionClicked();
        functionEdits.last()->setText(tokens[i]);
        parameterEdits.last()->setText(i + 1 < tokens.size() ? tokens[i + 1] : QString());
    }
    if (functionEdits.isEmpty()) onAddFunctionClicked();
    m_loading = false;
    updateCommandPreview();
    return true;
}

void CommandTab::onParseCommandClicked() {
    const bool hasRows = std::any_of(functionEdits.begin(), functionEdits.end(), [](auto *edit) { return !edit->text().isEmpty(); })
        || std::any_of(parameterEdits.begin(), parameterEdits.end(), [](auto *edit) { return !edit->text().isEmpty(); })
        || std::any_of(commentEdits.begin(), commentEdits.end(), [](auto *edit) { return !edit->text().isEmpty(); })
        || !otherArgsEdit->text().isEmpty();
    if (hasRows && QMessageBox::question(this, "解析命令", "解析将替换现有参数和行备注，是否继续？",
        QMessageBox::Yes | QMessageBox::No, QMessageBox::No) != QMessageBox::Yes) return;
    if (!parseCommandText(programEdit->text())) QMessageBox::warning(this, "无法解析", "请输入命令，并检查引号是否完整。");
}

void CommandTab::selectAllParameters(bool selected) {
    for (auto *check : rowCheckBoxes) check->setChecked(selected);
}

void CommandTab::onAddFunctionClicked() {
    QHBoxLayout *rowLayout = new QHBoxLayout();
    rowLayout->setContentsMargins(0, 0, 0, 0);
    rowLayout->setSpacing(6);

    CheckButton *checkBox = new CheckButton();
    checkBox->setTheme(m_theme);
    checkBox->setAccessibleName(QString("启用参数%1").arg(functionCounter));

    QLineEdit *funcEdit = new PathLineEdit();
    funcEdit->setPlaceholderText(QString("功能%1").arg(functionCounter));
    funcEdit->setObjectName("paramFuncEdit");
    QLineEdit *paramEdit = new PathLineEdit();
    paramEdit->setPlaceholderText(QString("参数%1").arg(functionCounter));
    paramEdit->setObjectName("paramValueEdit");
    QLineEdit *commentEdit = new PathLineEdit();
    commentEdit->setPlaceholderText("备注（不参与执行）");
    commentEdit->setObjectName("paramCommentEdit");

    QPushButton *removeBtn = new QPushButton();
    removeBtn->setProperty("role", "icon");
    removeBtn->setFixedSize(30, 30);
    removeBtn->setToolTip("删除此参数行");
    removeBtn->setAccessibleName("删除参数行");
    setButtonIcon(removeBtn, "icon_delete", m_theme);
    removeBtn->setObjectName("paramRemoveBtn");
    rowLayout->addWidget(checkBox, 0, Qt::AlignVCenter);
    rowLayout->addWidget(funcEdit, 1);
    rowLayout->addWidget(paramEdit, 1);
    rowLayout->addWidget(commentEdit, 1);
    rowLayout->addWidget(removeBtn);

    rowCheckBoxes.append(checkBox);
    functionEdits.append(funcEdit);
    parameterEdits.append(paramEdit);
    commentEdits.append(commentEdit);
    removeButtons.append(removeBtn);

    QWidget *rowWidget = new QWidget();
    rowWidget->setLayout(rowLayout);
    rowWidget->setObjectName(QString("paramRow_%1").arg(functionCounter++));
    rowWidget->setSizePolicy(QSizePolicy::Preferred, QSizePolicy::Fixed);

    // 勾选变化时实时更新预览
    connect(checkBox, &QPushButton::toggled, this, &CommandTab::updateCommandPreview);
    // 参数名/值变化时也实时更新预览
    connect(funcEdit,  &QLineEdit::textChanged, this, &CommandTab::updateCommandPreview);
    connect(paramEdit, &QLineEdit::textChanged, this, &CommandTab::updateCommandPreview);

    functionsLayout->addWidget(rowWidget);
    connect(commentEdit, &QLineEdit::textChanged, this, &CommandTab::configurationChanged);

    connect(removeBtn, &QPushButton::clicked, [this, rowWidget, checkBox, funcEdit, paramEdit, commentEdit, removeBtn]() {
        rowCheckBoxes.removeOne(checkBox);
        functionEdits.removeOne(funcEdit);
        parameterEdits.removeOne(paramEdit);
        commentEdits.removeOne(commentEdit);
        removeButtons.removeOne(removeBtn);
        functionsLayout->removeWidget(rowWidget);
        rowWidget->hide();
        rowWidget->deleteLater();
        if (functionEdits.isEmpty()) onAddFunctionClicked();
        updateCommandPreview();
    });
}

void CommandTab::updateRemoveButtonIcons(const QString &theme) {
    m_theme = theme;
    for (auto *button : findChildren<QPushButton*>()) {
        const QString icon = button->property("svgIcon").toString();
        if (!icon.isEmpty()) button->setIcon(themedIcon(icon, theme));
    }
    for (QPushButton *cb : rowCheckBoxes) {
        static_cast<CheckButton*>(cb)->setTheme(theme);
    }
}

void CommandTab::onRemoveFunctionClicked() {
    // 已经通过 lambda 在 onAddFunctionClicked 实现了单行删除
}

void CommandTab::onPreviewCommandClicked() {
    updateCommandPreview();
}

void CommandTab::setupUI() {
    // 表单整体滚动；运行操作固定，页面两侧导航由主窗口提供。
    QVBoxLayout *layout = new QVBoxLayout(this);
    layout->setContentsMargins(12, 12, 12, 12);
    layout->setSpacing(10);
    QScrollArea *scrollArea = new QScrollArea(this);
    scrollArea->setObjectName("commandScrollArea");
    scrollArea->setWidgetResizable(true);
    QWidget *form = new QWidget();
    QVBoxLayout *formLayout = new QVBoxLayout(form);
    formLayout->setContentsMargins(4, 2, 4, 2);
    formLayout->setSpacing(10);
    auto fieldLabel = [](const QString &text) {
        auto *label = new QLabel(text);
        label->setFixedWidth(66);
        return label;
    };

    QHBoxLayout *titleLayout = new QHBoxLayout();
    titleEdit = new PathLineEdit();
    titleEdit->setObjectName("titleEdit");
    titleEdit->setPlaceholderText("给这条命令起个名字");
    titleLayout->addWidget(fieldLabel("名称"));
    titleLayout->addWidget(titleEdit);
    formLayout->addLayout(titleLayout);

    QHBoxLayout *dirLayout = new QHBoxLayout();
    workingDirEdit = new PathLineEdit();
    workingDirEdit->setObjectName("workingDirEdit");
    workingDirEdit->setPlaceholderText("为空则使用程序当前目录");
    dirLayout->addWidget(fieldLabel("工作目录"));
    dirLayout->addWidget(workingDirEdit);
    QPushButton *browseButton = new QPushButton();
    browseButton->setProperty("role", "icon");
    browseButton->setFixedSize(32, 32);
    browseButton->setToolTip("选择工作目录，也可以直接拖入文件夹");
    browseButton->setAccessibleName("选择工作目录");
    setButtonIcon(browseButton, "icon_folder");
    dirLayout->addWidget(browseButton);
    connect(browseButton, &QPushButton::clicked, this, [this]() {
        const QString path = QFileDialog::getExistingDirectory(this, "选择工作目录", workingDirEdit->text());
        if (!path.isEmpty()) workingDirEdit->setText(path);
    });
    formLayout->addLayout(dirLayout);

    QHBoxLayout *programLayout = new QHBoxLayout();
    programEdit = new PathLineEdit();
    programEdit->setObjectName("programEdit");
    programEdit->setPlaceholderText("粘贴命令可以解析");
    parseButton = new QPushButton("解析");
    parseButton->setToolTip("将完整命令拆成可编辑的参数行（不会执行）");
    setButtonIcon(parseButton, "icon_preview");
    programLayout->addWidget(fieldLabel("程序"));
    programLayout->addWidget(programEdit);
    programLayout->addWidget(parseButton);
    formLayout->addLayout(programLayout);

    QHBoxLayout *parameterHeader = new QHBoxLayout();
    parameterHeader->setContentsMargins(37, 0, 36, 0);
    for (const QString &text : {QString("选项 / 功能"), QString("参数值"), QString("备注")}) {
        QLabel *label = new QLabel(text);
        label->setProperty("role", "hint");
        parameterHeader->addWidget(label, 1);
    }
    formLayout->addLayout(parameterHeader);
    functionsLayout = new QVBoxLayout();
    functionsLayout->setContentsMargins(0, 0, 0, 0);
    functionsLayout->setSpacing(6);
    formLayout->addLayout(functionsLayout);
    onAddFunctionClicked();

    QHBoxLayout *paramActions = new QHBoxLayout();
    addFunctionButton = new QPushButton("添加参数");
    setButtonIcon(addFunctionButton, "icon_add");
    addFunctionButton->setObjectName("addFunctionBtn");
    selectAllButton = new QPushButton("全选");
    selectAllButton->setToolTip("启用全部参数 · Ctrl+Shift+A");
    deselectAllButton = new QPushButton("全不选");
    deselectAllButton->setToolTip("禁用全部参数 · Ctrl+D");
    paramActions->addWidget(addFunctionButton);
    paramActions->addWidget(selectAllButton);
    paramActions->addWidget(deselectAllButton);
    paramActions->addStretch();
    parameterCountLabel = new QLabel();
    parameterCountLabel->setProperty("role", "hint");
    paramActions->addWidget(parameterCountLabel);
    formLayout->addLayout(paramActions);

    QHBoxLayout *appendLayout = new QHBoxLayout();
    appendCommandEdit = new PathLineEdit();
    appendCommandEdit->setObjectName("appendCommandEdit");
    appendCommandEdit->setPlaceholderText("粘贴部分命令，追加到现有参数…");
    QPushButton *appendButton = new QPushButton("追加解析");
    appendLayout->addWidget(appendCommandEdit, 1);
    appendLayout->addWidget(appendButton);
    formLayout->addLayout(appendLayout);
    connect(appendButton, &QPushButton::clicked, this, [this]() {
        if (parseCommandText(appendCommandEdit->text(), true)) appendCommandEdit->clear();
        else QMessageBox::warning(this, "无法解析", "请输入部分命令，并检查引号是否完整。");
    });
    connect(appendCommandEdit, &QLineEdit::returnPressed, appendButton, &QPushButton::click);

    otherArgsEdit = new PathLineEdit();
    otherArgsEdit->setObjectName("otherArgsEdit");
    otherArgsEdit->setPlaceholderText("其他参数 / 重定向 / 管道（按原文追加）");
    formLayout->addWidget(otherArgsEdit);
    descriptionEdit = new PathTextEdit();
    descriptionEdit->setObjectName("descriptionEdit");
    descriptionEdit->setPlaceholderText("描述 / 使用说明（不会执行）");
    descriptionEdit->setMinimumHeight(64);
    descriptionEdit->setMaximumHeight(100);
    formLayout->addWidget(descriptionEdit);
    commandPreviewEdit = new QTextEdit();
    commandPreviewEdit->setObjectName("commandPreviewEdit");
    commandPreviewEdit->setReadOnly(true);
    commandPreviewEdit->setPlaceholderText("命令预览 · 修改参数后实时更新");
    commandPreviewEdit->setMinimumHeight(72);
    commandPreviewEdit->setMaximumHeight(90);
    scrollArea->setWidget(form);
    layout->addWidget(scrollArea, 1);
    layout->addWidget(commandPreviewEdit);

    QHBoxLayout *runActions = new QHBoxLayout();
    QPushButton *logButton = new QPushButton("运行日志");
    setButtonIcon(logButton, "icon_terminal");
    connect(logButton, &QPushButton::clicked, this, &CommandTab::logRequested);
    runActions->addWidget(logButton);
    runActions->addStretch();
    previewButton = new QPushButton("刷新预览");
    setButtonIcon(previewButton, "icon_preview");
    runActions->addWidget(previewButton);
    QPushButton *copyButton = new QPushButton("复制命令");
    setButtonIcon(copyButton, "icon_copy");
    connect(copyButton, &QPushButton::clicked, this, [this]() { QApplication::clipboard()->setText(getCommand()); });
    runActions->addWidget(copyButton);
    QPushButton *runButton = new QPushButton("运行");
    runButton->setObjectName("runBtn");
    runButton->setToolTip("在独立 shell 进程中运行 · Ctrl+Enter");
    setButtonIcon(runButton, "icon_run");
    runActions->addWidget(runButton);
    layout->addLayout(runActions);
    connect(runButton, &QPushButton::clicked, this, &CommandTab::runRequested);
}

void CommandTab::setupConnections() {
    connect(parseButton, &QPushButton::clicked, this, &CommandTab::onParseCommandClicked);
    connect(addFunctionButton, &QPushButton::clicked, this, &CommandTab::onAddFunctionClicked);
    connect(previewButton, &QPushButton::clicked, this, &CommandTab::onPreviewCommandClicked);
    connect(titleEdit, &QLineEdit::textChanged, this, &CommandTab::titleChanged);
    connect(titleEdit, &QLineEdit::textChanged, this, &CommandTab::configurationChanged);
    connect(workingDirEdit, &QLineEdit::textChanged, this, &CommandTab::configurationChanged);
    connect(descriptionEdit, &QTextEdit::textChanged, this, &CommandTab::configurationChanged);

    // Live preview on program / other-args changes
    connect(programEdit,  &QLineEdit::textChanged, this, &CommandTab::updateCommandPreview);
    connect(otherArgsEdit, &QLineEdit::textChanged, this, &CommandTab::updateCommandPreview);

    // Select-all / Deselect-all
    connect(selectAllButton, &QPushButton::clicked, [this]() {
        for (QPushButton *cb : rowCheckBoxes) {
            if (cb) cb->setChecked(true);
        }
    });
    connect(deselectAllButton, &QPushButton::clicked, [this]() {
        for (QPushButton *cb : rowCheckBoxes) {
            if (cb) cb->setChecked(false);
        }
    });
}

void CommandTab::updateCommandPreview() {
    if (m_loading || !commandPreviewEdit) return;
    int checkedCount = 0;
    for (auto *check : rowCheckBoxes) if (check->isChecked()) ++checkedCount;
    parameterCountLabel->setText(QString("%1 / %2 已启用").arg(checkedCount).arg(rowCheckBoxes.size()));
    emit configurationChanged();
    QString program = programEdit->text().trimmed();
    if (program.isEmpty()) {
        commandPreviewEdit->setPlainText(QString());
        return;
    }

    QVector<QPair<QString, QString>> functions;
    functions.reserve(functionEdits.size());
    for (int i = 0; i < functionEdits.size(); ++i) {
        // 保留参数首尾空格的原始内容；只用 trimmed() 判断这一格是否为空。
        functions.append({functionEdits[i]->text(), parameterEdits[i]->text()});
    }
    const QString command = RustBackend::buildCommand(program, functions, getEnabledStates(), otherArgsEdit->text());
    commandPreviewEdit->setPlainText(command);
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
    if (auto *window = qobject_cast<AppWindow*>(parent)) originalTheme = window->getCurrentTheme();
    connect(this, &QDialog::rejected, this, [this]() {
        if (auto *window = qobject_cast<AppWindow*>(this->parent())) window->onThemeChanged(originalTheme);
    });
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
    languageCombo->setEnabled(false);
    languageCombo->setToolTip("翻译资源尚未提供，目前仅支持中文");
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
    m_loadingSettings = true;
    AppWindow* mainWindow = qobject_cast<AppWindow*>(parent());
    if (mainWindow) {
        QString currentTheme = mainWindow->getCurrentTheme();
        int index = themeCombo->findData(currentTheme);
        if (index >= 0) {
            themeCombo->setCurrentIndex(index);
        }
    }
    backupCombo->clear();
    for (const QString &name : RustBackend::getBackups(configFilePath(), backupDirPath())) backupCombo->addItem(name);
    restoreButton->setEnabled(backupCombo->count() > 0);
    m_loadingSettings = false;
}

void SettingsDialog::onThemeComboChanged(int index) {
    if (m_loadingSettings) return;  // Don't fire during loadSettings()
    QString theme = themeCombo->itemData(index).toString();
    // Emit signal or apply directly if we have a pointer to main window
    AppWindow* mainWindow = qobject_cast<AppWindow*>(parent());
    if (mainWindow) {
        mainWindow->onThemeChanged(theme);
    }
}

void SettingsDialog::onRestoreBackupClicked() {
    auto *window = qobject_cast<AppWindow*>(parent());
    if (!window || backupCombo->currentText().isEmpty()) return;
    if (QMessageBox::question(this, "恢复备份", "将用所选备份替换当前配置。恢复前会保留现有文件副本。继续？",
        QMessageBox::Yes | QMessageBox::No, QMessageBox::No) != QMessageBox::Yes) return;
    if (window->restoreBackup(backupCombo->currentText())) {
        originalTheme = window->getCurrentTheme();
        loadSettings();
    } else QMessageBox::warning(this, "恢复失败", "恢复未完成；现有配置已保留。");
}

void SettingsDialog::onOkClicked() {
    accept();
}

void SettingsDialog::onCancelClicked() {
    // 恢复原来的主题？目前只在接受后保存
    reject();
}

void SettingsDialog::onApplyClicked() {
    QString theme = themeCombo->currentData().toString();
    AppWindow* mainWindow = qobject_cast<AppWindow*>(parent());
    if (mainWindow) {
        mainWindow->onThemeChanged(theme);
        mainWindow->saveApplicationSettings();
        originalTheme = theme;
    }
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

#ifndef ECR_NO_MAIN
int main(int argc, char *argv[])
{
    QApplication app(argc, argv);
    app.setStyle(QStyleFactory::create("Fusion"));
    app.setApplicationName("EasyCommandRunner");
    app.setApplicationVersion("1.0.0");
    app.setWindowIcon(makeAppIcon());

    AppWindow window;
    return app.exec();
}
#endif
