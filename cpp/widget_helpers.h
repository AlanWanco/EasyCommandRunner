#pragma once

#include <QApplication>
#include <QClipboard>
#include <QDragEnterEvent>
#include <QDropEvent>
#include <QFile>
#include <QFileInfo>
#include <QIcon>
#include <QKeyEvent>
#include <QLineEdit>
#include <QMenu>
#include <QMimeData>
#include <QPainter>
#include <QPixmap>
#include <QPushButton>
#include <QTextEdit>
#include <QUrl>

inline QIcon themedIcon(const QString &name, const QString &theme) {
    QFile file(":/res/" + name + ".svg");
    if (!file.open(QIODevice::ReadOnly))
        return {};
    QByteArray svg = file.readAll();
    svg.replace("currentColor", theme == "light" ? "#4b5563" : "#c6c9cc");
    QPixmap image;
    image.loadFromData(svg, "SVG");
    return QIcon(image);
}

inline void setButtonIcon(QPushButton *button, const QString &name, const QString &theme = "dark") {
    button->setProperty("svgIcon", name);
    button->setIcon(themedIcon(name, theme));
    button->setIconSize(QSize(16, 16));
    button->setCursor(Qt::PointingHandCursor);
}

inline QString localClipboardPath(const QMimeData *mime) {
    for (const QUrl &url : mime->urls()) {
        if (url.isLocalFile())
            return url.toLocalFile();
    }
    return {};
}

// 文件 URL 用平台原生路径语义，不再把 macOS/Linux 路径强制改成反斜杠。
class PathLineEdit : public QLineEdit {
  public:
    explicit PathLineEdit(QWidget *parent = nullptr) : QLineEdit(parent) { setAcceptDrops(true); }

  protected:
    void dragEnterEvent(QDragEnterEvent *event) override {
        if (!localClipboardPath(event->mimeData()).isEmpty())
            event->acceptProposedAction();
        else
            QLineEdit::dragEnterEvent(event);
    }
    void dropEvent(QDropEvent *event) override {
        const QString path = localClipboardPath(event->mimeData());
        if (path.isEmpty()) {
            QLineEdit::dropEvent(event);
            return;
        }
        setText(path);
        event->acceptProposedAction();
    }
    void keyPressEvent(QKeyEvent *event) override {
        const QString path = localClipboardPath(QApplication::clipboard()->mimeData());
        if (event->matches(QKeySequence::Paste) && !path.isEmpty()) {
            setText(path);
            return;
        }
        QLineEdit::keyPressEvent(event);
    }
    void contextMenuEvent(QContextMenuEvent *event) override {
        QMenu *menu = createStandardContextMenu();
        const QString path = localClipboardPath(QApplication::clipboard()->mimeData());
        if (!path.isEmpty()) {
            menu->addSeparator();
            connect(menu->addAction("粘贴文件路径"), &QAction::triggered, this, [this, path]() { setText(path); });
        }
        menu->exec(event->globalPos());
        delete menu;
    }
};

class PathTextEdit : public QTextEdit {
  public:
    explicit PathTextEdit(QWidget *parent = nullptr) : QTextEdit(parent) { setAcceptRichText(false); }

  protected:
    bool canInsertFromMimeData(const QMimeData *source) const override {
        return !localClipboardPath(source).isEmpty() || QTextEdit::canInsertFromMimeData(source);
    }
    void insertFromMimeData(const QMimeData *source) override {
        const QString path = localClipboardPath(source);
        if (!path.isEmpty())
            insertPlainText(path);
        else
            insertPlainText(source->text());
    }
};
