#include "app.h"
#include "log_panel.h"
#include "widget_helpers.h"

#include <QApplication>
#include <QComboBox>
#include <QDir>
#include <QFile>
#include <QFontDatabase>
#include <QFontInfo>
#include <QFontMetricsF>
#include <QRawFont>
#include <QScrollBar>
#include <QStandardPaths>
#include <QTabBar>
#include <QToolButton>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QMenuBar>
#include <QMouseEvent>
#include <QPlainTextEdit>
#include <QProcess>
#include <QPointer>
#include <QSignalSpy>
#include <QRegularExpression>
#include <QScrollArea>
#include <QStringList>
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
void moveParameterDrag(QWidget *handle, const QPoint &globalPosition) {
    QMouseEvent move(QEvent::MouseMove, handle->mapFromGlobal(globalPosition), globalPosition,
        Qt::NoButton, Qt::LeftButton, Qt::NoModifier);
    QApplication::sendEvent(handle, &move);
}

void releaseParameterDrag(QWidget *handle, const QPoint &globalPosition) {
    QMouseEvent release(QEvent::MouseButtonRelease, handle->mapFromGlobal(globalPosition), globalPosition,
        Qt::LeftButton, Qt::NoButton, Qt::NoModifier);
    QApplication::sendEvent(handle, &release);
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
        window.resize(850, 1000);
        QTest::qWait(150);
        auto *tabs = window.findChild<QTabWidget *>();
        auto *tab = qobject_cast<CommandTab *>(tabs->currentWidget());
        auto *log = window.findChild<LogPanel *>();
        QVERIFY(tab);
        QVERIFY(log->isHidden());
        // Windows hosted runners may clamp a top-level window to the
        // Hyper-V monitor work area.  The layout assertions below do not
        // depend on the requested height being fully available.
        QCOMPARE(window.width(), 850);
        QVERIFY(window.height() > 0 && window.height() <= 1000);
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
        QCOMPARE(image.pixelColor(window.centralWidget()->mapTo(&window, QPoint(8, 8))), QColor("#121414"));
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

    void bundledFontAndParameterScrolling() {
        AppWindow window;
        auto *tab = qobject_cast<CommandTab *>(window.findChild<QTabWidget *>()->currentWidget());
        QJsonArray rows;
        for (int i = 0; i < 40; ++i)
            rows.append(QJsonObject{{"function", QString("--option%1").arg(i)}, {"parameter", "中文 abc"}});
        tab->loadConfiguration(QJsonObject{{"name", "滚动测试"}, {"program", "echo"}, {"functions", rows}});
        auto *scroll = tab->findChild<QScrollArea *>("paramsScrollArea");
        auto *description = tab->findChild<QTextEdit *>("descriptionEdit");
        auto *preview = tab->findChild<QTextEdit *>("commandPreviewEdit");
        QVERIFY(scroll && description && preview);
        QVERIFY(!tab->findChild<QScrollArea *>("commandScrollArea"));
        QVERIFY(!scroll->isAncestorOf(tab->findChild<QLineEdit *>("programEdit")));
        window.resize(1000, 950);
        QTest::qWait(60);
        QVector<int> heights;
        for (int count = 1; count <= 7; ++count) {
            QJsonArray countRows;
            for (int i = 0; i < count; ++i)
                countRows.append(QJsonObject{{"function", QString("--row%1").arg(i)}});
            tab->loadConfiguration(QJsonObject{{"name", "行数测试"}, {"program", "echo"}, {"functions", countRows}});
            QTest::qWait(40);
            heights.append(scroll->height());
        }
        QStringList heightValues;
        for (int height : heights) heightValues << QString::number(height);
        const QString heightSummary = heightValues.join(", ");
        for (int i = 1; i < 6; ++i)
            QVERIFY2(heights[i] > heights[i - 1], qPrintable("Parameter area heights: " + heightSummary));
        QVERIFY2(heights[6] <= heights[5] + 2,
            qPrintable("Parameter area heights: " + heightSummary));
        tab->loadConfiguration(QJsonObject{{"name", "滚动测试"}, {"program", "echo"}, {"functions", rows}});
        QTest::qWait(60);
        const QString family = QFontDatabase::applicationFontFamilies(0).value(0);
        QVERIFY(family.contains("Sarasa") || family.contains("更纱"));
        for (const QString &theme : {QString("dark"), QString("light")}) {
            window.onThemeChanged(theme);
            for (int fontSize : {14, 20}) {
                window.onFontSizeChanged(fontSize);
                window.resize(1000, 950);
                QTest::qWait(60);
                const QFont font = tab->findChild<QLineEdit *>("programEdit")->font();
                QCOMPARE(QFontInfo(font).family(), family);
                const QRawFont rawFont = QRawFont::fromFont(font);
                QVERIFY(rawFont.supportsCharacter(QChar(u'中')));
                QVERIFY(rawFont.supportsCharacter(QChar(u'矢')));
                QVERIFY(rawFont.supportsCharacter(QChar(u'「')));
                const QFontMetricsF metrics(font);
                QVERIFY(qAbs(metrics.horizontalAdvance("i") - metrics.horizontalAdvance("W")) < 0.01);
                QVERIFY2(metrics.horizontalAdvance("中") >= metrics.horizontalAdvance("W"),
                    qPrintable(QString("CJK=%1 Latin=%2 font=%3").arg(metrics.horizontalAdvance("中"))
                        .arg(metrics.horizontalAdvance("W")).arg(font.toString())));
                QCOMPARE(QFontInfo(preview->font()).family(), family);
                auto *logOutput = window.findChild<QPlainTextEdit *>("runLogOutput");
                QCOMPARE(QFontInfo(logOutput->font()).family(), family);
                scroll->verticalScrollBar()->setValue(0);
                QVERIFY(scroll->verticalScrollBar()->maximum() > 0);
                auto *sixthRow = tab->findChild<QWidget *>("paramRow_6");
                QVERIFY(sixthRow);
                const QRect sixthRect(sixthRow->mapTo(scroll->viewport(), QPoint()), sixthRow->size());
                QVERIFY2(scroll->viewport()->rect().contains(sixthRect), "Six parameter rows must be fully visible");
                QCOMPARE(description->height(), 80);
                const int oldPreviewHeight = preview->height();
                const int oldWindowHeight = window.height();
                window.resize(window.width(), oldWindowHeight + 180);
                QTest::qWait(60);
                QCOMPARE(description->height(), 80);
                // If the desktop work area is already full, Windows keeps
                // the top-level window at its maximum size and there is no
                // room for the preview to grow.
                if (window.height() > oldWindowHeight + 2)
                    QVERIFY(preview->height() > oldPreviewHeight);
            }
        }
        window.onFontSizeChanged(14);
    }

    void settingsFontChangesAreDeferred() {
        AppWindow window;
        const QString originalTheme = window.getCurrentTheme();
        const int originalSize = window.getFontSize();
        const int originalWeight = window.getFontWeight();
        SettingsDialog settings(&window);
        auto *size = settings.findChild<QSpinBox *>("fontSizeSpin");
        auto *weight = settings.findChild<QComboBox *>("fontWeightCombo");
        auto *theme = settings.findChild<QComboBox *>("themeCombo");
        QVERIFY(size && weight && theme);
        const int pendingSize = originalSize == 24 ? 23 : originalSize + 1;
        size->setValue(pendingSize);
        weight->setCurrentIndex(weight->findData(700));
        const QString pendingTheme = originalTheme == "dark" ? "light" : "dark";
        theme->setCurrentIndex(theme->findData(pendingTheme));
        QCOMPARE(window.getCurrentTheme(), originalTheme);
        QCOMPARE(window.getFontSize(), originalSize);
        QCOMPARE(window.getFontWeight(), originalWeight);
        QVERIFY(buttonWithText(&settings, "应用"));
        buttonWithText(&settings, "应用")->click();
        QCOMPARE(window.getCurrentTheme(), pendingTheme);
        QCOMPARE(window.getFontSize(), pendingSize);
        QCOMPARE(window.getFontWeight(), 700);

        SettingsDialog cancelled(&window);
        auto *cancelledSize = cancelled.findChild<QSpinBox *>("fontSizeSpin");
        cancelledSize->setValue(pendingSize == 24 ? 23 : pendingSize + 1);
        QCOMPARE(window.getFontSize(), pendingSize);
        cancelled.reject();
        QCOMPARE(window.getFontSize(), pendingSize);
        QCOMPARE(window.getFontWeight(), 700);
    }

    void crowdedTabBarControls() {
        AppWindow window;
        auto *tabs = window.findChild<QTabWidget *>();
        for (int i = 1; i < 32; ++i) {
            buttonWithText(&window, "新建标签")->click();
            tabs->setTabText(i, QString("中文标签 %1 long name").arg(i));
        }
        auto *combo = window.findChild<QComboBox *>("tabCombo");
        auto *bar = tabs->tabBar();
        auto *left = window.findChild<QPushButton *>("previousTabBtn");
        auto *right = window.findChild<QPushButton *>("nextTabBtn");
        QVERIFY(left && right);
        QVERIFY(!bar->drawBase());
        QMap<QString, QVector<QRect>> darkHeaderGeometry;
        for (const QString &theme : {QString("dark"), QString("light")}) {
            window.onThemeChanged(theme);
            QVERIFY(!bar->drawBase());
            for (int fontSize : {14, 20}) {
                window.onFontSizeChanged(fontSize);
                for (int width : {850, 1200}) {
                    window.resize(width, 950);
                    for (int index : {0, 15, 31}) {
                        tabs->setCurrentIndex(index);
                        QTest::qWait(30);
                        const int barTop = bar->mapTo(&window, QPoint()).y();
                        QCOMPARE(left->mapTo(&window, QPoint()).y(), barTop);
                        QCOMPARE(right->mapTo(&window, QPoint()).y(), barTop);
                        const QRect currentTabRect = bar->tabRect(tabs->currentIndex());
                        const int tabHeight = currentTabRect.height();
                        QVERIFY(tabHeight > 0);
                        QCOMPARE(currentTabRect.top(), 0);
                        QCOMPARE(currentTabRect.bottom(), bar->rect().bottom());
                        QCOMPARE(left->height(), tabHeight);
                        QCOMPARE(right->height(), tabHeight);
                        // The content pane must start immediately below the actual tabs, not a blank strip.
                        QCOMPARE(tabs->currentWidget()->mapTo(tabs, QPoint()).y(),
                            bar->geometry().bottom() + 1);
                        QVector<QRect> headerGeometry{bar->geometry(), combo->geometry(), currentTabRect};
                        QVERIFY2(tabs->rect().contains(combo->geometry()),
                            qPrintable(QString("combo=%1,%2 %3x%4 bar=%5x%6 min=%7 hint=%8 theme=%9 size=%10")
                                .arg(combo->x()).arg(combo->y()).arg(combo->width()).arg(combo->height())
                                .arg(bar->width()).arg(bar->height()).arg(bar->minimumHeight())
                                .arg(bar->sizeHint().height()).arg(theme).arg(fontSize)));
                        QVERIFY(!bar->geometry().intersects(combo->geometry()));
                        for (auto *arrow : bar->findChildren<QToolButton *>()) {
                            if (!arrow->isVisible()) continue;
                            // Check the overflow arrows INSIDE QTabBar, not only the outer navigation buttons.
                            QCOMPARE(arrow->geometry().top(), currentTabRect.top());
                            QCOMPARE(arrow->geometry().bottom(), currentTabRect.bottom());
                            headerGeometry.append(arrow->geometry());
                            QCOMPARE(bar->childAt(arrow->geometry().center()), arrow);
                            // 关闭图标不能从按钮背景透出；箭头边角应为不透明主题色。
                            const auto image = arrow->grab().toImage();
                            QCOMPARE(image.pixelColor(3, 3), QColor(theme == "dark" ? "#202323" : "#F3F4F6"));
                        }
                        const QString key = QString("%1/%2/%3").arg(fontSize).arg(width).arg(index);
                        if (theme == "dark") darkHeaderGeometry.insert(key, headerGeometry);
                        else QCOMPARE(headerGeometry, darkHeaderGeometry.value(key));
                        const QString artifactDir = qEnvironmentVariable("ECR_UI_ARTIFACT_DIR");
                        if (!artifactDir.isEmpty() && fontSize == 14 && width == 850 && index == 15) {
                            QDir().mkpath(artifactDir);
                            const QString path = artifactDir + "/tab-header-" + theme;
                            QVERIFY(tabs->grab(QRect(0, 0, tabs->width(), bar->height() + 8)).save(path + ".png"));
                            QJsonArray rects;
                            for (const QRect &rect : headerGeometry)
                                rects.append(QJsonArray{rect.x(), rect.y(), rect.width(), rect.height()});
                            QFile output(path + ".json");
                            QVERIFY(output.open(QIODevice::WriteOnly));
                            output.write(QJsonDocument(QJsonObject{
                                {"rectOrder", "bar, combo, currentTab, overflowArrows"}, {"rects", rects}}).toJson());
                        }
                    }
                }
            }
        }
        window.onFontSizeChanged(14);
    }

    void singleTabHeaderHasNoBottomGap() {
        AppWindow window;
        auto *tabs = window.findChild<QTabWidget *>();
        auto *bar = tabs->tabBar();
        auto *combo = window.findChild<QComboBox *>("tabCombo");
        QCOMPARE(tabs->count(), 1);
        for (int fontSize : {10, 14, 20, 24}) {
            window.onFontSizeChanged(fontSize);
            QVector<QRect> reference;
            for (const QString &theme : {QString("dark"), QString("light"), QString("dark")}) {
                window.onThemeChanged(theme);
                window.resize(1000, 1000);
                QTest::qWait(60);
                const QRect tabRect = bar->tabRect(0);
                QCOMPARE(tabRect.top(), 0);
                QCOMPARE(tabRect.bottom(), bar->rect().bottom());
                QCOMPARE(tabs->currentWidget()->mapTo(tabs, QPoint()).y(),
                    bar->geometry().bottom() + 1);
                QVERIFY(tabs->rect().contains(combo->geometry()));
                QVERIFY(!bar->geometry().intersects(combo->geometry()));
                auto *close = bar->tabButton(0, QTabBar::RightSide);
                if (!close) close = bar->tabButton(0, QTabBar::LeftSide);
                QVERIFY(close);
                QVERIFY(tabRect.contains(close->geometry()));
                const QVector<QRect> geometry{bar->geometry(), tabRect, combo->geometry(), close->geometry()};
                if (reference.isEmpty()) reference = geometry;
                else QCOMPARE(geometry, reference);

                // Geometry alone is insufficient: the tab's fill must reach its bottom edge.
                const QImage image = bar->grab().toImage();
                const QPoint sample(tabRect.center().x(), tabRect.bottom() - 1);
                QCOMPARE(image.pixelColor(QPoint(qRound(sample.x() * image.devicePixelRatio()),
                    qRound(sample.y() * image.devicePixelRatio()))),
                    QColor(theme == "dark" ? "#343838" : "#FFFFFF"));
            }
        }
    }

    void parameterRowsCanBeReordered_data() {
        QTest::addColumn<QString>("theme");
        QTest::newRow("dark") << QString("dark");
        QTest::newRow("light") << QString("light");
    }

    void parameterRowsCanBeReordered() {
        QFETCH(QString, theme);
        AppWindow window;
        window.onThemeChanged(theme);
        auto *tab = qobject_cast<CommandTab *>(window.findChild<QTabWidget *>()->currentWidget());
        tab->loadConfiguration(QJsonObject{
            {"name", "拖动测试"}, {"program", "echo"},
            {"functions", QJsonArray{
                QJsonObject{{"function", "--first"}, {"parameter", "1"}, {"comment", "第一行"}},
                QJsonObject{{"function", "--second"}, {"parameter", "2"}, {"enabled", false}},
                QJsonObject{{"function", "--third"}, {"parameter", "3"}, {"comment", "一起移动"}}}}});
        window.resize(1000, 900);
        QTest::qWait(80);

        auto *thirdRow = tab->findChild<QWidget *>("paramRow_3");
        auto *firstRow = tab->findChild<QWidget *>("paramRow_1");
        QVERIFY(thirdRow && firstRow);
        auto *handle = thirdRow->findChild<QWidget *>("paramDragHandle");
        QVERIFY(handle);
        const auto original = tab->saveConfiguration();
        const QString originalCommand = tab->getCommand();
        const QRect originalRowRect = thirdRow->geometry();
        const QPoint press(11, 15);
        const QPoint hotSpot = handle->mapTo(thirdRow, press);
        QSignalSpy changed(tab, &CommandTab::configurationChanged);
        QTest::mousePress(handle, Qt::LeftButton, Qt::NoModifier, press);
        QVERIFY(!window.findChild<QWidget *>("paramDragPreview"));
        const QPoint targetGlobal = firstRow->mapToGlobal(QPoint(11, 5));
        moveParameterDrag(handle, targetGlobal);

        auto *ghost = window.findChild<QWidget *>("paramDragPreview");
        auto *marker = window.findChild<QWidget *>("paramDropIndicator");
        auto *mask = thirdRow->findChild<QWidget *>("paramDragSourceMask");
        QVERIFY(ghost && marker && mask);
        QVERIFY(ghost->isVisible() && marker->isVisible() && mask->isVisible());
        QVERIFY(ghost->testAttribute(Qt::WA_TransparentForMouseEvents));
        QCOMPARE(ghost->size(), thirdRow->size() + QSize(6, 6));
        QCOMPARE(ghost->mapToGlobal(QPoint(3, 3)), targetGlobal - hotSpot);
        QCOMPARE(thirdRow->geometry(), originalRowRect);
        QCOMPARE(tab->saveConfiguration(), original);
        QCOMPARE(tab->getCommand(), originalCommand);
        QCOMPARE(changed.count(), 0);
        const QPoint nextGlobal = targetGlobal + QPoint(24, 0);
        moveParameterDrag(handle, nextGlobal);
        QCOMPARE(ghost->mapToGlobal(QPoint(3, 3)), nextGlobal - hotSpot);
        QCOMPARE(marker->parentWidget(), ghost->parentWidget());
        // The insertion line must remain visible even where it overlaps the floating row.
        const QPoint markerCenter = marker->mapTo(&window, marker->rect().center());
        QVERIFY(ghost->geometry().contains(markerCenter));
        const QImage feedbackImage = window.grab().toImage();
        QCOMPARE(feedbackImage.pixelColor(QPoint(qRound(markerCenter.x() * feedbackImage.devicePixelRatio()),
            qRound(markerCenter.y() * feedbackImage.devicePixelRatio()))),
            QColor(theme == "light" ? "#3B82F6" : "#8BAFC4"));

        const QString artifacts = qEnvironmentVariable("ECR_UI_ARTIFACT_DIR");
        if (!artifacts.isEmpty()) {
            QDir().mkpath(artifacts);
            QVERIFY(window.grab().save(artifacts + "/parameter-drag-" + theme + ".png"));
            QVERIFY(ghost->grab().save(artifacts + "/parameter-drag-preview-" + theme + ".png"));
        }
        releaseParameterDrag(handle, nextGlobal);
        QTest::qWait(60);
        QVERIFY(!window.findChild<QWidget *>("paramDragPreview"));
        QVERIFY(!window.findChild<QWidget *>("paramDropIndicator"));
        QVERIFY(!window.findChild<QWidget *>("paramDragSourceMask"));
        const auto rows = original.value("functions").toArray();
        QCOMPARE(tab->saveConfiguration().value("functions").toArray(), QJsonArray({rows[2], rows[0], rows[1]}));
        QCOMPARE(tab->getCommand(), QString("echo --third 3 --first 1"));
        QCOMPARE(changed.count(), 1);

        // Drag the same row down to the end; metadata and disabled states still travel with it.
        auto *secondRow = tab->findChild<QWidget *>("paramRow_2");
        QTest::mousePress(handle, Qt::LeftButton, Qt::NoModifier, press);
        const QPoint end = secondRow->mapToGlobal(QPoint(11, secondRow->height() - 2));
        moveParameterDrag(handle, end);
        releaseParameterDrag(handle, end);
        QCOMPARE(tab->saveConfiguration(), original);
        QCOMPARE(tab->getCommand(), originalCommand);
        QCOMPARE(changed.count(), 2);
    }

    void parameterDragCancellationAndAutoScroll() {
        AppWindow window;
        auto *tab = qobject_cast<CommandTab *>(window.findChild<QTabWidget *>()->currentWidget());
        QJsonArray rows;
        for (int i = 1; i <= 12; ++i)
            rows.append(QJsonObject{{"function", QString("--row%1").arg(i)}});
        tab->loadConfiguration(QJsonObject{{"program", "echo"}, {"functions", rows}});
        window.resize(1000, 1000);
        QTest::qWait(80);
        const auto original = tab->saveConfiguration();
        auto *area = tab->findChild<QScrollArea *>("paramsScrollArea");
        auto *handle = tab->findChild<QWidget *>("paramRow_1")->findChild<QWidget *>("paramDragHandle");
        auto *viewport = area->viewport();
        auto *scroll = area->verticalScrollBar();
        const QPoint target = viewport->mapToGlobal(QPoint(20, viewport->height() / 2));
        QSignalSpy changed(tab, &CommandTab::configurationChanged);

        QTest::mouseClick(handle, Qt::LeftButton);
        QVERIFY(!window.findChild<QWidget *>("paramDragPreview"));
        QTest::mousePress(handle, Qt::LeftButton);
        moveParameterDrag(handle, target);
        QVERIFY(window.findChild<QWidget *>("paramDragPreview"));
        QTest::keyClick(handle, Qt::Key_Escape);
        releaseParameterDrag(handle, target);
        QVERIFY(!window.findChild<QWidget *>("paramDragPreview"));
        QCOMPARE(tab->saveConfiguration(), original);

        QTest::mousePress(handle, Qt::LeftButton);
        const QPoint outside = viewport->mapToGlobal(QPoint(viewport->width() + 10, 50));
        moveParameterDrag(handle, outside);
        QVERIFY(!window.findChild<QWidget *>("paramDropIndicator")->isVisible());
        releaseParameterDrag(handle, outside);
        QCOMPARE(tab->saveConfiguration(), original);
        QCOMPARE(changed.count(), 0);

        // Holding at the bottom auto-scrolls even without additional mouse movement.
        QTest::mousePress(handle, Qt::LeftButton);
        const QPoint bottom = viewport->mapToGlobal(QPoint(20, viewport->height() - 3));
        moveParameterDrag(handle, bottom);
        QVERIFY(scroll->maximum() > 0);
        QTRY_COMPARE_WITH_TIMEOUT(scroll->value(), scroll->maximum(), 4000);
        QCOMPARE(tab->saveConfiguration(), original);
        releaseParameterDrag(handle, bottom);
        auto expected = original.value("functions").toArray();
        const auto first = expected.takeAt(0);
        expected.append(first);
        QCOMPARE(tab->saveConfiguration().value("functions").toArray(), expected);
        QCOMPARE(changed.count(), 1);
        QTest::qWait(60);

        // From the bottom, holding at the top scrolls back and supports dropping at the beginning.
        QTest::mousePress(handle, Qt::LeftButton);
        const QPoint top = viewport->mapToGlobal(QPoint(20, 2));
        moveParameterDrag(handle, top);
        QTRY_COMPARE_WITH_TIMEOUT(scroll->value(), 0, 4000);
        releaseParameterDrag(handle, top);
        QCOMPARE(tab->saveConfiguration(), original);

        // Removing a row/tab while dragging must not leave a ghost or active timer behind.
        QTest::qWait(60);
        QTest::mousePress(handle, Qt::LeftButton);
        moveParameterDrag(handle, target);
        QPointer<QWidget> ghost = window.findChild<QWidget *>("paramDragPreview");
        QVERIFY(ghost);
        tab->clear();
        QVERIFY(ghost.isNull());
        QVERIFY(!window.findChild<QWidget *>("paramDropIndicator"));
        QTest::qWait(100);
    }

    void quotedUnicodeArgumentsAndShellSyntax() {
        LogPanel log;
        const QString input = "I:/Downloads/[20260913]矢野妃菜喜×立花日菜 「PARALLEL」[7-1181].mp4";
        const QString command = childCommand("--show-args") + " -i \"" + input + "\" --gemini";
        log.startCommand("argv", command, data->path());
        QTRY_COMPARE(log.runningCount(), 0);
        auto *output = log.findChild<QPlainTextEdit *>("runLogOutput");
        QVERIFY2(output->toPlainText().contains("arg-count=3\narg=-i\narg=" + input + "\narg=--gemini"),
                 qPrintable(output->toPlainText()));
        QVERIFY(!output->toPlainText().contains(QChar::ReplacementCharacter));
        log.startCommand("redirect", childCommand("--short-log") + " > \"output with spaces.txt\"", data->path());
        QTRY_COMPARE(log.runningCount(), 0);
        QFile file(data->filePath("output with spaces.txt"));
        QVERIFY(file.open(QIODevice::ReadOnly));
        QVERIFY(file.readAll().contains("second-output"));
#ifdef Q_OS_WIN
        log.startCommand("cmd-unicode", "echo 中文日志& echo second-marker", data->path());
#else
        log.startCommand("shell-unicode", "echo 中文日志; echo second-marker", data->path());
#endif
        QTRY_COMPARE(log.runningCount(), 0);
        QVERIFY2(output->toPlainText().contains("中文日志\nsecond-marker"), qPrintable(output->toPlainText()));
    }

    void pythonUtf8OutputAndArguments() {
#ifdef Q_OS_WIN
        const QString python = QStandardPaths::findExecutable("python.exe");
        if (python.isEmpty()) QSKIP("Python is not installed");
        QFile script(data->filePath("中文 script.py"));
        QVERIFY(script.open(QIODevice::WriteOnly));
        script.write(QStringLiteral("import sys\nprint('正在加载配置：中文输出')\n"
            "print('参数=' + sys.argv[1])\nprint('错误输出', file=sys.stderr)\n").toUtf8());
        script.close();
        LogPanel log;
        log.startCommand("python", "\"" + QDir::toNativeSeparators(python) + "\" \"" + script.fileName()
            + "\" \"矢野妃菜喜×立花日菜 「PARALLEL」.mp4\"", data->path());
        QTRY_COMPARE(log.runningCount(), 0);
        const QString text = log.findChild<QPlainTextEdit *>("runLogOutput")->toPlainText();
        QVERIFY2(text.contains("正在加载配置：中文输出"), qPrintable(text));
        QVERIFY(text.contains("参数=矢野妃菜喜×立花日菜 「PARALLEL」.mp4"));
        QVERIFY(text.contains("错误输出"));
        QVERIFY(!text.contains(QChar::ReplacementCharacter));
#else
        QSKIP("Windows Python pipe encoding regression");
#endif
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
#ifdef Q_OS_WIN
        QCOMPARE(tab.getCommand(), QString("ffmpeg -i \"input file.mp4\" -y -n -1"));
#else
        QCOMPARE(tab.getCommand(), QString("ffmpeg -i 'input file.mp4' -y -n -1"));
#endif
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
        if (mode == "--show-args") {
            QCoreApplication app(argc, argv);
            const QStringList args = app.arguments().mid(2);
            std::cout << "arg-count=" << args.size() << '\n';
            for (const QString &arg : args)
                std::cout << "arg=" << arg.toUtf8().constData() << '\n';
            return 0;
        }
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
