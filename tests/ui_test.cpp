#include "app.h"
#include "log_panel.h"
#include "widget_helpers.h"

#include <QApplication>
#include <QComboBox>
#include <QDir>
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QMenuBar>
#include <QPlainTextEdit>
#include <QProcess>
#include <QRegularExpression>
#include <QScrollArea>
#include <QTemporaryDir>
#include <QTest>
#include <QTextDocument>
#include <QThread>
#include <iostream>
#ifdef Q_OS_UNIX
#include <cerrno>
#include <signal.h>
#include <unistd.h>
#endif

namespace {
QString childCommand(const QString &mode) {
    return "\"" + QDir::toNativeSeparators(QCoreApplication::applicationFilePath()) + "\" " + mode;
}

QPushButton *buttonWithText(QWidget *root, const QString &text) {
    for (QPushButton *button : root->findChildren<QPushButton *>()) {
        if (button->text() == text)
            return button;
    }
    return nullptr;
}
} // namespace

class UiTest : public QObject {
    Q_OBJECT
  private:
    std::unique_ptr<QTemporaryDir> data;

  private slots:
    void init() {
        data = std::make_unique<QTemporaryDir>();
        QVERIFY(data->isValid());
        qputenv("ECR_DATA_DIR", data->path().toUtf8());
    }

    void darkLayoutAndAssets() {
        QFile file(data->filePath("config.json"));
        QVERIFY(file.open(QIODevice::WriteOnly));
        QJsonArray rows;
        rows.append(QJsonObject{{"function", "-i"}, {"parameter", "input.mp4"}, {"comment", "输入文件"}});
        for (int i = 0; i < 3; ++i)
            rows.append(QJsonObject());
        QJsonObject config{
            {"theme", "dark"},
            {"tabs", QJsonArray{QJsonObject{{"name", "ffmpeg"}, {"program", "ffmpeg"}, {"functions", rows}}}}};
        file.write(QJsonDocument(config).toJson());
        file.close();
        AppWindow window;
#ifdef Q_OS_MACOS
        // offscreen 插件没有系统菜单栏；隐藏它以模拟实际 macOS 客户区。
        window.menuBar()->hide();
#endif
        window.resize(850, 700);
        QTest::qWait(150);
        auto *tabs = window.findChild<QTabWidget *>();
        auto *tab = qobject_cast<CommandTab *>(tabs->currentWidget());
        auto *log = window.findChild<LogPanel *>();
        QVERIFY(tab);
        QVERIFY(log->isHidden());
        QCOMPARE(window.size(), QSize(850, 700));
        auto *run = tab->findChild<QPushButton *>("runBtn");
        auto *save = buttonWithText(&window, "保存");
        QVERIFY(run && save);
        QVERIFY(run->mapTo(&window, QPoint()).y() < save->mapTo(&window, QPoint()).y());
        QVERIFY(!run->icon().isNull());
        auto *left = window.findChild<QPushButton *>("previousTabBtn");
        auto *right = window.findChild<QPushButton *>("nextTabBtn");
        QVERIFY(left && right);
        QVERIFY(left->mapTo(&window, QPoint()).x() < tabs->mapTo(&window, QPoint()).x());
        QVERIFY(right->mapTo(&window, QPoint()).x() >= tabs->mapTo(&window, QPoint()).x() + tabs->width());
        QVERIFY(!left->isEnabled());
        QVERIFY(!QPixmap(":/classic/checked_image.png").isNull());
        QVERIFY(!QPixmap(":/classic/close-button.png").isNull());
        const QImage appIcon(":/res/app_icon.png");
        QVERIFY(!appIcon.isNull());
        QCOMPARE(appIcon.pixelColor(0, 0).alpha(), 0);
        const QImage image = window.grab().toImage();
        QCOMPARE(image.pixelColor(8, 8), QColor("#121414"));
        QCOMPARE(run->grab().toImage().pixelColor(7, 7), QColor("#1a263b"));
        QCOMPARE(tab->getCommand(), QString("ffmpeg -i input.mp4"));

        // 可选快照：仅导出测试生成的虚构配置，不触及用户桌面或真实配置。
        const QString artifactDir = qEnvironmentVariable("ECR_UI_ARTIFACT_DIR");
        if (!artifactDir.isEmpty()) {
            QDir().mkpath(artifactDir);
            image.save(artifactDir + "/classic-qt6.png");
            QJsonArray geometry;
            for (auto *widget : window.findChildren<QWidget *>()) {
                if (!widget->isVisible())
                    continue;
                const QPoint position = widget->mapTo(&window, QPoint());
                QJsonObject item{
                    {"class", widget->metaObject()->className()},
                    {"name", widget->objectName()},
                    {"geometry", QJsonArray{position.x(), position.y(), widget->width(), widget->height()}}};
                if (auto *edit = qobject_cast<QLineEdit *>(widget))
                    item["text"] = edit->text();
                if (auto *label = qobject_cast<QLabel *>(widget))
                    item["text"] = label->text();
                if (auto *button = qobject_cast<QPushButton *>(widget))
                    item["text"] = button->text();
                geometry.append(item);
            }
            QFile output(artifactDir + "/classic-qt6-geometry.json");
            QVERIFY(output.open(QIODevice::WriteOnly));
            output.write(QJsonDocument(geometry).toJson());
        }
    }

    void rowsPreviewAndTabButtons() {
        AppWindow window;
        auto *tabs = window.findChild<QTabWidget *>();
        auto *tab = qobject_cast<CommandTab *>(tabs->currentWidget());
        tab->findChild<QLineEdit *>("programEdit")->setText("echo");
        tab->findChild<QLineEdit *>("paramValueEdit")->setText("hello");
        QCOMPARE(tab->getCommand(), QString("echo hello"));
        auto *checkbox = tab->findChild<QPushButton *>("paramCheckBox");
        checkbox->click();
        QCOMPARE(tab->getCommand(), QString("echo"));
        checkbox->click();
        tab->onAddFunctionClicked();
        QCOMPARE(tab->getFunctions().size(), 2);
        tab->findChildren<QPushButton *>("paramRemoveBtn").last()->click();
        QCOMPARE(tab->getFunctions().size(), 1);
        QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
        buttonWithText(&window, "新建标签")->click();
        QCOMPARE(tabs->count(), 2);
        window.findChild<QPushButton *>("previousTabBtn")->click();
        QCOMPARE(tabs->currentIndex(), 0);
        window.findChild<QPushButton *>("nextTabBtn")->click();
        QCOMPARE(tabs->currentIndex(), 1);
        window.onThemeChanged("light");
        window.onThemeChanged("dark");
        QVERIFY(!window.findChild<LogPanel *>()->isVisible());
    }

    void checkboxPersistenceAndCopy() {
        AppWindow window;
        auto *tabs = window.findChild<QTabWidget *>();
        auto *tab = qobject_cast<CommandTab *>(tabs->currentWidget());
        tab->findChild<QLineEdit *>("programEdit")->setText("echo");
        tab->findChild<QLineEdit *>("paramValueEdit")->setText("hello");
        tab->findChild<QPushButton *>("paramCheckBox")->setChecked(false);
        const auto config = tab->saveConfiguration();
        QVERIFY(!config["functions"].toArray()[0].toObject()["enabled"].toBool());
        buttonWithText(&window, "复制标签")->click();
        auto *copy = qobject_cast<CommandTab *>(tabs->currentWidget());
        QCOMPARE(copy->getCommand(), QString("echo"));
        QVERIFY(!copy->findChild<QPushButton *>("paramCheckBox")->isChecked());
        QCOMPARE(copy->findChild<QLineEdit *>("paramValueEdit")->text(), QString("hello"));
        buttonWithText(&window, "保存")->click();
        QVERIFY(QMetaObject::invokeMethod(&window, "onReloadConfigClicked"));
        QCOMPARE(tabs->count(), 2);
        QCOMPARE(tabs->currentIndex(), 1);
        QCOMPARE(qobject_cast<CommandTab *>(tabs->currentWidget())->getCommand(), QString("echo"));
    }

    void parseAndAppendRoundTrip() {
        CommandTab tab;
        QVERIFY(tab.parseCommandText("ffmpeg -i 'input file.mp4' -y -n -1"));
        QCOMPARE(tab.getProgram(), QString("ffmpeg"));
        QCOMPARE(tab.getCommand(), QString("ffmpeg -i 'input file.mp4' -y -n -1"));
        QVERIFY(tab.parseCommandText("-c:v libx264 -an", true));
        QVERIFY(tab.getCommand().endsWith("-c:v libx264 -an"));
        const auto before = tab.saveConfiguration();
        QVERIFY(!tab.parseCommandText("ffmpeg 'incomplete"));
        QCOMPARE(tab.saveConfiguration(), before);
        tab.clear();
        QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
        QCOMPARE(tab.getFunctions().size(), 1);
        QCOMPARE(tab.findChildren<QLineEdit *>("paramValueEdit").size(), 1);
        QCOMPARE(tab.getCommand(), QString());
    }

    void pathClipboardAndLiteralDescription() {
        PathLineEdit edit;
        auto *mime = new QMimeData();
        const QString path = data->filePath("path with spaces.txt");
        mime->setUrls({QUrl::fromLocalFile(path)});
        QApplication::clipboard()->setMimeData(mime);
        QTest::keyClick(&edit, Qt::Key_V, Qt::ControlModifier);
        QCOMPARE(edit.text(), path);
        CommandTab tab;
        tab.loadConfiguration(QJsonObject{{"program", "echo"},
                                          {"description", "<b>not html</b>"},
                                          {"functions", QJsonArray{QJsonObject{{"parameter", "hello world"}}}}});
        QCOMPARE(tab.getDescription(), QString("<b>not html</b>"));
#ifdef Q_OS_WIN
        QCOMPARE(tab.getCommand(), QString("echo \"hello world\""));
#else
        QCOMPARE(tab.getCommand(), QString("echo 'hello world'"));
#endif
    }

    void legacyConfigurationMigration() {
        QFile file(data->filePath("config.json"));
        QVERIFY(file.open(QIODevice::WriteOnly));
        file.write(
            R"({"tabs":[{"name_edit_title":"old","name_edit2":"echo","name_edit3_1":"hello","function3":"world","parameter3":"value"}],"line_codes":[{"3":3}],"checkbox_statuses":[{"chkbox1":false,"chkbox3":true}]})");
        file.close();
        AppWindow window;
        auto *tab = qobject_cast<CommandTab *>(window.findChild<QTabWidget *>()->currentWidget());
        QCOMPARE(tab->getTabName(), QString("old"));
        QCOMPARE(tab->getFunctions().size(), 2);
        QCOMPARE(tab->getCommand(), QString("echo world value"));
    }

    void backupRestoreAndThemeCancel() {
        AppWindow window;
        buttonWithText(&window, "保存")->click();
        const QByteArray oldData = [&]() {
            QFile file(data->filePath("config.json"));
            file.open(QIODevice::ReadOnly);
            return file.readAll();
        }();
        QFile backup(data->filePath("backup/test.json"));
        QVERIFY(backup.open(QIODevice::WriteOnly));
        backup.write(R"({"tabs":[{"name":"restored","program":"echo","functions":[]}],"theme":"dark"})");
        backup.close();
        QVERIFY(window.restoreBackup("test.json"));
        auto *tab = qobject_cast<CommandTab *>(window.findChild<QTabWidget *>()->currentWidget());
        QCOMPARE(tab->getTabName(), QString("restored"));
        const auto safetyFiles = QDir(data->filePath("backup")).entryList({"before_restore_*.json"});
        QCOMPARE(safetyFiles.size(), 1);
        QFile safety(data->filePath("backup/" + safetyFiles[0]));
        QVERIFY(safety.open(QIODevice::ReadOnly));
        QCOMPARE(safety.readAll(), oldData);
        SettingsDialog settings(&window);
        window.onThemeChanged("light");
        settings.reject();
        QCOMPARE(window.getCurrentTheme(), QString("dark"));
    }

    void corruptedConfigIsNotOverwritten() {
        QFile file(data->filePath("config.json"));
        QVERIFY(file.open(QIODevice::WriteOnly));
        file.write("broken json");
        file.close();
        {
            AppWindow window;
        }
        QVERIFY(file.open(QIODevice::ReadOnly));
        QCOMPARE(file.readAll(), QByteArray("broken json"));
    }

    void streamHistoryAndDocking() {
        QMainWindow window;
        auto *log = new LogPanel(&window);
        window.addDockWidget(Qt::BottomDockWidgetArea, log);
        window.show();
        auto *history = log->findChild<QComboBox *>("runHistoryCombo");
        auto *output = log->findChild<QPlainTextEdit *>("runLogOutput");
        auto *detach = log->findChild<QPushButton *>("detachLogBtn");
        log->startCommand("first", childCommand("--emit-log"), data->path());
        QTRY_VERIFY(output->toPlainText().contains("stream-start"));
        QCOMPARE(log->runningCount(), 1); // 未退出就必须看到输出。
        detach->click();
        QVERIFY(log->isFloating());
        QCOMPARE(detach->text(), QString("停靠"));
        QTRY_COMPARE(log->runningCount(), 0);
        QVERIFY(output->toPlainText().contains("中文输出"));
        QVERIFY(output->toPlainText().contains("stderr-marker"));
        QVERIFY(output->toPlainText().contains("退出 7"));
        QVERIFY(!output->toPlainText().contains(QChar::ReplacementCharacter));
        const QString firstLog = output->toPlainText();
        detach->click();
        QVERIFY(!log->isFloating());
        log->startCommand("second", childCommand("--short-log"), data->path());
        QTRY_COMPARE(log->runningCount(), 0);
        QCOMPARE(history->count(), 2);
        QVERIFY(output->toPlainText().contains("second-output"));
        history->setCurrentIndex(0);
        QCOMPARE(output->toPlainText(), firstLog);
    }

    void concurrentRunsStopOnlySelected() {
        LogPanel log;
        log.startCommand("long", childCommand("--long-log"), data->path());
        auto *history = log.findChild<QComboBox *>("runHistoryCombo");
        auto *output = log.findChild<QPlainTextEdit *>("runLogOutput");
        QTRY_VERIFY(output->toPlainText().contains("long-start"));
        log.startCommand("independent", childCommand("--emit-log"), data->path());
        QTRY_VERIFY(output->toPlainText().contains("stream-start"));
        QCOMPARE(log.runningCount(), 2);
        history->setCurrentIndex(0);
        log.stopCurrent();
        QTRY_VERIFY(output->toPlainText().contains("已停止"));
        history->setCurrentIndex(1);
        QTRY_COMPARE(log.runningCount(), 0);
        QVERIFY(output->toPlainText().contains("退出 7"));
        QVERIFY(!output->toPlainText().contains("已停止"));
    }

    void stopAlsoKillsShellDescendants() {
#ifdef Q_OS_UNIX
        LogPanel log;
        log.startCommand("tree", childCommand("--tree-log"), data->path());
        auto *output = log.findChild<QPlainTextEdit *>("runLogOutput");
        const QRegularExpression pattern("descendant=(\\d+)");
        QTRY_VERIFY(pattern.match(output->toPlainText()).hasMatch());
        const pid_t child = pattern.match(output->toPlainText()).captured(1).toLongLong();
        QVERIFY(child > 0);
        log.stopCurrent();
        QTRY_COMPARE(log.runningCount(), 0);
        QTRY_VERIFY_WITH_TIMEOUT(::kill(child, 0) == -1 && errno == ESRCH, 5000);
#else
        QSKIP("Unix process-group test");
#endif
    }

    void invalidDirectoryAndWorkingDirectory() {
        LogPanel log;
        log.startCommand("bad", childCommand("--short-log"), data->filePath("missing"));
        QTRY_COMPARE(log.runningCount(), 0);
        auto *output = log.findChild<QPlainTextEdit *>("runLogOutput");
        QVERIFY(output->toPlainText().contains("[错误]"));
        log.startCommand("cwd", childCommand("--show-cwd"), data->path());
        QTRY_COMPARE(log.runningCount(), 0);
        QVERIFY(output->toPlainText().contains("child-cwd=" + QDir(data->path()).canonicalPath()));
    }

    void runButtonActuallyStartsShell() {
        AppWindow window;
        auto *tab = qobject_cast<CommandTab *>(window.findChild<QTabWidget *>()->currentWidget());
        tab->findChild<QLineEdit *>("programEdit")->setText(childCommand("--short-log"));
        auto *log = window.findChild<LogPanel *>();
        tab->findChild<QPushButton *>("runBtn")->click();
        QVERIFY(log->isVisible());
        QTRY_COMPARE(log->runningCount(), 0);
        QVERIFY(log->findChild<QPlainTextEdit *>()->toPlainText().contains("second-output"));
    }
};

int main(int argc, char **argv) {
    if (argc > 1) {
        const QByteArray mode(argv[1]);
        if (mode == "--emit-log") {
            std::cout << "stream-start\n" << std::flush;
            const QByteArray utf8 = QStringLiteral("中文输出\n").toUtf8();
            std::cout.write(utf8.constData(), 2).flush();
            QThread::msleep(80);
            std::cout.write(utf8.constData() + 2, utf8.size() - 2).flush();
            std::cerr << "stderr-marker\n" << std::flush;
            QThread::msleep(700);
            return 7;
        }
        if (mode == "--short-log") {
            std::cout << "second-output\n";
            return 0;
        }
        if (mode == "--long-log") {
            std::cout << "long-start\n" << std::flush;
            QThread::sleep(30);
            return 0;
        }
#ifdef Q_OS_UNIX
        if (mode == "--tree-log") {
            const pid_t child = ::fork();
            if (child == 0) {
                ::signal(SIGTERM, SIG_IGN);
                std::cout << "descendant=" << ::getpid() << '\n' << std::flush;
                ::sleep(30);
                ::_exit(0);
            }
            if (child < 0)
                return 1;
            ::sleep(30);
            return 0;
        }
#endif
        if (mode == "--show-cwd") {
            std::cout << "child-cwd=" << QDir::current().canonicalPath().toStdString() << '\n';
            return 0;
        }
    }
    QApplication app(argc, argv);
    app.setStyle("Fusion");
    UiTest test;
    return QTest::qExec(&test, argc, argv);
}

#include "ui_test.moc"
