#pragma once

#include <QDockWidget>
#include <memory>
#include <vector>

class QComboBox;
class QPlainTextEdit;
class QPushButton;
class QWidget;

// 每次运行独立的非交互 shell；Dock 与浮动窗口共用同一组进程和日志。
class LogPanel : public QDockWidget {
    Q_OBJECT

  public:
    explicit LogPanel(QWidget *parent = nullptr);
    ~LogPanel() override;

    void startCommand(const QString &title, const QString &command, const QString &workingDirectory);
    int runningCount() const;

  public slots:
    void stopCurrent();

  protected:
    bool eventFilter(QObject *object, QEvent *event) override;

  private:
    struct Run;
    void selectRun(int index);
    void append(Run *run, const QString &text);
    void updateRun(Run *run, const QString &status);
    void stopRun(Run *run);
    void adjustOutputFontSize(int delta);
    void saveCurrent();

    std::vector<std::unique_ptr<Run>> runs;
    QWidget *customTitleBar;
    QComboBox *historyCombo;
    QPlainTextEdit *outputEdit;
    QPushButton *stopButton;
    QPushButton *detachButton;
};
