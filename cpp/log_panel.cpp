#include "log_panel.h"

#include <algorithm>

#include <QApplication>
#include <QClipboard>
#include <QComboBox>
#include <QDateTime>
#include <QDir>
#include <QFileDialog>
#include <QFileInfo>
#include <QHBoxLayout>
#include <QLabel>
#include <QMessageBox>
#include <QPlainTextEdit>
#include <QPointer>
#include <QProcess>
#include <QPushButton>
#include <QSaveFile>
#include <QScrollBar>
#include <QSignalBlocker>
#include <QStringDecoder>
#include <QTextCursor>
#include <QTextDocument>
#include <QTimer>
#include <QVBoxLayout>

#ifdef Q_OS_UNIX
#include <pwd.h>
#include <signal.h>
#include <unistd.h>
#endif

namespace {
constexpr int maxRuns = 100;
constexpr int maxLogCharacters = 2 * 1024 * 1024;

QString defaultShell() {
#ifdef Q_OS_WIN
    return qEnvironmentVariable("COMSPEC", "cmd.exe");
#else
    QString shell = qEnvironmentVariable("SHELL");
    if (shell.isEmpty()) {
        if (const passwd *user = getpwuid(getuid())) {
            shell = QString::fromLocal8Bit(user->pw_shell);
        }
    }
    return QFileInfo(shell).isExecutable() ? shell : QStringLiteral("/bin/sh");
#endif
}
} // namespace

struct LogPanel::Run {
    QString caption;
    QProcess *process = nullptr;
    QTextDocument *document = nullptr;
    QStringDecoder decoder{QStringDecoder::Utf8};
    std::shared_ptr<qint64> processGroup = std::make_shared<qint64>(0);
    bool finished = false;
    bool stopping = false;
};

LogPanel::LogPanel(QWidget *parent) : QDockWidget("运行日志", parent) {
    setObjectName("runLogDock");
    setAllowedAreas(Qt::BottomDockWidgetArea);
    setFeatures(DockWidgetClosable | DockWidgetMovable | DockWidgetFloatable);

    // 明确的单击分离/停靠按钮，不依赖拖动或系统标题栏的小图标。
    QWidget *titleBar = new QWidget(this);
    QHBoxLayout *titleLayout = new QHBoxLayout(titleBar);
    titleLayout->setContentsMargins(9, 3, 9, 3);
    titleLayout->addWidget(new QLabel("运行日志", titleBar));
    titleLayout->addStretch();
    detachButton = new QPushButton("分离窗口", titleBar);
    detachButton->setObjectName("detachLogBtn");
    titleLayout->addWidget(detachButton);
    QPushButton *hideButton = new QPushButton("收起", titleBar);
    titleLayout->addWidget(hideButton);
    setTitleBarWidget(titleBar);

    QWidget *content = new QWidget(this);
    QVBoxLayout *layout = new QVBoxLayout(content);
    layout->setContentsMargins(9, 6, 9, 9);
    layout->setSpacing(6);
    QHBoxLayout *toolbar = new QHBoxLayout();
    historyCombo = new QComboBox(content);
    historyCombo->setObjectName("runHistoryCombo");
    historyCombo->setSizeAdjustPolicy(QComboBox::AdjustToMinimumContentsLengthWithIcon);
    historyCombo->setMinimumContentsLength(12);
    historyCombo->setToolTip(
        "本次会话最近 100 次运行；每次保留最多 10000 行 / 2 Mi 字符，可保存为文本。输出按 UTF-8 解码。");
    toolbar->addWidget(historyCombo, 1);
    stopButton = new QPushButton("停止", content);
    stopButton->setObjectName("stopLogBtn");
    stopButton->setEnabled(false);
    QPushButton *copyButton = new QPushButton("复制日志", content);
    QPushButton *saveButton = new QPushButton("保存日志", content);
    toolbar->addWidget(stopButton);
    toolbar->addWidget(copyButton);
    toolbar->addWidget(saveButton);
    layout->addLayout(toolbar);
    outputEdit = new QPlainTextEdit(content);
    outputEdit->setObjectName("runLogOutput");
    outputEdit->setReadOnly(true);
    outputEdit->setMinimumHeight(100);
    outputEdit->setPlaceholderText("每次运行都会启动独立 shell，输出和历史显示在这里。\n这是日志视图，不是交互终端。");
    layout->addWidget(outputEdit, 1);
    setWidget(content);

    connect(detachButton, &QPushButton::clicked, this, [this]() {
        setFloating(!isFloating());
        show();
        if (isFloating())
            resize(850, 360);
    });
    connect(this, &QDockWidget::topLevelChanged, this,
            [this](bool floating) { detachButton->setText(floating ? "停靠" : "分离窗口"); });
    connect(hideButton, &QPushButton::clicked, this, &QWidget::hide);
    connect(historyCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), this, &LogPanel::selectRun);
    connect(stopButton, &QPushButton::clicked, this, &LogPanel::stopCurrent);
    connect(copyButton, &QPushButton::clicked, this,
            [this]() { QApplication::clipboard()->setText(outputEdit->toPlainText()); });
    connect(saveButton, &QPushButton::clicked, this, &LogPanel::saveCurrent);
}

LogPanel::~LogPanel() {
    // 窗口隐藏、浮动/停靠都不会走到这里；只有应用真正退出才结束任务。
    for (const auto &run : runs) {
#ifdef Q_OS_UNIX
        if (run->stopping && *run->processGroup > 0)
            ::kill(-*run->processGroup, SIGKILL);
#endif
        *run->processGroup = 0;
        if (run->finished)
            continue;
        run->process->disconnect(this);
        if (run->process->state() == QProcess::Starting)
            run->process->waitForStarted(1000);
        stopRun(run.get());
#ifdef Q_OS_UNIX
        const qint64 pid = run->process->processId();
        if (pid > 0)
            ::kill(-pid, SIGKILL);
#endif
        if (!run->process->waitForFinished(1500)) {
            run->process->kill();
            run->process->waitForFinished(1000);
        }
    }
}

int LogPanel::runningCount() const {
    int count = 0;
    for (const auto &run : runs) {
        if (!run->finished)
            ++count;
    }
    return count;
}

void LogPanel::startCommand(const QString &title, const QString &command, const QString &workingDirectory) {
    if (command.trimmed().isEmpty())
        return;
    if (runs.size() >= maxRuns) {
        // 只回收已完成的历史，绝不丢弃正在执行的任务。
        auto oldest = std::find_if(runs.begin(), runs.end(), [](const auto &run) { return run->finished; });
        if (oldest == runs.end()) {
            QMessageBox::warning(this, "提示", "同时运行的任务已达 100 个，请先停止部分任务。");
            return;
        }
        const int index = static_cast<int>(std::distance(runs.begin(), oldest));
        (*oldest)->process->disconnect(this);
        (*oldest)->process->deleteLater();
        (*oldest)->document->deleteLater();
        {
            QSignalBlocker blocker(historyCombo);
            historyCombo->removeItem(index);
            runs.erase(oldest);
        }
    }

    auto ownedRun = std::make_unique<Run>();
    Run *run = ownedRun.get();
    run->caption = QDateTime::currentDateTime().toString("HH:mm:ss") + "  " +
                   (title.trimmed().isEmpty() ? command.left(60) : title);
    run->document = new QTextDocument(this);
    run->document->setDocumentLayout(new QPlainTextDocumentLayout(run->document));
    run->document->setMaximumBlockCount(10000);
    run->process = new QProcess(this);
    runs.push_back(std::move(ownedRun));
    historyCombo->addItem(run->caption + "  [启动中]");
    historyCombo->setCurrentIndex(static_cast<int>(runs.size()) - 1);
    selectRun(historyCombo->currentIndex());
    show();
    raise();

    const QString shell = defaultShell();
    const QString directory = workingDirectory.trimmed().isEmpty() ? QDir::currentPath() : workingDirectory;
    append(run, QString("[%1]\nShell: %2\n运行路径：%3\n$ %4\n\n")
                    .arg(QDateTime::currentDateTime().toString(Qt::ISODate), shell, directory, command));
    run->process->setWorkingDirectory(directory);
    run->process->setProgram(shell);
    run->process->setProcessChannelMode(QProcess::MergedChannels);
    run->process->setInputChannelMode(QProcess::ManagedInputChannel);
#ifdef Q_OS_WIN
    // Windows Terminal 是宿主，COMSPEC 才是 cmd 的选择；不启动外部终端窗口。
    // 让 Qt 负责把完整命令作为一个参数传给 cmd，避免手工拼 native 引号。
    run->process->setArguments({"/D", "/S", "/C", command});
#else
    run->process->setArguments({"-c", command});
    // 独立进程组，停止时同时结束 shell 派生的普通子进程。
    run->process->setChildProcessModifier([]() { ::setsid(); });
#endif
    connect(run->process, &QProcess::started, this, [this, run]() {
        *run->processGroup = run->process->processId();
        run->process->closeWriteChannel(); // 非交互日志任务，不让程序一直等待 stdin。
        updateRun(run, "运行中");
        if (run->stopping)
            stopRun(run);
    });
    connect(run->process, &QProcess::readyReadStandardOutput, this,
            [this, run]() { append(run, run->decoder(run->process->readAllStandardOutput())); });
    connect(run->process, &QProcess::errorOccurred, this, [this, run](QProcess::ProcessError error) {
        append(run, "\n[错误] " + run->process->errorString() + "\n");
        if (error == QProcess::FailedToStart) {
            run->finished = true;
            updateRun(run, "启动失败");
        }
    });
    connect(run->process, QOverload<int, QProcess::ExitStatus>::of(&QProcess::finished), this,
            [this, run](int code, QProcess::ExitStatus status) {
                append(run, run->decoder(run->process->readAllStandardOutput()));
                const QString state = run->stopping                   ? "已停止"
                                      : status == QProcess::CrashExit ? "异常退出"
                                                                      : QString("退出 %1").arg(code);
                run->finished = true;
                if (!run->stopping)
                    *run->processGroup = 0;
                append(run, QString("\n[%1] %2\n").arg(QDateTime::currentDateTime().toString("HH:mm:ss"), state));
                updateRun(run, state);
            });
    run->process->start();
}

void LogPanel::selectRun(int index) {
    if (index < 0 || index >= static_cast<int>(runs.size()))
        return;
    const auto &run = runs[index];
    outputEdit->setDocument(run->document);
    stopButton->setEnabled(!run->finished && !run->stopping);
    outputEdit->verticalScrollBar()->setValue(outputEdit->verticalScrollBar()->maximum());
}

void LogPanel::append(Run *run, const QString &text) {
    if (text.isEmpty())
        return;
    QScrollBar *scroll = outputEdit->verticalScrollBar();
    const bool follow = outputEdit->document() == run->document && scroll->value() >= scroll->maximum() - 2;
    QTextCursor cursor(run->document);
    cursor.movePosition(QTextCursor::End);
    cursor.insertText(text);
    if (run->document->characterCount() > maxLogCharacters) {
        cursor.setPosition(0);
        cursor.setPosition(run->document->characterCount() - maxLogCharacters, QTextCursor::KeepAnchor);
        cursor.removeSelectedText();
    }
    if (follow)
        scroll->setValue(scroll->maximum());
}

void LogPanel::updateRun(Run *run, const QString &status) {
    for (int i = 0; i < static_cast<int>(runs.size()); ++i) {
        if (runs[i].get() != run)
            continue;
        historyCombo->setItemText(i, run->caption + "  [" + status + "]");
        if (historyCombo->currentIndex() == i)
            stopButton->setEnabled(!run->finished && !run->stopping);
        break;
    }
}

void LogPanel::stopCurrent() {
    const int index = historyCombo->currentIndex();
    if (index < 0 || index >= static_cast<int>(runs.size()))
        return;
    Run *run = runs[index].get();
    if (run->finished || run->stopping)
        return;
    run->stopping = true;
    updateRun(run, "停止中");
    append(run, "\n[请求停止任务]\n");
    stopRun(run);
}

void LogPanel::stopRun(Run *run) {
    if (run->finished || run->process->state() == QProcess::NotRunning)
        return;
    const qint64 pid = run->process->processId();
    if (pid <= 0)
        return; // Starting 状态由 started 回调继续停止。
#ifdef Q_OS_WIN
    QProcess *killer = new QProcess(this);
    connect(killer, QOverload<int, QProcess::ExitStatus>::of(&QProcess::finished), killer, &QObject::deleteLater);
    const QString taskkill = QDir(qEnvironmentVariable("SystemRoot", "C:/Windows")).filePath("System32/taskkill.exe");
    killer->start(taskkill, {"/PID", QString::number(pid), "/T", "/F"});
#else
    if (::kill(-pid, SIGTERM) != 0)
        run->process->terminate();
#endif
    // shell 可能先退出，而子进程忽略了 SIGTERM；仍需结束整个原进程组。
    // 不捕获 Run 指针，避免历史回收后悬空。
    const QPointer<QProcess> process(run->process);
    const auto group = run->processGroup;
    QTimer::singleShot(1500, this, [process, group]() {
#ifdef Q_OS_UNIX
        if (*group > 0)
            ::kill(-*group, SIGKILL);
#endif
        *group = 0; // 不在稍后的退出/历史清理时再次使用旧 PID。
        if (process && process->state() != QProcess::NotRunning)
            process->kill();
    });
}

void LogPanel::saveCurrent() {
    if (historyCombo->currentIndex() < 0)
        return;
    const QString path = QFileDialog::getSaveFileName(this, "保存日志", "run.log", "日志 (*.log *.txt)");
    if (path.isEmpty())
        return;
    QSaveFile file(path);
    const QByteArray text = outputEdit->toPlainText().toUtf8();
    if (!file.open(QIODevice::WriteOnly) || file.write(text) != text.size() || !file.commit()) {
        QMessageBox::warning(this, "错误", "保存日志失败：" + file.errorString());
    }
}
