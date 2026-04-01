#include "app.h"
#include "rust_backend.h"
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
#include <QEnterEvent>
#include <QPixmap>
#include <QPainter>
#include <QRegularExpression>
#include <QSize>
#include <QAbstractButton>
#include <QTabBar>
#include <QStyledItemDelegate>
#include <QListView>

namespace {

QString configFilePath()
{
    return QCoreApplication::applicationDirPath() + "/config.json";
}

QString backupDirPath()
{
    return QCoreApplication::applicationDirPath() + "/backup";
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
// Combo box item delegate: fixed row height
// ============================================================================

class ComboItemDelegate : public QStyledItemDelegate {
public:
    explicit ComboItemDelegate(int rowHeight, QObject *parent = nullptr)
        : QStyledItemDelegate(parent), m_rowHeight(rowHeight) {}
    QSize sizeHint(const QStyleOptionViewItem &option, const QModelIndex &index) const override {
        QSize s = QStyledItemDelegate::sizeHint(option, index);
        s.setHeight(m_rowHeight);
        return s;
    }
private:
    int m_rowHeight;
};

// ============================================================================
// CheckButton: fully custom-painted checkbox button (bypasses macOS native)
// ============================================================================

class CheckButton : public QPushButton {
public:
    explicit CheckButton(QWidget *parent = nullptr) : QPushButton(parent) {
        setCheckable(true);
        setChecked(true);
        setFixedSize(18, 18);
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
        p.setRenderHint(QPainter::Antialiasing);

        const QRectF r = QRectF(rect()).adjusted(0.5, 0.5, -0.5, -0.5);
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
            const float cx = width() / 2.0f;
            const float cy = height() / 2.0f;
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

// ============================================================================
// Shared helper: themed SVG icon
// ============================================================================

static QIcon makeThemedIcon(const QString &svgPath, const QString &theme) {
    QFile file(svgPath);
    if (!file.open(QFile::ReadOnly)) return QIcon();

    QString svgData = QString::fromUtf8(file.readAll());
    file.close();

    // Softer colors — not pure black/white
    const QString iconColor = (theme == "light") ? "#4B5563" : "#C1C2C5";

    svgData.replace("fill=\"currentColor\"",   QString("fill=\"%1\"").arg(iconColor));
    svgData.replace("stroke=\"currentColor\"", QString("stroke=\"%1\"").arg(iconColor));
    // Also replace any hardcoded fill hex so SVG looks right when re-themed
    QRegularExpression fillHex("fill=\"#[0-9a-fA-F]{6}\"");
    svgData.replace(fillHex, QString("fill=\"%1\"").arg(iconColor));

    QPixmap pixmap;
    pixmap.loadFromData(svgData.toUtf8(), "SVG");
    return QIcon(pixmap);
}

static QIcon makeAppIcon() {
    QIcon icon;
    icon.addFile(":/res/SleepyKanata.jpg");
    icon.addFile(":/res/icon2.ico");
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
    , runButton(nullptr)
    , trayIcon(nullptr)
    , trayMenu(nullptr)
    , settings(nullptr)
    , currentTheme("dark")
    , currentTabIndex(0)
{
    setWindowTitle("EasyCommandRunner");
    setWindowIcon(makeAppIcon());

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
    mainLayout->setContentsMargins(16, 16, 16, 8);
    mainLayout->setSpacing(8);

    // 创建标签页widget
    tabWidget = new QTabWidget(this);
    tabWidget->setTabsClosable(true);
    tabWidget->setMovable(true);
    tabWidget->setDocumentMode(false);

    // 下拉菜单选择标签
    tabCombo = new QComboBox(this);
    tabCombo->setObjectName("tabCombo");
    tabCombo->setItemDelegate(new ComboItemDelegate(32, tabCombo));
    tabWidget->setCornerWidget(tabCombo, Qt::TopRightCorner);
    connect(tabCombo, QOverload<int>::of(&QComboBox::activated), tabWidget, &QTabWidget::setCurrentIndex);

    mainLayout->addWidget(tabWidget);

    // 按钮布局 - 现代化设计
    QHBoxLayout *buttonLayout = new QHBoxLayout();
    buttonLayout->setContentsMargins(8, 8, 8, 8);
    buttonLayout->setSpacing(6);

    addTabButton = new QPushButton(this);
    addTabButton->setIcon(QIcon(":/res/icon_add.svg"));
    addTabButton->setText("新建");
    addTabButton->setObjectName("addTabBtn");
    addTabButton->setMinimumWidth(80);
    addTabButton->setMinimumHeight(36);
    addTabButton->setIconSize(QSize(20, 20));

    reloadButton = new QPushButton(this);
    reloadButton->setIcon(QIcon(":/res/icon_refresh.svg"));
    reloadButton->setText("重载");
    reloadButton->setObjectName("reloadBtn");
    reloadButton->setMinimumWidth(80);
    reloadButton->setMinimumHeight(36);
    reloadButton->setIconSize(QSize(20, 20));

    copyTabButton = new QPushButton(this);
    copyTabButton->setIcon(QIcon(":/res/icon_copy.svg"));
    copyTabButton->setText("复制");
    copyTabButton->setObjectName("copyTabBtn");
    copyTabButton->setMinimumWidth(80);
    copyTabButton->setMinimumHeight(36);
    copyTabButton->setIconSize(QSize(20, 20));

    previewButton = new QPushButton(this);
    previewButton->setIcon(QIcon(":/res/icon_preview.svg"));
    previewButton->setText("预览");
    previewButton->setObjectName("previewBtn");
    previewButton->setMinimumWidth(80);
    previewButton->setMinimumHeight(36);
    previewButton->setIconSize(QSize(20, 20));

    saveButton = new QPushButton(this);
    saveButton->setIcon(QIcon(":/res/icon_save.svg"));
    saveButton->setText("保存");
    saveButton->setObjectName("saveBtn");
    saveButton->setMinimumWidth(80);
    saveButton->setMinimumHeight(36);
    saveButton->setIconSize(QSize(20, 20));

    runButton = new QPushButton(this);
    runButton->setIcon(QIcon(":/res/icon_run.svg"));
    runButton->setText("运行");
    runButton->setObjectName("runBtn");
    runButton->setMinimumWidth(80);
    runButton->setMinimumHeight(36);
    runButton->setIconSize(QSize(20, 20));

    settingsButton = new QPushButton(this);
    settingsButton->setIcon(QIcon(":/res/icon_settings.svg"));
    settingsButton->setText("设置");
    settingsButton->setObjectName("settingsBtn");
    settingsButton->setMinimumWidth(80);
    settingsButton->setMinimumHeight(36);
    settingsButton->setIconSize(QSize(20, 20));

    buttonLayout->addWidget(addTabButton);
    buttonLayout->addWidget(reloadButton);
    buttonLayout->addWidget(copyTabButton);
    buttonLayout->addWidget(previewButton);
    buttonLayout->addStretch();
    buttonLayout->addWidget(saveButton);
    buttonLayout->addWidget(runButton);
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
    if (saveConfiguration()) {
        QMessageBox::information(this, "成功", "配置已保存！");
    }
}

void AppWindow::onReloadConfigClicked() {
    loadConfiguration();
    QMessageBox::information(this, "成功", "配置已重新加载！");
}

void AppWindow::onCopyTabConfigClicked() {
    CommandTab *currentTab = qobject_cast<CommandTab*>(tabWidget->currentWidget());
    if (!currentTab) return;

    const QString sourceName = currentTab->getTabName().trimmed();
    const QString newName = sourceName.isEmpty()
        ? QString("标签%1").arg(tabWidget->count() + 1)
        : sourceName + " - 副本";

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

QIcon AppWindow::createThemedIcon(const QString &svgPath, const QString &theme) {
    return makeThemedIcon(svgPath, theme);
}

void AppWindow::updateButtonIcons() {
    // 根据当前主题更新所有按钮的图标
    addTabButton->setIcon(createThemedIcon(":/res/icon_add.svg", currentTheme));
    reloadButton->setIcon(createThemedIcon(":/res/icon_refresh.svg", currentTheme));
    copyTabButton->setIcon(createThemedIcon(":/res/icon_copy.svg", currentTheme));
    previewButton->setIcon(createThemedIcon(":/res/icon_preview.svg", currentTheme));
    saveButton->setIcon(createThemedIcon(":/res/icon_save.svg", currentTheme));
    runButton->setIcon(createThemedIcon(":/res/icon_run.svg", currentTheme));
    settingsButton->setIcon(createThemedIcon(":/res/icon_settings.svg", currentTheme));

    // 更新所有标签页关闭按钮
    for (int i = 0; i < tabWidget->count(); ++i) {
        QAbstractButton *btn = qobject_cast<QAbstractButton*>(
            tabWidget->tabBar()->tabButton(i, QTabBar::RightSide));
        if (btn) {
            btn->setIcon(createThemedIcon(":/res/icon_close.svg", currentTheme));
        }
        // 更新参数行删除按钮
        CommandTab *tab = qobject_cast<CommandTab*>(tabWidget->widget(i));
        if (tab) {
            tab->updateRemoveButtonIcons(currentTheme);
        }
    }
}


void AppWindow::loadConfiguration() {
    const QString configJson = RustBackend::loadConfig(configFilePath(), backupDirPath());
    if (configJson.trimmed().isEmpty()) {
        if (tabWidget->count() == 0) {
            createTab(QStringLiteral("标签1"));
        }
        return;
    }

    QJsonParseError parseError;
    const QJsonDocument doc = QJsonDocument::fromJson(configJson.toUtf8(), &parseError);
    if (parseError.error != QJsonParseError::NoError || !doc.isObject()) {
        if (tabWidget->count() == 0) {
            createTab(QStringLiteral("标签1"));
        }
        return;
    }

    const QJsonObject config = doc.object();

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

    int savedIndex = config.value("current_tab_index").toInt(currentTabIndex);
    if (savedIndex < 0 || savedIndex >= tabWidget->count()) {
        savedIndex = 0;
    }
    tabWidget->setCurrentIndex(savedIndex);
    currentTabIndex = savedIndex;
    updateTabCombo();
}

bool AppWindow::saveConfiguration() {
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

    const QJsonDocument doc(config);
    if (!RustBackend::saveConfig(configFilePath(), backupDirPath(), QString::fromUtf8(doc.toJson(QJsonDocument::Indented)))) {
        QMessageBox::warning(this, "错误", "保存配置失败");
        return false;
    }

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
    int index = tabWidget->addTab(tab, name);

    // Apply theme icons to the new tab's param rows immediately
    tab->updateRemoveButtonIcons(currentTheme);

    // Set a proper themed close button instead of the QSS-controlled one
    QPushButton *closeBtn = createTabCloseButton();
    tabWidget->tabBar()->setTabButton(index, QTabBar::RightSide, closeBtn);
    connect(closeBtn, &QPushButton::clicked, [this, closeBtn]() {
        // Find which tab this button belongs to
        for (int i = 0; i < tabWidget->tabBar()->count(); ++i) {
            if (tabWidget->tabBar()->tabButton(i, QTabBar::RightSide) == closeBtn) {
                onCloseTab(i);
                break;
            }
        }
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

QPushButton* AppWindow::createTabCloseButton() {
    QPushButton *btn = new QPushButton(this);
    btn->setIcon(createThemedIcon(":/res/icon_close.svg", currentTheme));
    btn->setFixedSize(22, 22);
    btn->setIconSize(QSize(12, 12));
    btn->setFlat(true);
    btn->setObjectName("tabCloseBtn");
    btn->setCursor(Qt::PointingHandCursor);
    btn->setToolTip("关闭标签页");
    return btn;
}

void AppWindow::updateTabCombo() {
    tabCombo->blockSignals(true);
    tabCombo->clear();
    for (int i = 0; i < tabWidget->count(); ++i) {
        tabCombo->addItem(tabWidget->tabText(i));
    }
    tabCombo->setCurrentIndex(tabWidget->currentIndex());
    tabCombo->blockSignals(false);
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

void CommandTab::loadConfiguration(const QJsonObject &config) {
    titleEdit->setText(config.value("name").toString());
    workingDirEdit->setText(config.value("working_dir").toString());
    programEdit->setText(config.value("program").toString());
    otherArgsEdit->setText(config.value("other_args").toString());
    descriptionEdit->setText(config.value("description").toString());

    // 清除现有参数行（保留第一行占位符，先全清）
    // Remove all existing rows from the layout
    while (functionEdits.size() > 0) {
        QLineEdit *fe = functionEdits.takeLast();
        QLineEdit *pe = parameterEdits.takeLast();
        QLineEdit *ce = commentEdits.size() > 0 ? commentEdits.takeLast() : nullptr;
        QPushButton *rb = removeButtons.size() > 0 ? removeButtons.takeLast() : nullptr;
        QPushButton *cb = rowCheckBoxes.size() > 0 ? rowCheckBoxes.takeLast() : nullptr;
        // Find and remove the parent rowWidget
        if (fe->parentWidget()) {
            QWidget *row = fe->parentWidget();
            functionsLayout->removeWidget(row);
            row->deleteLater();
        }
        Q_UNUSED(pe); Q_UNUSED(ce); Q_UNUSED(rb); Q_UNUSED(cb);
    }

    rowCheckBoxes.clear();
    removeButtons.clear();
    commentEdits.clear();
    parameterEdits.clear();
    functionEdits.clear();

    // 加载函数列表
    QJsonArray functions = config.value("functions").toArray();
    for (const auto &func : functions) {
        QJsonObject funcObj = func.toObject();
        onAddFunctionClicked();
        int last = functionEdits.size() - 1;
        functionEdits[last]->setText(funcObj.value("function").toString());
        parameterEdits[last]->setText(funcObj.value("parameter").toString());
        if (last < commentEdits.size()) {
            commentEdits[last]->setText(funcObj.value("comment").toString());
        }
    }

    // 若没有任何行则保留一个空行
    if (functionEdits.isEmpty()) {
        onAddFunctionClicked();
    }
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
    functionEdits.clear();
    parameterEdits.clear();
    commentEdits.clear();
}

void CommandTab::onParseCommandClicked() {
    // 实现命令解析
}

void CommandTab::onAddFunctionClicked() {
    // 行内水平布局：左右各 4px 边距（和 spacing 对称），上下 0（由 rowWidget 固定高度控制）
    QHBoxLayout *rowLayout = new QHBoxLayout();
    rowLayout->setContentsMargins(4, 0, 4, 0);
    rowLayout->setSpacing(6);

    // 多选框：自绘 CheckButton，完全绕过 macOS native button 渲染
    CheckButton *checkBox = new CheckButton();
    checkBox->setTheme(m_theme);

    QLineEdit *funcEdit = new QLineEdit();
    funcEdit->setPlaceholderText("参数名");
    funcEdit->setObjectName("paramFuncEdit");
    funcEdit->setFixedHeight(28);

    QLineEdit *paramEdit = new QLineEdit();
    paramEdit->setPlaceholderText("参数值");
    paramEdit->setObjectName("paramValueEdit");
    paramEdit->setFixedHeight(28);

    QLineEdit *commentEdit = new QLineEdit();
    commentEdit->setPlaceholderText("备注");
    commentEdit->setObjectName("paramCommentEdit");
    commentEdit->setFixedHeight(28);

    QPushButton *removeBtn = new QPushButton();
    removeBtn->setIcon(makeThemedIcon(":/res/icon_delete.svg", m_theme));
    removeBtn->setFixedSize(26, 26);
    removeBtn->setIconSize(QSize(13, 13));
    removeBtn->setToolTip("删除此参数");
    removeBtn->setCursor(Qt::PointingHandCursor);
    removeBtn->setObjectName("paramRemoveBtn");

    // 垂直居中对齐所有 widget
    rowLayout->setAlignment(Qt::AlignVCenter);
    rowLayout->addWidget(checkBox, 0, Qt::AlignVCenter);
    // 参数名:参数值:备注 = 2:4:2
    rowLayout->addWidget(funcEdit, 2);
    rowLayout->addWidget(paramEdit, 4);
    rowLayout->addWidget(commentEdit, 2);
    rowLayout->addWidget(removeBtn, 0, Qt::AlignVCenter);

    rowCheckBoxes.append(checkBox);
    functionEdits.append(funcEdit);
    parameterEdits.append(paramEdit);
    commentEdits.append(commentEdit);
    removeButtons.append(removeBtn);

    QWidget *rowWidget = new QWidget();
    rowWidget->setLayout(rowLayout);
    rowWidget->setObjectName(QString("paramRow_%1").arg(functionCounter++));
    // 固定行高：28px 内容 + 0 margins = 36px 给足空间不截字
    rowWidget->setFixedHeight(36);

    // 勾选变化时实时更新预览
    connect(checkBox, &QPushButton::toggled, this, &CommandTab::updateCommandPreview);
    // 参数名/值变化时也实时更新预览
    connect(funcEdit,  &QLineEdit::textChanged, this, &CommandTab::updateCommandPreview);
    connect(paramEdit, &QLineEdit::textChanged, this, &CommandTab::updateCommandPreview);

    // Insert before the stretch
    functionsLayout->insertWidget(functionsLayout->count() - 1, rowWidget);

    connect(removeBtn, &QPushButton::clicked, [this, rowWidget, checkBox, funcEdit, paramEdit, commentEdit, removeBtn]() {
        rowCheckBoxes.removeOne(checkBox);
        functionEdits.removeOne(funcEdit);
        parameterEdits.removeOne(paramEdit);
        commentEdits.removeOne(commentEdit);
        removeButtons.removeOne(removeBtn);
        functionsLayout->removeWidget(rowWidget);
        rowWidget->deleteLater();
        updateCommandPreview();
    });
}

void CommandTab::updateRemoveButtonIcons(const QString &theme) {
    m_theme = theme;
    QIcon icon = makeThemedIcon(":/res/icon_delete.svg", theme);
    for (QPushButton *btn : removeButtons) {
        btn->setIcon(icon);
    }
    // 同步 CheckButton 主题色
    for (QPushButton *cb : rowCheckBoxes) {
        static_cast<CheckButton*>(cb)->setTheme(theme);
    }
    // 同步参数操作按钮图标
    if (addFunctionButton)
        addFunctionButton->setIcon(makeThemedIcon(":/res/icon_add.svg", theme));
    if (selectAllButton)
        selectAllButton->setIcon(makeThemedIcon(":/res/icon_select_all.svg", theme));
    if (deselectAllButton)
        deselectAllButton->setIcon(makeThemedIcon(":/res/icon_deselect_all.svg", theme));
    if (previewButton)
        previewButton->setIcon(makeThemedIcon(":/res/icon_preview.svg", theme));
}

void CommandTab::onRemoveFunctionClicked() {
    // 已经通过 lambda 在 onAddFunctionClicked 实现了单行删除
}

void CommandTab::onPreviewCommandClicked() {
    updateCommandPreview();
}

void CommandTab::setupUI() {
    QVBoxLayout *layout = new QVBoxLayout(this);
    layout->setContentsMargins(12, 12, 12, 12);
    layout->setSpacing(8);

    // ===== 标题行 =====
    QHBoxLayout *titleLayout = new QHBoxLayout();
    titleLayout->setSpacing(8);
    QLabel *titleLabel = new QLabel("标题:");
    titleLabel->setMinimumWidth(80);
    titleLabel->setMaximumWidth(80);
    titleEdit = new QLineEdit();
    titleLayout->addWidget(titleLabel);
    titleLayout->addWidget(titleEdit);
    layout->addLayout(titleLayout);

    // ===== 运行路径行 =====
    QHBoxLayout *dirLayout = new QHBoxLayout();
    dirLayout->setSpacing(8);
    QLabel *dirLabel = new QLabel("运行路径:");
    dirLabel->setMinimumWidth(80);
    dirLabel->setMaximumWidth(80);
    workingDirEdit = new QLineEdit();
    workingDirEdit->setPlaceholderText("为空则使用程序当前目录");
    dirLayout->addWidget(dirLabel);
    dirLayout->addWidget(workingDirEdit);
    layout->addLayout(dirLayout);

    // ===== 程序本体行 =====
    QHBoxLayout *programLayout = new QHBoxLayout();
    programLayout->setSpacing(8);
    QLabel *programLabel = new QLabel("程序本体:");
    programLabel->setMinimumWidth(80);
    programLabel->setMaximumWidth(80);
    programEdit = new QLineEdit();
    programEdit->setPlaceholderText("粘贴命令可以解析");
    parseButton = new QPushButton("解析");
    parseButton->setMaximumWidth(80);
    parseButton->setMinimumHeight(32);
    programLayout->addWidget(programLabel);
    programLayout->addWidget(programEdit);
    programLayout->addWidget(parseButton);
    layout->addLayout(programLayout);

    // ===== 函数/参数区域（使用滚动区，更紧凑）=====
    QLabel *functionsLabel = new QLabel("参数配置:");
    functionsLabel->setStyleSheet("font-weight: bold;");
    layout->addWidget(functionsLabel);
    
    QScrollArea *scrollArea = new QScrollArea();
    scrollArea->setObjectName("paramsScrollArea");
    scrollArea->setMinimumHeight(150);
    scrollArea->setFrameShape(QFrame::NoFrame);
    scrollArea->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);

    QWidget *scrollWidget = new QWidget();
    functionsLayout = new QVBoxLayout(scrollWidget);
    functionsLayout->setContentsMargins(6, 4, 6, 4);
    functionsLayout->setSpacing(4);

    // stretch 先加，后续 insertWidget(count-1) 永远插在它前面
    functionsLayout->addStretch();

    // 默认添加一行
    onAddFunctionClicked();

    scrollArea->setWidget(scrollWidget);
    scrollArea->setWidgetResizable(true);

    QHBoxLayout *paramActionLayout = new QHBoxLayout();
    paramActionLayout->setSpacing(6);
    paramActionLayout->setContentsMargins(0, 2, 0, 2);

    addFunctionButton = new QPushButton("增加参数行");
    addFunctionButton->setIcon(makeThemedIcon(":/res/icon_add.svg", m_theme));
    addFunctionButton->setIconSize(QSize(15, 15));
    addFunctionButton->setMinimumHeight(30);

    selectAllButton = new QPushButton("全选");
    selectAllButton->setIcon(makeThemedIcon(":/res/icon_select_all.svg", m_theme));
    selectAllButton->setIconSize(QSize(15, 15));
    selectAllButton->setMinimumHeight(30);

    deselectAllButton = new QPushButton("取消全选");
    deselectAllButton->setIcon(makeThemedIcon(":/res/icon_deselect_all.svg", m_theme));
    deselectAllButton->setIconSize(QSize(15, 15));
    deselectAllButton->setMinimumHeight(30);

    paramActionLayout->addWidget(addFunctionButton);
    paramActionLayout->addWidget(selectAllButton);
    paramActionLayout->addWidget(deselectAllButton);
    paramActionLayout->addStretch();

    QWidget *paramSection = new QWidget(this);
    paramSection->setMinimumHeight(188);
    QVBoxLayout *paramSectionLayout = new QVBoxLayout(paramSection);
    paramSectionLayout->setContentsMargins(0, 0, 0, 0);
    paramSectionLayout->setSpacing(4);
    paramSectionLayout->addWidget(scrollArea);
    paramSectionLayout->addLayout(paramActionLayout);

    QWidget *detailsSection = new QWidget(this);
    detailsSection->setMinimumHeight(120);
    QVBoxLayout *detailsLayout = new QVBoxLayout(detailsSection);
    detailsLayout->setContentsMargins(0, 0, 0, 0);
    detailsLayout->setSpacing(8);

    QSplitter *paramSplitter = new QSplitter(Qt::Vertical, this);
    paramSplitter->setObjectName("paramContentSplitter");
    paramSplitter->setChildrenCollapsible(false);
    paramSplitter->setHandleWidth(10);

    // ===== 其他参数行 =====
    QHBoxLayout *otherLayout = new QHBoxLayout();
    otherLayout->setSpacing(8);
    QLabel *otherLabel = new QLabel("其他参数:");
    otherLabel->setMinimumWidth(80);
    otherLabel->setMaximumWidth(80);
    otherArgsEdit = new QLineEdit();
    otherArgsEdit->setPlaceholderText("额外的命令行参数");
    otherLayout->addWidget(otherLabel);
    otherLayout->addWidget(otherArgsEdit);
    detailsLayout->addLayout(otherLayout);

    // ===== 描述 =====
    QLabel *descLabel = new QLabel("描述:");
    descLabel->setStyleSheet("font-weight: bold;");
    detailsLayout->addWidget(descLabel);
    descriptionEdit = new QTextEdit();
    descriptionEdit->setPlaceholderText("输入命令描述信息");
    descriptionEdit->setMaximumHeight(80);
    descriptionEdit->setMinimumHeight(0);
    detailsLayout->addWidget(descriptionEdit);

    // ===== 命令预览 =====
    QLabel *previewLabel = new QLabel("预览命令:");
    previewLabel->setStyleSheet("font-weight: bold;");
    detailsLayout->addWidget(previewLabel);
    commandPreviewEdit = new QTextEdit();
    commandPreviewEdit->setReadOnly(true);
    commandPreviewEdit->setPlaceholderText("最终执行的完整命令");
    commandPreviewEdit->setMaximumHeight(100);
    commandPreviewEdit->setMinimumHeight(0);
    detailsLayout->addWidget(commandPreviewEdit);

    // ===== 预生成命令按钮 =====
    QHBoxLayout *previewButtonLayout = new QHBoxLayout();
    previewButtonLayout->setSpacing(6);
    previewButtonLayout->setContentsMargins(0, 2, 0, 2);

    previewButton = new QPushButton("预生成命令");
    previewButton->setIcon(makeThemedIcon(":/res/icon_preview.svg", m_theme));
    previewButton->setIconSize(QSize(15, 15));
    previewButton->setMinimumHeight(30);

    previewButtonLayout->addStretch();
    previewButtonLayout->addWidget(previewButton);

    detailsLayout->addLayout(previewButtonLayout);
    detailsLayout->addStretch();

    paramSplitter->addWidget(paramSection);
    paramSplitter->addWidget(detailsSection);
    paramSplitter->setStretchFactor(0, 0);
    paramSplitter->setStretchFactor(1, 1);
    paramSplitter->setSizes(QList<int>() << 220 << 320);

    layout->addWidget(paramSplitter, 1);
}

void CommandTab::setupConnections() {
    connect(parseButton, &QPushButton::clicked, this, &CommandTab::onParseCommandClicked);
    connect(addFunctionButton, &QPushButton::clicked, this, &CommandTab::onAddFunctionClicked);
    connect(previewButton, &QPushButton::clicked, this, &CommandTab::onPreviewCommandClicked);
    connect(titleEdit, &QLineEdit::textChanged, this, &CommandTab::titleChanged);

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
    QString program = programEdit->text().trimmed();
    if (program.isEmpty()) {
        commandPreviewEdit->setPlainText(QString());
        return;
    }

    QStringList parts;
    parts << program;

    for (int i = 0; i < rowCheckBoxes.size(); ++i) {
        if (!rowCheckBoxes[i] || !rowCheckBoxes[i]->isChecked()) continue;
        QString func  = (i < functionEdits.size())  ? functionEdits[i]->text().trimmed()  : QString();
        QString param = (i < parameterEdits.size())  ? parameterEdits[i]->text().trimmed() : QString();
        if (!func.isEmpty())  parts << func;
        if (!param.isEmpty()) parts << param;
    }

    QString other = otherArgsEdit->text().trimmed();
    if (!other.isEmpty()) parts << other;

    commandPreviewEdit->setPlainText(parts.join(' '));
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
    m_loadingSettings = true;
    AppWindow* mainWindow = qobject_cast<AppWindow*>(parent());
    if (mainWindow) {
        QString currentTheme = mainWindow->getCurrentTheme();
        int index = themeCombo->findData(currentTheme);
        if (index >= 0) {
            themeCombo->setCurrentIndex(index);
        }
    }
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
    // 恢复备份
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

int main(int argc, char *argv[])
{
    QApplication app(argc, argv);
    app.setApplicationName("EasyCommandRunner");
    app.setApplicationVersion("1.0.0");
    app.setWindowIcon(makeAppIcon());

    AppWindow window;
    return app.exec();
}
