#pragma once

#include <QPointer>
#include <QTimer>
#include <QWidget>
#include <functional>

class QScrollArea;

// Only the handle starts a drag; editing/selecting text in a row remains unchanged.
class ParameterDragHandle final : public QWidget {
public:
    using DropCallback = std::function<void(QWidget *, int)>;
    ParameterDragHandle(QScrollArea *area, DropCallback callback, QWidget *parent = nullptr);
    ~ParameterDragHandle() override;
    void setTheme(const QString &theme);

protected:
    void paintEvent(QPaintEvent *) override;
    void mousePressEvent(QMouseEvent *event) override;
    void mouseMoveEvent(QMouseEvent *event) override;
    void mouseReleaseEvent(QMouseEvent *event) override;
    void keyPressEvent(QKeyEvent *event) override;
    bool event(QEvent *event) override;
    bool eventFilter(QObject *object, QEvent *event) override;

private:
    void beginDrag();
    void updateFeedback();
    void autoScroll();
    void finishDrag(bool restoreFocus = true);

    QPointer<QScrollArea> m_area;
    DropCallback m_onDrop;
    QTimer m_scrollTimer;
    QPointer<QWidget> m_preview;
    QPointer<QWidget> m_indicator;
    QPointer<QWidget> m_sourceMask;
    QPointer<QWidget> m_dragWindow;
    QPointer<QWidget> m_previousFocus;
    QPoint m_pressGlobal;
    QPoint m_pointerGlobal;
    QPoint m_hotSpot;
    int m_dropIndex = -1;
    bool m_pressed = false;
    bool m_dragging = false;
};
