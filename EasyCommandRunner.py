import os
import re
import shutil
import sys
import json
from PyQt5.QtWidgets import *
from PyQt5.QtCore import *
from PyQt5.QtGui import*
from datetime import datetime
import subprocess
import threading

class MyApp(QWidget):

    def __init__(self):
        super().__init__()
        self.current_tab_index = 0  # 记录当前显示的Tab索引
        self.config_file = 'config.json'

        self.initUI()
        if not os.path.exists('stylesheet.css'):
            with open('stylesheet.css', 'w') as f:
                pass
        with open(".\\stylesheet.css", 'r') as f:
            stylesheet = f.read()
        self.setStyleSheet(stylesheet)
        
        self.auto_backup()
        self.load_config()

        self.settings = QSettings("SleepyKanata", "EasyCommandRunner")
        currentIndex = self.settings.value("currentTabIndex", 0)  
        self.tabs.setCurrentIndex(int(currentIndex))

        self.tray_icon = QSystemTrayIcon(QIcon("icon2.ico"), self)
        self.tray_icon.setToolTip("EasyCommandRunner")
        self.tray_icon.activated.connect(self.handle_tray_icon_activated)
        self.tray_icon.show()

        shcNextTAB = QShortcut(QKeySequence(Qt.CTRL + Qt.Key_Right), self)
        shcNextTAB.activated.connect(self.nextTab)

        shcPrevTAB = QShortcut(QKeySequence(Qt.CTRL + Qt.Key_Left), self)
        shcPrevTAB.activated.connect(self.prevTab)

        self.create_menu()

    def create_menu(self):
        with open(".\\stylesheet.css", 'r') as f:
            stylesheet = f.read()
        self.setStyleSheet(stylesheet)
        self.tray_menu = QMenu()
        self.tray_menu.setStyleSheet(stylesheet)

        self.restore_action = QAction("还原", self)
        self.restore_action.triggered.connect(self.restore_window)
        self.tray_menu.addAction(self.restore_action)
        self.minimize_action = QAction("最小化", self)
        self.minimize_action.triggered.connect(self.minimize_to_tray)
        self.tray_menu.addAction(self.minimize_action)
        self.exit_action = QAction("退出", self)
        self.exit_action.triggered.connect(self.handle_exit_action_triggered)
        self.tray_menu.addAction(self.exit_action)
        self.tray_icon.setContextMenu(self.tray_menu)

    def initUI(self):

        self.tabs = QTabWidget()
        self.tabs.setTabsClosable(True)
        self.tabs.setAcceptDrops(True)
        self.tabs.setMovable(True)
        self.tabs.tabCloseRequested.connect(self.close_tab)  

        self.addTabButton = QPushButton("增加标签")
        self.addTabButton.clicked.connect(self.add_new_tab)

        self.saveButton = QPushButton('保存(Ctrl+S)', self)
        self.saveButton.clicked.connect(self.save_config)

        self.reloadConfig = QPushButton("重新加载所有配置")
        self.reloadConfig.clicked.connect(self.on_reload_button)

        self.copyConfig = QPushButton("复制本页配置")
        self.copyConfig.clicked.connect(self.copy_config_to_new_tab)

        if not os.path.exists('stylesheet.css'):
            with open('stylesheet.css', 'w') as f:
                pass
        with open(".\\stylesheet.css", 'r') as f:
            stylesheet = f.read()
        self.tabs.setStyleSheet(stylesheet)
        self.addTabButton.setStyleSheet(stylesheet)
        self.saveButton.setStyleSheet(stylesheet)
        self.reloadConfig.setStyleSheet(stylesheet)
        self.copyConfig.setStyleSheet(stylesheet)

        hboxx = QHBoxLayout()
        hboxx.addWidget(self.addTabButton)
        hboxx.addWidget(self.reloadConfig)
        hboxx.addWidget(self.copyConfig)
        hboxx.addWidget(self.saveButton)

        widget = QWidget()
        widget.setLayout(hboxx)

        vbox = QVBoxLayout()
        vbox.addWidget(self.tabs)
        vbox.addWidget(widget)

        self.setLayout(vbox)

        self.setWindowTitle('EasyCommandRunner')
        self.setGeometry(300, 300, 850, 700)
        self.show()

    def new_line(self, tab, line_codes):
        if not line_codes:
            return
        for line_code in line_codes:
            tab.add_new_line(line_code)

    def add_new_tab(self, line_codes, text_dict=None, checkbox_dict=None):
        tab_index = self.tabs.count() + 1
        tab = MyTab(self)
        self.tabs.addTab(tab, f'标签{tab_index}')
        self.tabs.setCurrentIndex(self.tabs.count() - 1)
        self.new_line(tab, line_codes)
        self.write_text(tab, text_dict, tab_index)
        self.toggle_checkbox(tab, checkbox_dict)

    def load_config(self):
        try:
            with open('config.json', 'r') as f:
                config = json.load(f)
        except FileNotFoundError:
            config = {}
        except json.JSONDecodeError:
            print("无法解析 config.json 文件，将使用默认配置。")
            config = {}

        #建立tab line_codes
        tabs = config.get('tabs', [])
        line_codes_list = config.get('line_codes', [])
        tab_checkbox_statuses_list = config.get('checkbox_statuses', [])

        #循环tab页
        for i, tab in enumerate(tabs):
            line_codes = line_codes_list[i] if i < len(line_codes_list) else []
            tab_checkbox_statuses = tab_checkbox_statuses_list[i] if i < len(tab_checkbox_statuses_list) else {}
            self.add_new_tab(line_codes, tab, tab_checkbox_statuses)
        self.current_tab_index = config.get('current_tab_index', 0)

    def save_config(self):
        config = {
            'current_tab_index': self.tabs.currentIndex(),
            'tabs': [],
            'counters':[],
            'line_codes': [], 
            'checkbox_statuses':[]
        }
        for index in range(self.tabs.count()):
            #初始化
            tab = self.tabs.widget(index)
            tab_config = {}

            #循环获得所有QLineEdit的内容
            line_edits = tab.findChildren(QLineEdit)
            edits_dict = {edit.objectName(): edit for edit in line_edits}
            for field, text_edit in edits_dict.items():
                tab_config[field] = text_edit.text()

            #单独设置描述
            tab_config['editDescription'] = tab.editDescription.toPlainText()

            #设置标题
            tab_name = tab.edit_title.text() 
            if tab_name:
                self.tabs.setTabText(index, tab_name)

            #新行计数器
            config['counters'].append(tab.counter)
            
            #写入单页配置的内容
            config['tabs'].append(tab_config)
            #单独行的index
            config['line_codes'].append(tab.line_codes)

            checkbox_statuses = {}
            checkboxes = tab.findChildren(QCheckBox)
            for checkbox in checkboxes:
                checkbox_statuses[checkbox.objectName()] = checkbox.isChecked()
            config['checkbox_statuses'].append(checkbox_statuses)
            
        #计数tab页数
        config['tab_count'] = self.tabs.count()

        self.auto_backup()
        with open('config.json', 'w') as f:
            json.dump(config, f, indent=4)

        print("配置已保存!")

    def toggle_checkbox(self, tab, checkbox_dicts):
        tab.check_box_toggle(checkbox_dicts)

    def write_text(self, tab, text_dict, tab_index):
        if text_dict:
            for object_name, value in text_dict.items():
                child = tab.findChild(QLineEdit, object_name)
                if child is not None:
                    child.setText(value)
                else:
                    pass

            tab.editDescription.setText(text_dict.get('editDescription', ''))
            tab_name = tab.edit_title.text()

            if not tab_name:
                self.tabs.addTab(tab, f'标签{tab_index}')
            else:
                self.tabs.setTabText(self.tabs.indexOf(tab), tab_name)

    def close_tab(self, index):
        tab = self.tabs.widget(index)
        tab.deleteLater()
        self.tabs.removeTab(index)

    def on_reload_button(self):
        current_tab_index = self.tabs.currentIndex()
        self.tabs.clear()  # 清空当前的Tab部件
        self.load_config()  # 重新加载配置
        self.tabs.setCurrentIndex(current_tab_index)

    def keyPressEvent(self, event):
        if event.key() == Qt.Key_S and QApplication.keyboardModifiers() == Qt.ControlModifier:
            self.save_config()  

    def copy_config_to_new_tab(self):
        # 创建一个新的标签页，并复制当前标签页的配置
        current_tab_index = self.tabs.currentIndex()
        current_tab = self.tabs.widget(current_tab_index)
        
        line_codes = current_tab.line_codes
        text_dict = {}
        
        line_edits = current_tab.findChildren(QLineEdit)
        for edit in line_edits:
            text_dict[edit.objectName()] = edit.text()
            
        text_dict['editDescription'] = current_tab.editDescription.toPlainText()

        new_tab = MyTab(self)
        new_tab_index = current_tab_index + 1  # 插入到当前标签页的后面
        self.tabs.insertTab(new_tab_index, new_tab, f'标签{new_tab_index}')
        self.new_line(new_tab, line_codes)
        self.write_text(new_tab, text_dict, new_tab_index)
        self.tabs.setCurrentIndex(new_tab_index)

    def compare_files(self, file1, file2):
        with open(file1, 'r') as f1, open(file2, 'r') as f2:
            file1_content = f1.read()
            file2_content = f2.read()
        return file1_content == file2_content

    def auto_backup(self):

        self.formatted_datetime = datetime.now().strftime("%Y-%m-%d_%H-%M-%S")
        self.backup_dir = './backup'
        self.destination_file = self.backup_dir + '/config_' + self.formatted_datetime +'.json'

        if os.path.exists('config.json'): # 先判断是否存在配置文件

            if not os.path.exists(self.backup_dir):# 判断是否存在路径 没有则创建路径
                os.makedirs(self.backup_dir)

            files = os.listdir(self.backup_dir)# 获取文件列表

            if not files:# 如果空文件夹 直接复制
                shutil.copy2('config.json', self.destination_file)
            else:
                paths = [os.path.join(self.backup_dir, basename) for basename in files]
                latest_file = max(paths, key=os.path.getctime)

                if not self.compare_files(latest_file, 'config.json'):
                    shutil.copy2('config.json', self.destination_file)

    def minimize_to_tray(self):
        if self.isMinimized():
            self.showNormal()
            self.activateWindow()
        else:
            self.hide()

    def restore_window(self):
        self.showNormal()
        self.activateWindow()

    def handle_tray_icon_activated(self, reason):
        if reason == QSystemTrayIcon.DoubleClick:
            self.restore_window()

    def handle_exit_action_triggered(self):
        self.tray_icon.hide()
        QApplication.quit()

    def prevTab(self):
        tab_index = self.tabs.currentIndex()
        tab_index = (tab_index - 1) % self.tabs.count()
        self.tabs.setCurrentIndex(tab_index)

    def nextTab(self):
        tab_index = self.tabs.currentIndex()
        tab_index = (tab_index + 1) % self.tabs.count()
        self.tabs.setCurrentIndex(tab_index)

    def closeEvent(self, event):
        self.settings = QSettings("SleepyKanata", "EasyCommandRunner")
        self.settings.setValue("currentTabIndex", self.tabs.currentIndex())
        # super(MyApp, self).closeEvent(event)
        event.ignore()
        self.hide()

class MyTab(QWidget):

    def __init__(self, parent):
        super().__init__()
        self.counter = 1
        self.line_codes = {}
        self.parent = parent
        self.initUI()
        with open(".\\stylesheet.css", 'r') as f:
            stylesheet = f.read()
        self.setStyleSheet(stylesheet)

    def initUI(self):
        self.scrollArea = QScrollArea()
        self.scrollWidget = QWidget()
        self.vbox = QVBoxLayout(self.scrollWidget)

        self.label_title = QLabel("标题：")
        self.edit_title = NewQLineEdit(self.parent)
        self.edit_title.setObjectName("name_edit_title")

        self.hbox_title = QHBoxLayout()
        self.hbox_title.addWidget(self.label_title)
        self.hbox_title.addWidget(self.edit_title)

        self.label1 = QLabel("运行路径：")
        self.edit1 = NewQLineEdit(self.parent)
        self.edit1.setPlaceholderText("为空则使用程序当前目录")
        self.edit1.setObjectName("name_edit1")

        self.hbox1 = QHBoxLayout()
        self.hbox1.addWidget(self.label1)
        self.hbox1.addWidget(self.edit1)

        self.label2 = QLabel("程序本体：")
        self.edit2 = NewQLineEdit(self.parent)
        self.edit2.setObjectName("name_edit2")
        self.edit2.setPlaceholderText("粘贴命令可以解析")

        self.analysisBtn = QPushButton("解析")
        self.analysisBtn.setStyleSheet("QPushButton{width:30px;}")
        self.analysisBtn.clicked.connect(self.analysis_command)

        self.hbox2 = QHBoxLayout()
        self.hbox2.addWidget(self.label2)
        self.hbox2.addWidget(self.edit2)
        self.hbox2.addWidget(self.analysisBtn)

        self.chk1 = QCheckBox(self)
        self.chk1.setObjectName("chkbox1") 
        self.chk1.setChecked(True) 
        self.chk1.stateChanged.connect(lambda state: self.toggle_run(self.chk1, state))

        self.edit3_1 = NewQLineEdit(self.parent)
        self.edit3_1.setPlaceholderText("功能1")
        self.edit3_1.setObjectName("name_edit3_1")

        self.label3 = QLabel("：")

        self.edit3_2 = NewQLineEdit(self.parent)
        self.edit3_2.setPlaceholderText("参数1")
        self.edit3_2.setObjectName("name_edit3_2")

        self.edit3_3 = NewQLineEdit(self.parent)
        self.edit3_3.setPlaceholderText("备注1")
        self.edit3_3.setObjectName("name_edit3_3")

        self.hbox3 = QHBoxLayout()
        self.hbox3.addWidget(self.chk1)
        self.hbox3.addWidget(self.edit3_1)
        self.hbox3.addWidget(self.label3)
        self.hbox3.addWidget(self.edit3_2)
        self.hbox3.addWidget(self.edit3_3)

        self.addLineButton = QPushButton(" + ")
        self.addLineButton.clicked.connect(self.add_new_line)

        self.allSelectBtn = QPushButton("全选(Ctrl+A)")
        self.allSelectBtn.clicked.connect(lambda: self.selectAllCheckboxes(True))

        self.celAllSelectBtn = QPushButton("取消全选(Ctrl+D)")
        self.celAllSelectBtn.clicked.connect(lambda: self.selectAllCheckboxes(False))

        self.hbox5 = QHBoxLayout()
        self.hbox5.addWidget(self.addLineButton)
        self.hbox5.addWidget(self.allSelectBtn)
        self.hbox5.addWidget(self.celAllSelectBtn)

        self.editOther = NewQLineEdit(self.parent)
        self.editOther.setPlaceholderText("其他参数")
        self.editOther.setObjectName("name_editOther")

        self.editDescription = NewQTextEdit(self.parent)
        self.editDescription.setPlaceholderText("描述")
        with open(".\\stylesheet.css", 'r') as f:
            stylesheet = f.read()
        self.editDescription.setStyleSheet(stylesheet)

        self.editDescription.setStyleSheet(stylesheet)

        self.commandReview = QTextEdit()
        self.commandReview.setReadOnly(True)
        self.commandReview.viewport().setCursor(Qt.IBeamCursor)

        self.commandReview.setStyleSheet(stylesheet)
        self.commandReview.setPlaceholderText("预生成命令 文件路径有空格不需要前后加上双引号")

        self.hbox4 = QHBoxLayout()

        self.prevTabBtn = QPushButton("←上一个标签页")
        self.prevTabBtn.clicked.connect(self.prevTab)

        self.reviewButton = QPushButton("预生成命令")
        self.reviewButton.clicked.connect(self.on_reviewButton_clicked)

        self.runButton = QPushButton("运行(Ctrl+Enter)")
        self.runButton.clicked.connect(self.run_command)

        self.nextTabBtn =  QPushButton("下一个标签页→")
        self.nextTabBtn.clicked.connect(self.nextTab)
      
        self.vbox.addLayout(self.hbox_title)
        self.vbox.addLayout(self.hbox1) 
        self.vbox.addLayout(self.hbox2)
        self.vbox.addLayout(self.hbox3)
        self.vbox.addLayout(self.hbox5)
        self.vbox.addWidget(self.editOther)
        self.vbox.addWidget(self.editDescription)
        self.vbox.addWidget(self.commandReview)

        self.scrollWidget.setLayout(self.vbox)
        self.scrollArea.setWidget(self.scrollWidget)
        self.scrollArea.setWidgetResizable(True)

        self.buttonContainer = QHBoxLayout()
        self.buttonContainer.addWidget(self.prevTabBtn)
        self.buttonContainer.addWidget(self.reviewButton)
        self.buttonContainer.addWidget(self.runButton)
        self.buttonContainer.addWidget(self.nextTabBtn)

        self.mainLayout = QVBoxLayout() 
        self.mainLayout.addWidget(self.scrollArea)
        self.mainLayout.addLayout(self.buttonContainer)  

        self.setLayout(self.mainLayout)  

    def check_box_toggle(self, checkbox_dict):
        if checkbox_dict:
            for checkbox_name, is_checked in checkbox_dict.items():
                checkbox = self.findChild(QCheckBox, checkbox_name)
                if checkbox is not None:
                    checkbox.setChecked(is_checked)

    def add_new_line(self, line_code=None):
        #判断配置文件是否存在
        if line_code is not None:
            if not isinstance(line_code, bool):
                line_code = int(line_code)
                i = 1
                while i <= line_code:
                    if i == line_code:
                        self.new_line_UI(i,line_code)
                        self.counter += 1
                    i += 1
            else:
                if self.line_codes:
                    maxCounter = max(int(key) for key in self.line_codes)
                else:
                    maxCounter = 1
                self.new_line_UI(int(maxCounter)+1)

    def new_line_UI(self, counter=0, line_code=None):

        chk = QCheckBox(self)
        chk.setChecked(True) 
        chk.setObjectName("chkbox" + str(counter)) 
        chk.stateChanged.connect(lambda state: self.toggle_run(chk, state))
        line_edit1 = NewQLineEdit(self.parent)
        line_edit1.setPlaceholderText("功能" + str(counter))
        line_edit1.setObjectName("function" + str(counter)) 
        
        label = QLabel("：")

        line_edit2 = NewQLineEdit(self.parent)
        line_edit2.setPlaceholderText("参数" + str(counter))
        line_edit2.setObjectName("parameter" + str(counter)) 

        line_edit3 = NewQLineEdit(self.parent)
        line_edit3.setPlaceholderText("备注" + str(counter))
        line_edit3.setObjectName("comment" + str(counter))

        self.rmLineButton = QPushButton(" - ")
        self.rmLineButton.clicked.connect(self.remove_line)
        self.rmLineButton.setObjectName("removeButton" + str(counter))
        self.rmLineButton.setStyleSheet("QPushButton{width:30px;}")

        hbox = QHBoxLayout()
        hbox.setObjectName("exhbox" + str(counter)) 
        hbox.addWidget(chk)
        hbox.addWidget(line_edit1)
        hbox.addWidget(label)
        hbox.addWidget(line_edit2)
        hbox.addWidget(line_edit3)
        hbox.addWidget(self.rmLineButton)

        self.line_codes[str(counter)] = line_code
        self.vbox.insertLayout(self.vbox.count()-4, hbox)

    def toggle_run(self, checkbox, state):
        if state == Qt.Checked:
            pass
            # print(f"   复选框 '{checkbox.objectName()}' 被选中")
        else:
            pass
            # print(f"   复选框 '{checkbox.objectName()}' 被取消")

    def get_command(self):
        command = []
        other = self.editOther.text()
        command.append(self.edit2.text())
        # 遍历所有的vbox
        for i in range(self.vbox.count()):
            hbox = self.vbox.itemAt(i)
            if isinstance(hbox, QHBoxLayout):
                line_edit1 = None
                line_edit2 = None
                # 遍历所有的hbox
                for j in range(hbox.count()):
                    widget = hbox.itemAt(j).widget()
                    
                    if isinstance(widget, QCheckBox) and widget.checkState() == Qt.Checked:
                        line_edit1 = hbox.itemAt(j+1).widget()
                        command.append(line_edit1.text())
                        line_edit2 = hbox.itemAt(j+3).widget()
                        command.append(line_edit2.text())

        command.append(other)
        self.command_string = subprocess.list2cmdline(command).replace("\"\"","")

    def run_command(self):
        path = self.edit1.text()
        description = self.editDescription.toPlainText()

        self.get_command()  
        self.commandReview.setText(self.command_string)  

        if not path:
            path = os.getcwd()

        def run_cmd():
            os.system(f'cd /d {path} && {self.command_string}')

        threading.Thread(target=run_cmd).start()

        print("运行路径：", path)
        print("命令：", self.command_string)
        print("描述：", description)
    
    def keyPressEvent(self, event): 
        if event.key() in (Qt.Key_Return, Qt.Key_Enter) and event.modifiers() & Qt.ControlModifier:
            self.run_command()

        if event.key() == Qt.Key_A and QApplication.keyboardModifiers() == Qt.ControlModifier:
            self.selectAllCheckboxes(True)

        if event.key() == Qt.Key_D and QApplication.keyboardModifiers() == Qt.ControlModifier:
            self.selectAllCheckboxes(False)

        if event.key() == Qt.Key_S and QApplication.keyboardModifiers() == Qt.ControlModifier:
            self.parent.keyPressEvent(event)

        # if event.key() == Qt.Key_Left and QApplication.keyboardModifiers() == Qt.ControlModifier:
        #     self.prevTab()

        # if event.key() == Qt.Key_Right and QApplication.keyboardModifiers() == Qt.ControlModifier:
        #     self.nextTab()

    def on_reviewButton_clicked(self): 
        self.get_command()  
        self.commandReview.setText(self.command_string)  

    def selectAllCheckboxes(self, state):
        checkboxes = []
        for widget in self.findChildren(QCheckBox):
            checkboxes.append(widget)
        for checkbox in checkboxes:
            checkbox.setChecked(state)

    def analysis_command(self):
        s = ''
        s = self.edit2.text()
        parts = re.split(r'( ".+?"| )', s)
        array = s.split()
        array = [part.replace('"', '') for part in parts if part.strip()] 

        buttons = self.findChildren(QPushButton)
        remove_buttons = [button.objectName() for button in buttons if 'removeButton' in button.objectName()]
        
        # 当两个-开头的元素在一起时，中间增加空元素
        new_array = []
        for i in range(len(array)):
            new_array.append(array[i].strip())
            if array[i].startswith(("-", "/")) and i < len(array) - 1 and array[i+1].startswith(("-", "/")):
                new_array.append('')

        # 当第一个元素之后的元素数量是奇数时，最后增加空元素 
        index = 0
        for i in range(1, len(new_array)):
            if new_array[i].startswith(("-", "/")):
                index = i + 1
                break
        if index and index % 2 == 1:
            new_array.insert(index - 1, '')

        # 处理途中
        j = 0
        while j < len(new_array):
            if new_array[j].startswith(("-", "/")):
                counter = 0
                j += 1
                while j < len(new_array) and not new_array[j].startswith(("-", "/")):
                    counter += 1
                    j += 1
                if counter % 2 == 0:
                    new_array.insert(j, '')
            else:
                j += 1

        a = (len(new_array)-3)/2

        if not a.is_integer():
            count = int(a + 0.5 + 1)
        else:
            count = int(a + 1)
        if len(new_array)>1:
            indexs = []
            for remove_button in remove_buttons:
                indexs.append(int(remove_button.replace("removeButton", "")))
            if indexs:
                for index in indexs:
                    self.rm_line(index)

        i = 2
        while i<= count:
            self.add_new_line(str(i))
            i += 1

        # 填入内容
        if len(new_array) > 1:
            self.chk1.setChecked(True) 
            self.editOther.setText("")
            self.commandReview.setText("")
            line_edits = self.findChildren(NewQLineEdit)
            line_edits[4].setText("")
            line_edits[2].setText(new_array[0])
            line_edits[3].setText(new_array[1])
            if len(new_array) > 2:
                line_edits[4].setText(new_array[2])
            
                array_index = 3
                for i in range(7, len(line_edits), 3):
                    try:
                        line_edits[i].setText(new_array[array_index])
                        line_edits[i + 1].setText(new_array[array_index + 1])
                        array_index += 2
                    except IndexError:
                        break

    def remove_line(self):
        sender = self.sender()  # 获取触发点击事件的按钮
        if sender is not None:
            button_name = sender.objectName()  # 获取按钮的对象名称
            index = int(button_name.replace("removeButton", ""))
            self.rm_line(index)

    def prevTab(self):
        self.tab_index = self.parent.tabs.currentIndex()
        self.tab_index = (self.tab_index - 1) % self.parent.tabs.count()
        self.parent.tabs.setCurrentIndex(self.tab_index)

    def nextTab(self):
        self.tab_index = self.parent.tabs.currentIndex()
        self.tab_index = (self.tab_index + 1) % self.parent.tabs.count()
        self.parent.tabs.setCurrentIndex(self.tab_index)

    def rm_line(self, index):
        layout_name = "exhbox" + str(index)

        if str(index) in self.line_codes:
                del self.line_codes[str(index)]
        for i in range(self.vbox.count()):
            item = self.vbox.itemAt(i)
            if item and item.layout() and item.layout().objectName() == layout_name:
                while item.layout().count():
                    sub_item = item.layout().takeAt(0)
                    widget = sub_item.widget()
                    if widget:
                        widget.deleteLater()
                self.vbox.removeItem(item)
                self.counter -= 1
                break

class NewQLineEdit(QLineEdit):
    def __init__(self, myApp, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.setAcceptDrops(True)
        self.myApp = myApp

    def dragEnterEvent(self, event):
        if event.mimeData().hasUrls():
            event.accept()
        else:
            event.ignore()

    def dropEvent(self, event):
        if event.mimeData().hasUrls():
            urls = event.mimeData().urls()
            file_path = urls[0].toLocalFile()
            self.setText(file_path)
        else:
            super(NewQLineEdit, self).keyPressEvent(event) 

    def keyPressEvent(self, event):
        if event.key() == Qt.Key_Right and event.modifiers() == Qt.ControlModifier:
            self.myApp.nextTab()

        if event.key() == Qt.Key_Left and event.modifiers() == Qt.ControlModifier:
            self.myApp.prevTab()
        else:
            super(NewQLineEdit, self).keyPressEvent(event) 

class NewQTextEdit(QTextEdit):
    def __init__(self, myApp, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.setAcceptDrops(True)
        self.myApp = myApp

    def dragEnterEvent(self, event):
        if event.mimeData().hasUrls():
            event.accept()
        else:
            event.ignore()

    def dropEvent(self, event):
        if event.mimeData().hasUrls():
            urls = event.mimeData().urls()
            file_path = urls[0].toLocalFile()
            self.insertPlainText(file_path)
        else:
            super(NewQLineEdit, self).keyPressEvent(event) 

    def keyPressEvent(self, event):
        if event.key() == Qt.Key_Right and event.modifiers() == Qt.ControlModifier:
            self.myApp.nextTab()

        if event.key() == Qt.Key_Left and event.modifiers() == Qt.ControlModifier:
            self.myApp.prevTab()
        else:
            super(NewQTextEdit, self).keyPressEvent(event) 

if __name__ == '__main__':
    app = QApplication(sys.argv)
    app.setWindowIcon(QIcon("icon2.ico"))
    ex = MyApp()
    sys.exit(app.exec_())