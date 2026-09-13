#include "parameter_drag_handle.h"

#include <QApplication>
#include <QKeyEvent>
#include <QLayout>
#include <QMouseEvent>
#include <QPainter>
#include <QPixmap>
#include <QScrollArea>
#include <QScrollBar>
#include <utility>

namespace {
constexpr int previewMargin = 3;

// A mouse-transparent overlay, not a real row: it cannot steal focus or affect layout.
class RowDragPreview final : public QWidget {
public:
    RowDragPreview(QWidget *row, bool light, QWidget *parent)
        : QWidget(parent), m_snapshot(row->grab()), m_light(light) {
        setObjectName("paramDragPreview");
        setAttribute(Qt::WA_TransparentForMouseEvents);
        setAttribute(Qt::WA_NoSystemBackground);
        setFocusPolicy(Qt::NoFocus);
        resize(row->size() + QSize(2 * previewMargin, 2 * previewMargin));
    }

protected:
    void paintEvent(QPaintEvent *) override {
        QPainter painter(this);
        painter.setRenderHint(QPainter::Antialiasing);
        const QRectF body = QRectF(rect()).adjusted(1, 1, -3, -3);
        painter.setPen(Qt::NoPen);
        painter.setBrush(QColor(0, 0, 0, 50));
        painter.drawRoundedRect(body.translated(2, 2), 5, 5);
        painter.setBrush(m_light ? QColor(255, 255, 255, 240) : QColor(32, 35, 35, 240));
        painter.setPen(QPen(m_light ? QColor("#3B82F6") : QColor("#8BAFC4"), 1));
        painter.drawRoundedRect(body, 5, 5);
        painter.setOpacity(0.92);
        painter.drawPixmap(QPoint(previewMargin, previewMargin), m_snapshot);
    }

private:
    QPixmap m_snapshot;
    bool m_light;
};
}

ParameterDragHandle::ParameterDragHandle(QScrollArea *area, DropCallback callback, QWidget *parent)
    : QWidget(parent), m_area(area), m_onDrop(std::move(callback)) {
    setObjectName("paramDragHandle");
    setFixedSize(22, 30);
    setCursor(Qt::OpenHandCursor);
    setFocusPolicy(Qt::ClickFocus);
    setAccessibleName("拖动参数行");
    setToolTip("拖动调整参数顺序；松手放置，Esc 取消");
    setAttribute(Qt::WA_Hover);
    m_scrollTimer.setInterval(40);
    connect(&m_scrollTimer, &QTimer::timeout, this, &ParameterDragHandle::autoScroll);
}

ParameterDragHandle::~ParameterDragHandle() {
    finishDrag(false);
}

void ParameterDragHandle::setTheme(const QString &theme) {
    setProperty("theme", theme);
    update();
}

void ParameterDragHandle::paintEvent(QPaintEvent *) {
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing);
    const bool light = property("theme").toString() == "light";
    QColor color = light ? QColor("#9CA3AF") : QColor("#626C70");
    if (underMouse() || m_dragging) color = light ? QColor("#4B5563") : QColor("#B9BFC1");
    painter.setPen(Qt::NoPen);
    painter.setBrush(color);
    for (int column = 0; column < 2; ++column)
        for (int row = 0; row < 3; ++row)
            painter.drawEllipse(QPointF(7.0 + column * 6.0, 9.0 + row * 6.0), 1.3, 1.3);
}

void ParameterDragHandle::mousePressEvent(QMouseEvent *event) {
    if (event->button() != Qt::LeftButton) {
        QWidget::mousePressEvent(event);
        return;
    }
    m_pressed = true;
    m_pressGlobal = m_pointerGlobal = event->globalPosition().toPoint();
    m_hotSpot = mapTo(parentWidget(), event->position().toPoint());
    event->accept();
}

void ParameterDragHandle::mouseMoveEvent(QMouseEvent *event) {
    if (!m_pressed || !(event->buttons() & Qt::LeftButton)) {
        QWidget::mouseMoveEvent(event);
        return;
    }
    m_pointerGlobal = event->globalPosition().toPoint();
    if (!m_dragging && (m_pointerGlobal - m_pressGlobal).manhattanLength() >= QApplication::startDragDistance())
        beginDrag();
    if (m_dragging) updateFeedback();
    event->accept();
}

void ParameterDragHandle::mouseReleaseEvent(QMouseEvent *event) {
    if (event->button() != Qt::LeftButton) {
        QWidget::mouseReleaseEvent(event);
        return;
    }
    m_pointerGlobal = event->globalPosition().toPoint();
    if (m_dragging) updateFeedback();
    const int destination = m_dragging ? m_dropIndex : -1;
    QWidget *row = parentWidget();
    finishDrag();
    // The model and preview change exactly once, after a valid drop.
    if (destination >= 0 && m_onDrop) m_onDrop(row, destination);
    event->accept();
}

void ParameterDragHandle::beginDrag() {
    if (!m_area || !m_area->widget() || !parentWidget()) return;
    m_dragging = true;
    m_previousFocus = QApplication::focusWidget();
    setFocus(Qt::MouseFocusReason);
    m_dragWindow = window();
    m_dragWindow->installEventFilter(this);
    const bool light = property("theme").toString() == "light";
    m_preview = new RowDragPreview(parentWidget(), light, m_dragWindow);

    // Keep the original row in place until drop so the list doesn't jump under the mouse.
    m_sourceMask = new QWidget(parentWidget());
    m_sourceMask->setObjectName("paramDragSourceMask");
    m_sourceMask->setAttribute(Qt::WA_TransparentForMouseEvents);
    m_sourceMask->setStyleSheet(light
        ? "background: rgba(255,255,255,150); border: 1px dashed #9CA3AF;"
        : "background: rgba(21,23,23,150); border: 1px dashed #626C70;");
    m_sourceMask->setGeometry(parentWidget()->rect());
    m_sourceMask->show();

    // Use the same overlay parent so the insertion line can be raised above the ghost.
    m_indicator = new QWidget(m_dragWindow);
    m_indicator->setObjectName("paramDropIndicator");
    m_indicator->setAttribute(Qt::WA_TransparentForMouseEvents);
    m_indicator->setStyleSheet(light ? "background: #3B82F6; border: none;"
                                     : "background: #8BAFC4; border: none;");
    m_scrollTimer.start();
    update();
}

void ParameterDragHandle::updateFeedback() {
    m_dropIndex = -1;
    if (!m_area || !m_preview || !m_indicator || !m_dragWindow) return;
    m_preview->move(m_dragWindow->mapFromGlobal(m_pointerGlobal) - m_hotSpot
        - QPoint(previewMargin, previewMargin));
    m_preview->show();
    m_preview->raise();

    QWidget *viewport = m_area->viewport();
    QWidget *container = m_area->widget();
    QLayout *rows = container ? container->layout() : nullptr;
    if (!rows || rows->indexOf(parentWidget()) < 0
        || !viewport->rect().contains(viewport->mapFromGlobal(m_pointerGlobal))) {
        m_indicator->hide();
        setCursor(Qt::ForbiddenCursor);
        return;
    }
    setCursor(Qt::ClosedHandCursor);
    const int pointerY = container->mapFromGlobal(m_pointerGlobal).y();
    QWidget *before = nullptr;
    QWidget *last = nullptr;
    m_dropIndex = 0;
    for (int i = 0; i < rows->count(); ++i) {
        QWidget *candidate = rows->itemAt(i)->widget();
        if (!candidate || candidate == parentWidget()) continue;
        if (pointerY > candidate->geometry().center().y()) ++m_dropIndex;
        else if (!before) before = candidate;
        last = candidate;
    }
    const int gap = qMax(0, rows->spacing()) / 2;
    const int insertionY = before ? before->y() - gap
        : last ? last->geometry().bottom() + 1 + gap : parentWidget()->y();
    const int viewportY = container->mapTo(viewport, QPoint(0, insertionY)).y();
    const QPoint markerPosition = viewport->mapTo(m_dragWindow,
        QPoint(2, qBound(0, viewportY - 1, qMax(0, viewport->height() - 2))));
    m_indicator->setGeometry(QRect(markerPosition, QSize(qMax(0, viewport->width() - 4), 2)));
    m_indicator->show();
    m_indicator->raise();
}

void ParameterDragHandle::autoScroll() {
    if (!m_dragging || !m_area) return;
    QWidget *viewport = m_area->viewport();
    const QPoint position = viewport->mapFromGlobal(m_pointerGlobal);
    if (viewport->rect().contains(position)) {
        constexpr int edge = 24;
        const int direction = position.y() < edge ? -1 : position.y() >= viewport->height() - edge ? 1 : 0;
        if (direction) {
            QScrollBar *scroll = m_area->verticalScrollBar();
            scroll->setValue(scroll->value() + direction * qMax(4, parentWidget()->height() / 3));
        }
    }
    updateFeedback();
}

void ParameterDragHandle::finishDrag(bool restoreFocus) {
    m_scrollTimer.stop();
    m_dragging = m_pressed = false;
    m_dropIndex = -1;
    if (m_dragWindow) m_dragWindow->removeEventFilter(this);
    m_dragWindow.clear();
    delete m_preview.data();
    delete m_indicator.data();
    delete m_sourceMask.data();
    if (restoreFocus && m_previousFocus && m_previousFocus != this)
        m_previousFocus->setFocus(Qt::OtherFocusReason);
    m_previousFocus.clear();
    setCursor(Qt::OpenHandCursor);
    update();
}

void ParameterDragHandle::keyPressEvent(QKeyEvent *event) {
    if (event->key() == Qt::Key_Escape && m_pressed) {
        finishDrag();
        event->accept();
        return;
    }
    QWidget::keyPressEvent(event);
}

bool ParameterDragHandle::event(QEvent *event) {
    if (m_pressed && (event->type() == QEvent::Hide || event->type() == QEvent::UngrabMouse))
        finishDrag(false);
    return QWidget::event(event);
}

bool ParameterDragHandle::eventFilter(QObject *object, QEvent *event) {
    if (object == m_dragWindow && (event->type() == QEvent::WindowDeactivate || event->type() == QEvent::Hide))
        finishDrag(false);
    return QWidget::eventFilter(object, event);
}
