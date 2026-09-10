import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts

import org.kde.kirigami as Kirigami
import org.kde.notification as KNotifications
import org.kde.plasma.components as PlasmaComponents3
import org.kde.plasma.core as PlasmaCore
import org.kde.plasma.plasmoid

import "../code/Store.js" as Store

PlasmoidItem {
    id: root

    property var document: Store.defaultDocument()
    property var history: []
    property int revision: 0
    property bool archiveMode: false
    property int currentPage: 0
    property double selectedGoalId: 0
    property string draft: ""
    property string statusText: "完全本地保存"
    property bool undoAvailable: false

    readonly property real glassAlpha: Math.max(0.25, Math.min(0.95,
        Number(Plasmoid.configuration.glassOpacity) / 100))
    readonly property real cardAlpha: Math.max(0.35, Math.min(1,
        Number(Plasmoid.configuration.cardOpacity) / 100))
    readonly property real accentAlpha: Math.max(0, Math.min(0.45,
        Number(Plasmoid.configuration.accentStrength) / 100))
    readonly property real borderAlpha: Math.max(0, Math.min(0.5,
        Number(Plasmoid.configuration.borderStrength) / 100))
    readonly property real shadowAlpha: Math.max(0, Math.min(0.5,
        Number(Plasmoid.configuration.shadowStrength) / 100))
    readonly property real cornerFactor: Math.max(0.6, Math.min(1.8,
        Number(Plasmoid.configuration.cornerScale) / 100))
    readonly property real spacingFactor: Plasmoid.configuration.denseMode ? 0.68 : 1
    readonly property int motionDuration: Plasmoid.configuration.animationsEnabled
                                          ? Kirigami.Units.shortDuration : 0
    readonly property real panelRadius: Kirigami.Units.cornerRadius * 2 * cornerFactor
    readonly property real cardRadius: Kirigami.Units.cornerRadius * 1.3 * cornerFactor

    readonly property color glassSurface: Qt.rgba(
        Kirigami.Theme.backgroundColor.r,
        Kirigami.Theme.backgroundColor.g,
        Kirigami.Theme.backgroundColor.b,
        glassAlpha)
    readonly property color glassRaised: Qt.rgba(
        Kirigami.Theme.backgroundColor.r,
        Kirigami.Theme.backgroundColor.g,
        Kirigami.Theme.backgroundColor.b,
        cardAlpha)
    readonly property color glassBorder: Qt.rgba(
        Kirigami.Theme.textColor.r,
        Kirigami.Theme.textColor.g,
        Kirigami.Theme.textColor.b,
        borderAlpha)
    readonly property color accentWash: Qt.rgba(
        Kirigami.Theme.highlightColor.r,
        Kirigami.Theme.highlightColor.g,
        Kirigami.Theme.highlightColor.b,
        accentAlpha)
    readonly property color glassShadow: Qt.rgba(0, 0, 0, shadowAlpha)

    readonly property int pendingCount: {
        revision
        return Store.pendingCount(document)
    }
    readonly property int unreadCount: {
        revision
        let count = 0
        for (let index = 0; index < document.notifications.length; ++index) {
            if (!document.notifications[index].acknowledged)
                ++count
        }
        return count
    }
    readonly property var shownTasks: {
        revision
        return Store.visibleTasks(document, archiveMode)
    }
    readonly property var currentGoal: {
        revision
        const selected = Store.findGoal(document, selectedGoalId)
        return selected || (document.goals.length > 0 ? document.goals[0] : null)
    }
    readonly property var currentGoalProgress: {
        revision
        return Store.goalProgress(currentGoal)
    }

    Plasmoid.title: i18n("LiteList")
    Plasmoid.icon: "view-calendar-tasks"
    Plasmoid.status: pendingCount > 0 ? PlasmaCore.Types.ActiveStatus : PlasmaCore.Types.PassiveStatus
    // Desktop plasmoids are embedded in the shell rather than separate KWin
    // windows. NoBackground prevents an opaque theme panel from covering our
    // translucent surface; popups can still receive compositor blur from Plasma.
    Plasmoid.backgroundHints: PlasmaCore.Types.NoBackground
    toolTipMainText: i18n("LiteList")
    toolTipSubText: i18np("%1 item pending", "%1 items pending", pendingCount)
    preferredRepresentation: Plasmoid.formFactor === PlasmaCore.Types.Planar ? fullRepresentation : null

    component GlassToolButton: PlasmaComponents3.ToolButton {
        id: glassButton
        implicitWidth: Kirigami.Units.gridUnit * 2
        implicitHeight: Kirigami.Units.gridUnit * 2
        hoverEnabled: true

        background: Kirigami.ShadowedRectangle {
            radius: root.cardRadius
            color: glassButton.checked ? root.accentWash
                  : glassButton.hovered ? root.glassRaised
                  : Qt.rgba(Kirigami.Theme.backgroundColor.r,
                            Kirigami.Theme.backgroundColor.g,
                            Kirigami.Theme.backgroundColor.b, 0.2)
            border.width: 1
            border.color: glassButton.checked || glassButton.activeFocus
                          ? Qt.rgba(Kirigami.Theme.highlightColor.r,
                                    Kirigami.Theme.highlightColor.g,
                                    Kirigami.Theme.highlightColor.b, 0.48)
                          : root.glassBorder
            shadow.size: glassButton.hovered ? Kirigami.Units.smallSpacing : 0
            shadow.color: root.glassShadow
            shadow.yOffset: 1

            Behavior on color {
                ColorAnimation { duration: root.motionDuration }
            }
        }
    }

    function loadDocument() {
        const loaded = Store.load(Plasmoid.configuration.documentJson)
        document = loaded.document
        if (document.goals.length > 0)
            selectedGoalId = document.goals[0].id
        revision++
        if (loaded.error.length > 0) {
            statusText = "数据读取失败，已使用空清单"
            errorDialog.message = loaded.error
            errorDialog.open()
        }
        checkReminders()
    }

    function saveDocument() {
        Plasmoid.configuration.documentJson = JSON.stringify(document)
    }

    function scheduleSave(message) {
        revision++
        if (message)
            statusText = message
        saveTimer.restart()
    }

    function rememberTasks() {
        history.push(Store.clone(document.tasks))
        if (history.length > 30)
            history.shift()
        undoAvailable = history.length > 0
    }

    function undo() {
        if (history.length === 0)
            return
        document.tasks = history.pop()
        undoAvailable = history.length > 0
        scheduleSave("已撤销")
    }

    function addDraft() {
        if (draft.trim().length === 0)
            return
        rememberTasks()
        const count = Store.addTasks(document, draft)
        draft = ""
        archiveMode = false
        scheduleSave(count > 1 ? "已添加 " + count + " 条待办" : "已添加待办")
    }

    function toggleTask(id) {
        const task = Store.findTask(document, id)
        if (!task)
            return
        rememberTasks()
        task.completed_at = task.completed_at === null ? Store.now() : null
        scheduleSave(task.completed_at === null ? "已恢复待办" : "已完成")
    }

    function deleteTask(id) {
        const task = Store.findTask(document, id)
        if (!task)
            return
        rememberTasks()
        task.deleted_at = Store.now()
        scheduleSave("已删除，可从菜单恢复")
    }

    function restoreDeleted() {
        let newest = null
        for (let index = 0; index < document.tasks.length; ++index) {
            const task = document.tasks[index]
            if (task.deleted_at !== null && (!newest || task.deleted_at > newest.deleted_at))
                newest = task
        }
        if (!newest) {
            statusText = "没有可恢复的任务"
            return
        }
        rememberTasks()
        newest.deleted_at = null
        scheduleSave("已恢复最近删除的任务")
    }

    function moveTask(id, direction) {
        const rows = Store.visibleTasks(document, archiveMode)
        let rowIndex = -1
        for (let index = 0; index < rows.length; ++index) {
            if (rows[index].id === id) {
                rowIndex = index
                break
            }
        }
        const targetIndex = rowIndex + direction
        if (rowIndex < 0 || targetIndex < 0 || targetIndex >= rows.length)
            return
        const source = document.tasks.indexOf(rows[rowIndex])
        const target = document.tasks.indexOf(rows[targetIndex])
        rememberTasks()
        const moved = document.tasks.splice(source, 1)[0]
        document.tasks.splice(target, 0, moved)
        scheduleSave("已调整顺序")
    }

    function openEditor(id) {
        const task = Store.findTask(document, id)
        if (!task)
            return
        editDialog.taskId = id
        editArea.text = task.text
        editDialog.open()
        editArea.forceActiveFocus()
    }

    function openReminder(id) {
        const task = Store.findTask(document, id)
        if (!task)
            return
        reminderDialog.taskId = id
        reminderDialog.taskTitle = task.text
        reminderEnabled.checked = task.reminder ? task.reminder.enabled : false
        dueField.text = task.reminder ? Store.formatLocal(task.reminder.due_at)
                                      : Store.formatLocal(Store.now() + 60 * 60 * 1000)
        earlyField.text = task.reminder && task.reminder.early_at !== null
                        ? Store.formatLocal(task.reminder.early_at) : ""
        earlyNoteField.text = task.reminder ? task.reminder.early_note : ""
        reminderError.text = ""
        reminderDialog.open()
    }

    function saveReminder() {
        const task = Store.findTask(document, reminderDialog.taskId)
        if (!task)
            return
        try {
            const due = Store.parseLocal(dueField.text)
            const early = earlyField.text.trim().length === 0 ? null : Store.parseLocal(earlyField.text)
            if (early !== null && early >= due)
                throw new Error("提前提醒时间必须早于到点时间")
            const old = task.reminder
            rememberTasks()
            task.reminder = {
                enabled: reminderEnabled.checked,
                due_at: due,
                early_at: early,
                early_note: earlyNoteField.text.slice(0, 1000),
                due_sent: old && old.due_at === due ? old.due_sent : false,
                early_sent: old && old.early_at === early ? old.early_sent : false
            }
            task.reminder_configured = true
            task.schedule_checked = true
            reminderDialog.close()
            scheduleSave("提醒设置已保存")
            checkReminders()
        } catch (error) {
            reminderError.text = String(error)
        }
    }

    function addNote() {
        if (document.notes.length >= 200) {
            statusText = "便签已达到 200 张上限"
            return
        }
        let largest = 0
        for (let index = 0; index < document.notes.length; ++index)
            largest = Math.max(largest, document.notes[index].id)
        document.notes.push({
            id: largest + 1,
            title: "桌面便签",
            body: "",
            visible: true,
            topmost: false,
            x: 0, y: 0, width: 360, height: 360
        })
        currentPage = 1
        scheduleSave("已新建便签")
    }

    function deleteNote(id) {
        for (let index = 0; index < document.notes.length; ++index) {
            if (document.notes[index].id === id) {
                document.notes.splice(index, 1)
                scheduleSave("便签已删除")
                return
            }
        }
    }

    function openNewGoal() {
        goalDialog.goalId = 0
        goalTitleField.text = ""
        goalDescriptionField.text = ""
        goalDialog.open()
        goalTitleField.forceActiveFocus()
    }

    function openEditGoal() {
        if (!currentGoal)
            return
        goalDialog.goalId = currentGoal.id
        goalTitleField.text = currentGoal.title
        goalDescriptionField.text = currentGoal.description
        goalDialog.open()
        goalTitleField.forceActiveFocus()
    }

    function saveGoal() {
        const title = goalTitleField.text.replace(/\u0000/g, "").trim().slice(0, 200)
        if (title.length === 0)
            return
        const description = goalDescriptionField.text.replace(/\u0000/g, "").trim().slice(0, 2000)
        if (goalDialog.goalId > 0) {
            const goal = Store.findGoal(document, goalDialog.goalId)
            if (!goal)
                return
            goal.title = title
            goal.description = description
            scheduleSave("已更新大目标")
        } else {
            if (document.goals.length >= 100) {
                statusText = "大目标已达到 100 个上限"
                return
            }
            const goal = {
                id: document.next_goal_id++,
                title: title,
                description: description,
                created_at: Store.now(),
                completed_at: null,
                nodes: []
            }
            document.goals.push(goal)
            selectedGoalId = goal.id
            currentPage = 2
            scheduleSave("已创建大目标")
        }
        goalDialog.close()
    }

    function requestDeleteCurrentGoal() {
        if (!currentGoal)
            return
        deleteGoalDialog.goalId = currentGoal.id
        deleteGoalDialog.goalTitle = currentGoal.title
        deleteGoalDialog.open()
    }

    function deleteGoal(id) {
        for (let index = 0; index < document.goals.length; ++index) {
            if (document.goals[index].id === id) {
                document.goals.splice(index, 1)
                selectedGoalId = document.goals.length > 0 ? document.goals[0].id : 0
                scheduleSave("大目标已删除")
                return
            }
        }
    }

    function openNewGoalNode() {
        if (!currentGoal)
            return
        goalNodeDialog.goalId = currentGoal.id
        goalNodeDialog.requirementIds = []
        nodeTitleField.text = ""
        nodeDescriptionField.text = ""
        goalNodeDialog.open()
        nodeTitleField.forceActiveFocus()
    }

    function saveGoalNode() {
        const goal = Store.findGoal(document, goalNodeDialog.goalId)
        const title = nodeTitleField.text.replace(/\u0000/g, "").trim().slice(0, 200)
        if (!goal || title.length === 0)
            return
        let totalNodes = 0
        for (let index = 0; index < document.goals.length; ++index)
            totalNodes += document.goals[index].nodes.length
        if (totalNodes >= 2000) {
            statusText = "小目标已达到 2000 个上限"
            return
        }
        goal.nodes.push({
            id: document.next_node_id++,
            title: title,
            description: nodeDescriptionField.text.replace(/\u0000/g, "").trim().slice(0, 2000),
            requires: goalNodeDialog.requirementIds.slice(),
            created_at: Store.now(),
            completed_at: null
        })
        goal.completed_at = null
        goalNodeDialog.close()
        scheduleSave("已添加小目标")
    }

    function openEditGoalNode(nodeId) {
        const node = Store.findGoalNode(currentGoal, nodeId)
        if (!node)
            return
        editGoalNodeDialog.nodeId = nodeId
        editNodeTitleField.text = node.title
        editNodeDescriptionField.text = node.description
        editGoalNodeDialog.open()
        editNodeTitleField.forceActiveFocus()
    }

    function saveEditedGoalNode() {
        const node = Store.findGoalNode(currentGoal, editGoalNodeDialog.nodeId)
        const title = editNodeTitleField.text.replace(/\u0000/g, "").trim().slice(0, 200)
        if (!node || title.length === 0)
            return
        node.title = title
        node.description = editNodeDescriptionField.text.replace(/\u0000/g, "").trim().slice(0, 2000)
        editGoalNodeDialog.close()
        scheduleSave("已更新小目标")
    }

    function requestDeleteGoalNode(nodeId) {
        const node = Store.findGoalNode(currentGoal, nodeId)
        if (!node)
            return
        for (let index = 0; index < currentGoal.nodes.length; ++index) {
            if (currentGoal.nodes[index].requires.indexOf(nodeId) >= 0) {
                statusText = "该节点仍是其他小目标的前置条件，无法删除"
                return
            }
        }
        deleteGoalNodeDialog.nodeId = nodeId
        deleteGoalNodeDialog.nodeTitle = node.title
        deleteGoalNodeDialog.open()
    }

    function deleteGoalNode(nodeId) {
        if (!currentGoal)
            return
        for (let index = 0; index < currentGoal.nodes.length; ++index) {
            if (currentGoal.nodes[index].id === nodeId) {
                currentGoal.nodes.splice(index, 1)
                syncGoalCompletion(currentGoal)
                scheduleSave("小目标已删除")
                return
            }
        }
    }

    function syncGoalCompletion(goal) {
        const progress = Store.goalProgress(goal)
        if (progress.total > 0 && progress.completed === progress.total) {
            if (goal.completed_at === null)
                goal.completed_at = Store.now()
        } else {
            goal.completed_at = null
        }
    }

    function toggleGoalNode(nodeId) {
        const goal = currentGoal
        const node = Store.findGoalNode(goal, nodeId)
        if (!goal || !node)
            return
        if (node.completed_at === null) {
            if (!Store.nodeUnlocked(goal, node)) {
                statusText = "请先完成前置小目标"
                return
            }
            node.completed_at = Store.now()
            statusText = "小目标已完成"
        } else {
            for (let index = 0; index < goal.nodes.length; ++index) {
                const dependent = goal.nodes[index]
                if (dependent.completed_at !== null && dependent.requires.indexOf(nodeId) >= 0) {
                    statusText = "已有完成节点依赖它，暂时不能撤回"
                    return
                }
            }
            node.completed_at = null
            statusText = "小目标已恢复"
        }
        syncGoalCompletion(goal)
        scheduleSave(statusText)
    }

    function showNotification(item) {
        const popup = notificationFactory.createObject(root, {
            title: item.title,
            text: item.text
        })
        if (popup)
            popup.sendEvent()
    }

    function checkReminders() {
        const fresh = Store.collectReminders(document, Store.now())
        if (fresh.length === 0)
            return
        for (let index = 0; index < fresh.length; ++index)
            showNotification(fresh[index])
        scheduleSave(fresh.length > 1 ? "有 " + fresh.length + " 条新提醒" : "有一条新提醒")
    }

    function acknowledgeNotifications() {
        for (let index = 0; index < document.notifications.length; ++index)
            document.notifications[index].acknowledged = true
        scheduleSave("提醒已读")
    }

    Component.onCompleted: loadDocument()
    Component.onDestruction: saveDocument()

    Timer {
        id: saveTimer
        interval: 450
        repeat: false
        onTriggered: root.saveDocument()
    }

    Timer {
        interval: 1000
        repeat: true
        running: true
        onTriggered: root.checkReminders()
    }

    Component {
        id: notificationFactory
        KNotifications.Notification {
            componentName: "plasma_workspace"
            eventId: "notification"
            iconName: "view-calendar-tasks"
            flags: KNotifications.Notification.CloseOnTimeout
            urgency: KNotifications.Notification.NormalUrgency
            onClosed: destroy()
        }
    }

    compactRepresentation: MouseArea {
        implicitWidth: Kirigami.Units.gridUnit * 2
        implicitHeight: Kirigami.Units.gridUnit * 2
        hoverEnabled: true
        onClicked: root.expanded = !root.expanded

        Kirigami.Icon {
            anchors.centerIn: parent
            width: Kirigami.Units.iconSizes.medium
            height: width
            source: "view-calendar-tasks"
        }

        Rectangle {
            anchors.fill: parent
            anchors.margins: 1
            radius: width / 2
            color: "transparent"
            border.width: 1
            border.color: root.glassBorder
            z: -1
        }

        Rectangle {
            visible: root.pendingCount > 0
            anchors.right: parent.right
            anchors.top: parent.top
            width: Math.max(Kirigami.Units.smallSpacing * 3, countLabel.implicitWidth + Kirigami.Units.smallSpacing)
            height: Kirigami.Units.smallSpacing * 3
            radius: height / 2
            color: Kirigami.Theme.highlightColor

            PlasmaComponents3.Label {
                id: countLabel
                anchors.centerIn: parent
                text: root.pendingCount > 99 ? "99+" : root.pendingCount
                color: Kirigami.Theme.highlightedTextColor
                font.pixelSize: Kirigami.Theme.smallFont.pixelSize
            }
        }
    }

    fullRepresentation: Item {
        id: fullView
        Layout.minimumWidth: Kirigami.Units.gridUnit * 17
        Layout.minimumHeight: Kirigami.Units.gridUnit * 20
        Layout.preferredWidth: Kirigami.Units.gridUnit * 21
        Layout.preferredHeight: Kirigami.Units.gridUnit * 28

        Kirigami.ShadowedRectangle {
            anchors.fill: parent
            anchors.margins: Kirigami.Units.smallSpacing / 2
            radius: root.panelRadius
            color: root.glassSurface
            border.width: 1
            border.color: root.glassBorder
            shadow.size: Kirigami.Units.largeSpacing
            shadow.color: root.glassShadow
            shadow.yOffset: 3
            z: -2
        }

        Rectangle {
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.margins: Kirigami.Units.largeSpacing
            height: 1
            radius: 1
            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop { position: 0; color: "transparent" }
                GradientStop { position: 0.5; color: Qt.rgba(1, 1, 1, 0.42) }
                GradientStop { position: 1; color: "transparent" }
            }
            z: -1
        }

        Rectangle {
            width: parent.width * 0.58
            height: width
            radius: width / 2
            x: parent.width - width * 0.58
            y: -height * 0.56
            color: root.accentWash
            visible: root.accentAlpha > 0
            z: -1
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: Kirigami.Units.smallSpacing * root.spacingFactor
            spacing: Kirigami.Units.smallSpacing * 1.5 * root.spacingFactor

            RowLayout {
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.smallSpacing
                Layout.rightMargin: Kirigami.Units.smallSpacing

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 0

                    PlasmaComponents3.Label {
                        text: "L I T E L I S T"
                        visible: Plasmoid.configuration.showBrand
                        color: Kirigami.Theme.highlightColor
                        font.pixelSize: Kirigami.Theme.smallFont.pixelSize
                        font.weight: Font.DemiBold
                        opacity: 0.9
                    }
                    PlasmaComponents3.Label {
                        Layout.fillWidth: true
                        text: root.currentPage === 0
                              ? (root.archiveMode ? "已完成" : "近期要做")
                              : root.currentPage === 1 ? "桌面便签" : "目标路线"
                        font.pixelSize: Kirigami.Theme.defaultFont.pixelSize * 1.35
                        font.weight: Font.DemiBold
                    }
                }

                Rectangle {
                    visible: root.currentPage === 0
                    implicitWidth: taskCountLabel.implicitWidth + Kirigami.Units.largeSpacing
                    implicitHeight: Kirigami.Units.gridUnit * 1.55
                    radius: height / 2
                    color: root.accentWash
                    border.width: 1
                    border.color: Qt.rgba(Kirigami.Theme.highlightColor.r,
                                          Kirigami.Theme.highlightColor.g,
                                          Kirigami.Theme.highlightColor.b, 0.3)

                    PlasmaComponents3.Label {
                        id: taskCountLabel
                        anchors.centerIn: parent
                        text: root.archiveMode ? root.shownTasks.length : root.pendingCount
                        color: Kirigami.Theme.highlightColor
                        font.weight: Font.Bold
                    }
                }

                GlassToolButton {
                    icon.name: "view-calendar-tasks"
                    checked: root.currentPage === 0
                    checkable: true
                    text: "待办"
                    display: QQC2.AbstractButton.IconOnly
                    onClicked: root.currentPage = 0
                    PlasmaComponents3.ToolTip.text: text
                }
                GlassToolButton {
                    icon.name: "document-edit"
                    checked: root.currentPage === 1
                    checkable: true
                    text: "便签"
                    display: QQC2.AbstractButton.IconOnly
                    onClicked: root.currentPage = 1
                    PlasmaComponents3.ToolTip.text: text
                }
                GlassToolButton {
                    icon.name: "flag"
                    checked: root.currentPage === 2
                    checkable: true
                    text: "目标路线"
                    display: QQC2.AbstractButton.IconOnly
                    onClicked: root.currentPage = 2
                    PlasmaComponents3.ToolTip.text: text
                }
                GlassToolButton {
                    icon.name: root.unreadCount > 0 ? "notifications" : "notifications-disabled"
                    text: root.unreadCount > 0 ? "未读提醒 " + root.unreadCount : "提醒"
                    display: QQC2.AbstractButton.IconOnly
                    onClicked: notificationDialog.open()
                    PlasmaComponents3.ToolTip.text: text
                }
                GlassToolButton {
                    id: moreButton
                    icon.name: "application-menu"
                    text: "更多"
                    display: QQC2.AbstractButton.IconOnly
                    onClicked: moreMenu.popup(moreButton, 0, moreButton.height)
                    PlasmaComponents3.ToolTip.text: text
                }

                QQC2.Menu {
                    id: moreMenu
                    QQC2.MenuItem {
                        text: "外观设置…"
                        icon.name: "preferences-desktop-color"
                        onTriggered: Plasmoid.internalAction("configure").trigger()
                    }
                    QQC2.MenuSeparator {}
                    QQC2.MenuItem {
                        text: "撤销上一步"
                        icon.name: "edit-undo"
                        enabled: root.undoAvailable
                        onTriggered: root.undo()
                    }
                    QQC2.MenuItem {
                        text: "恢复最近删除的任务"
                        icon.name: "edit-undo"
                        onTriggered: root.restoreDeleted()
                    }
                    QQC2.MenuSeparator {}
                    QQC2.MenuItem {
                        text: "新建便签"
                        icon.name: "document-new"
                        onTriggered: root.addNote()
                    }
                    QQC2.MenuItem {
                        text: "JSON 导入 / 导出"
                        icon.name: "document-import"
                        onTriggered: {
                            dataArea.text = JSON.stringify(root.document, null, 2)
                            dataMessage.text = "编辑或粘贴 JSON 后点击导入；可用 Ctrl+A、Ctrl+C 导出。"
                            dataDialog.open()
                        }
                    }
                    QQC2.MenuItem {
                        text: "使用帮助"
                        icon.name: "help-about"
                        onTriggered: helpDialog.open()
                    }
                }
            }

            StackLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                currentIndex: root.currentPage

                Item {
                    ColumnLayout {
                        anchors.fill: parent
                        spacing: Kirigami.Units.smallSpacing * root.spacingFactor

                        ListView {
                            id: taskList
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            clip: true
                            spacing: Kirigami.Units.smallSpacing * root.spacingFactor
                            model: root.shownTasks

                            Item {
                                anchors.centerIn: parent
                                width: parent.width - Kirigami.Units.largeSpacing * 2
                                height: emptyColumn.implicitHeight
                                visible: taskList.count === 0

                                ColumnLayout {
                                    id: emptyColumn
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    width: parent.width
                                    spacing: Kirigami.Units.smallSpacing

                                    Rectangle {
                                        Layout.alignment: Qt.AlignHCenter
                                        Layout.preferredWidth: Kirigami.Units.gridUnit * 3.4
                                        Layout.preferredHeight: width
                                        radius: width / 2
                                        color: root.accentWash
                                        border.width: 1
                                        border.color: Qt.rgba(Kirigami.Theme.highlightColor.r,
                                                              Kirigami.Theme.highlightColor.g,
                                                              Kirigami.Theme.highlightColor.b, 0.28)

                                        Kirigami.Icon {
                                            anchors.centerIn: parent
                                            width: Kirigami.Units.iconSizes.medium
                                            height: width
                                            source: root.archiveMode ? "checkmark" : "view-calendar-tasks"
                                            opacity: 0.88
                                        }
                                    }

                                    PlasmaComponents3.Label {
                                        Layout.fillWidth: true
                                        Layout.topMargin: Kirigami.Units.smallSpacing
                                        horizontalAlignment: Text.AlignHCenter
                                        text: root.archiveMode ? "还没有已完成的事项" : "现在很轻松"
                                        font.pixelSize: Kirigami.Theme.defaultFont.pixelSize * 1.15
                                        font.weight: Font.DemiBold
                                    }
                                    PlasmaComponents3.Label {
                                        Layout.fillWidth: true
                                        horizontalAlignment: Text.AlignHCenter
                                        text: root.archiveMode ? "完成的事情会留在这里" : "把下一件要做的事写在下方"
                                        opacity: 0.62
                                        font: Kirigami.Theme.smallFont
                                    }
                                    PlasmaComponents3.Button {
                                        Layout.alignment: Qt.AlignHCenter
                                        Layout.topMargin: Kirigami.Units.smallSpacing
                                        visible: root.archiveMode
                                        flat: true
                                        text: "返回待办"
                                        icon.name: "go-previous"
                                        onClicked: root.archiveMode = false
                                    }
                                }
                            }

                            delegate: Kirigami.AbstractCard {
                                id: taskCard
                                required property var modelData
                                required property int index
                                width: taskList.width
                                implicitHeight: taskRow.implicitHeight + Kirigami.Units.smallSpacing * 2
                                hoverEnabled: true

                                background: Kirigami.ShadowedRectangle {
                                    radius: root.cardRadius
                                    color: taskCard.hovered ? root.glassRaised : root.glassSurface
                                    border.width: 1
                                    border.color: taskCard.hovered
                                                  ? Qt.rgba(Kirigami.Theme.highlightColor.r,
                                                            Kirigami.Theme.highlightColor.g,
                                                            Kirigami.Theme.highlightColor.b, 0.38)
                                                  : root.glassBorder
                                    shadow.size: taskCard.hovered ? Kirigami.Units.smallSpacing : 0
                                    shadow.color: root.glassShadow
                                    shadow.yOffset: 2

                                    Behavior on color {
                                        ColorAnimation { duration: root.motionDuration }
                                    }
                                }

                                contentItem: RowLayout {
                                    id: taskRow
                                    spacing: Kirigami.Units.smallSpacing * root.spacingFactor

                                    PlasmaComponents3.CheckBox {
                                        checked: taskCard.modelData.completed_at !== null
                                        onClicked: root.toggleTask(taskCard.modelData.id)
                                    }

                                    ColumnLayout {
                                        Layout.fillWidth: true
                                        spacing: 0

                                        PlasmaComponents3.Label {
                                            id: taskLabel
                                            Layout.fillWidth: true
                                            text: taskCard.modelData.text
                                            wrapMode: Text.Wrap
                                            opacity: taskCard.modelData.completed_at !== null ? 0.65 : 1

                                            MouseArea {
                                                anchors.fill: parent
                                                acceptedButtons: Qt.LeftButton
                                                cursorShape: Qt.PointingHandCursor
                                                onDoubleClicked: root.openEditor(taskCard.modelData.id)
                                            }
                                        }

                                        PlasmaComponents3.Label {
                                            Layout.fillWidth: true
                                            visible: taskCard.modelData.reminder && taskCard.modelData.reminder.enabled
                                            text: visible ? "提醒 " + Store.formatLocal(taskCard.modelData.reminder.due_at)
                                                          + (taskCard.modelData.reminder.early_at !== null ? " · 含提前提醒" : "") : ""
                                            color: Kirigami.Theme.highlightColor
                                            font: Kirigami.Theme.smallFont
                                        }
                                    }

                                    PlasmaComponents3.ToolButton {
                                        id: taskMenuButton
                                        icon.name: "overflow-menu"
                                        text: "任务操作"
                                        display: QQC2.AbstractButton.IconOnly
                                        onClicked: taskMenu.popup(taskMenuButton, 0, taskMenuButton.height)
                                    }

                                    QQC2.Menu {
                                        id: taskMenu
                                        QQC2.MenuItem {
                                            text: "编辑"
                                            icon.name: "document-edit"
                                            onTriggered: root.openEditor(taskCard.modelData.id)
                                        }
                                        QQC2.MenuItem {
                                            text: "提醒设置"
                                            icon.name: "notifications"
                                            onTriggered: root.openReminder(taskCard.modelData.id)
                                        }
                                        QQC2.MenuItem {
                                            text: taskCard.modelData.completed_at === null ? "标记完成" : "恢复为待办"
                                            icon.name: "checkmark"
                                            onTriggered: root.toggleTask(taskCard.modelData.id)
                                        }
                                        QQC2.MenuSeparator {}
                                        QQC2.MenuItem {
                                            text: "上移"
                                            icon.name: "arrow-up"
                                            enabled: taskCard.index > 0
                                            onTriggered: root.moveTask(taskCard.modelData.id, -1)
                                        }
                                        QQC2.MenuItem {
                                            text: "下移"
                                            icon.name: "arrow-down"
                                            enabled: taskCard.index + 1 < root.shownTasks.length
                                            onTriggered: root.moveTask(taskCard.modelData.id, 1)
                                        }
                                        QQC2.MenuSeparator {}
                                        QQC2.MenuItem {
                                            text: "删除"
                                            icon.name: "edit-delete"
                                            onTriggered: root.deleteTask(taskCard.modelData.id)
                                        }
                                    }
                                }
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            visible: !root.archiveMode

                            PlasmaComponents3.TextArea {
                                id: addField
                                Layout.fillWidth: true
                                Layout.preferredHeight: Kirigami.Units.gridUnit * 2.5
                                placeholderText: "添加待办，回车记下"
                                wrapMode: TextEdit.Wrap
                                text: root.draft
                                onTextChanged: root.draft = text
                                background: Kirigami.ShadowedRectangle {
                                    radius: root.cardRadius
                                    color: addField.activeFocus ? root.glassRaised : root.glassSurface
                                    border.width: addField.activeFocus ? 2 : 1
                                    border.color: addField.activeFocus ? Kirigami.Theme.highlightColor : root.glassBorder
                                    shadow.size: addField.activeFocus ? Kirigami.Units.smallSpacing : 0
                                    shadow.color: root.glassShadow
                                }
                                Keys.onReturnPressed: function(event) {
                                    if (!(event.modifiers & Qt.ShiftModifier)) {
                                        root.addDraft()
                                        event.accepted = true
                                    }
                                }
                            }
                            PlasmaComponents3.Button {
                                text: "添加"
                                icon.name: "list-add"
                                highlighted: true
                                enabled: root.draft.trim().length > 0
                                onClicked: root.addDraft()
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            PlasmaComponents3.Button {
                                flat: true
                                text: root.archiveMode ? "返回待办" : "查看已完成"
                                icon.name: root.archiveMode ? "go-previous" : "checkmark"
                                onClicked: root.archiveMode = !root.archiveMode
                            }
                            Item { Layout.fillWidth: true }
                            PlasmaComponents3.Label {
                                text: root.statusText
                                visible: Plasmoid.configuration.showStatus
                                opacity: 0.7
                                font: Kirigami.Theme.smallFont
                                elide: Text.ElideRight
                            }
                        }
                    }
                }

                Item {
                    ColumnLayout {
                        anchors.fill: parent
                        spacing: Kirigami.Units.smallSpacing * root.spacingFactor

                        QQC2.ScrollView {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            clip: true

                            ColumnLayout {
                                width: Math.max(0, fullView.width - Kirigami.Units.largeSpacing)
                                spacing: Kirigami.Units.smallSpacing * root.spacingFactor

                                Kirigami.PlaceholderMessage {
                                    Layout.fillWidth: true
                                    visible: root.document.notes.length === 0
                                    icon.name: "document-edit"
                                    text: "用便签记下灵感、会议要点或购物清单"
                                    helpfulAction: Kirigami.Action {
                                        text: "新建便签"
                                        onTriggered: root.addNote()
                                    }
                                }

                                Repeater {
                                    model: {
                                        root.revision
                                        return root.document.notes
                                    }

                                    delegate: Kirigami.AbstractCard {
                                        id: noteCard
                                        required property var modelData
                                        Layout.fillWidth: true

                                        background: Kirigami.ShadowedRectangle {
                                            radius: root.cardRadius
                                            color: root.glassSurface
                                            border.width: 1
                                            border.color: root.glassBorder
                                            shadow.size: Kirigami.Units.smallSpacing
                                            shadow.color: root.glassShadow
                                            shadow.yOffset: 2
                                        }

                                        contentItem: ColumnLayout {
                                            RowLayout {
                                                Layout.fillWidth: true
                                                PlasmaComponents3.TextField {
                                                    Layout.fillWidth: true
                                                    text: noteCard.modelData.title
                                                    placeholderText: "便签标题"
                                                    onTextEdited: {
                                                        noteCard.modelData.title = text.slice(0, 150)
                                                        saveTimer.restart()
                                                    }
                                                }
                                                PlasmaComponents3.ToolButton {
                                                    icon.name: "edit-delete"
                                                    text: "删除便签"
                                                    display: QQC2.AbstractButton.IconOnly
                                                    onClicked: root.deleteNote(noteCard.modelData.id)
                                                }
                                            }
                                            PlasmaComponents3.TextArea {
                                                Layout.fillWidth: true
                                                Layout.preferredHeight: Kirigami.Units.gridUnit * 7
                                                text: noteCard.modelData.body
                                                placeholderText: "写点什么……"
                                                wrapMode: TextEdit.Wrap
                                                onTextChanged: {
                                                    if (activeFocus) {
                                                        noteCard.modelData.body = text.slice(0, 200000)
                                                        saveTimer.restart()
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        PlasmaComponents3.Button {
                            Layout.alignment: Qt.AlignRight
                            text: "新建便签"
                            icon.name: "document-new"
                            onClicked: root.addNote()
                        }
                    }
                }

                Item {
                    ColumnLayout {
                        anchors.fill: parent
                        spacing: Kirigami.Units.smallSpacing * root.spacingFactor

                        Kirigami.PlaceholderMessage {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            visible: root.document.goals.length === 0
                            icon.name: "flag"
                            text: "建立你的第一条目标路线"
                            explanation: "把大目标拆成带前置条件的小目标，完成一项后解锁下一项。"
                            helpfulAction: Kirigami.Action {
                                text: "新建大目标"
                                icon.name: "list-add"
                                onTriggered: root.openNewGoal()
                            }
                        }

                        Kirigami.AbstractCard {
                            Layout.fillWidth: true
                            visible: root.currentGoal !== null

                            background: Kirigami.ShadowedRectangle {
                                radius: root.cardRadius
                                color: root.glassSurface
                                border.width: 1
                                border.color: root.currentGoal && root.currentGoal.completed_at !== null
                                              ? Kirigami.Theme.highlightColor : root.glassBorder
                                shadow.size: Kirigami.Units.smallSpacing
                                shadow.color: root.glassShadow
                                shadow.yOffset: 2
                            }

                            contentItem: ColumnLayout {
                                spacing: Kirigami.Units.smallSpacing

                                RowLayout {
                                    Layout.fillWidth: true
                                    PlasmaComponents3.ComboBox {
                                        id: goalSelector
                                        Layout.fillWidth: true
                                        model: {
                                            root.revision
                                            return root.document.goals
                                        }
                                        textRole: "title"
                                        currentIndex: {
                                            for (let index = 0; index < root.document.goals.length; ++index) {
                                                if (root.document.goals[index].id === root.currentGoal?.id)
                                                    return index
                                            }
                                            return 0
                                        }
                                        onActivated: function(index) {
                                            if (index >= 0 && index < root.document.goals.length)
                                                root.selectedGoalId = root.document.goals[index].id
                                        }
                                    }
                                    PlasmaComponents3.ToolButton {
                                        id: goalMenuButton
                                        icon.name: "overflow-menu"
                                        text: "目标操作"
                                        display: QQC2.AbstractButton.IconOnly
                                        onClicked: goalMenu.popup(goalMenuButton, 0, height)
                                    }
                                    QQC2.Menu {
                                        id: goalMenu
                                        QQC2.MenuItem {
                                            text: "新建大目标"
                                            icon.name: "list-add"
                                            onTriggered: root.openNewGoal()
                                        }
                                        QQC2.MenuItem {
                                            text: "编辑当前目标"
                                            icon.name: "document-edit"
                                            onTriggered: root.openEditGoal()
                                        }
                                        QQC2.MenuSeparator {}
                                        QQC2.MenuItem {
                                            text: "删除当前目标"
                                            icon.name: "edit-delete"
                                            onTriggered: root.requestDeleteCurrentGoal()
                                        }
                                    }
                                }

                                PlasmaComponents3.Label {
                                    Layout.fillWidth: true
                                    visible: root.currentGoal && root.currentGoal.description.length > 0
                                    text: root.currentGoal ? root.currentGoal.description : ""
                                    wrapMode: Text.Wrap
                                    maximumLineCount: 2
                                    elide: Text.ElideRight
                                    opacity: 0.7
                                    font: Kirigami.Theme.smallFont
                                }

                                RowLayout {
                                    Layout.fillWidth: true
                                    PlasmaComponents3.ProgressBar {
                                        Layout.fillWidth: true
                                        from: 0
                                        to: 1
                                        value: root.currentGoalProgress.ratio
                                    }
                                    PlasmaComponents3.Label {
                                        text: root.currentGoalProgress.completed + " / " + root.currentGoalProgress.total
                                        color: root.currentGoal && root.currentGoal.completed_at !== null
                                               ? Kirigami.Theme.highlightColor : Kirigami.Theme.textColor
                                        font.weight: Font.DemiBold
                                    }
                                }
                            }
                        }

                        GoalTree {
                            id: goalTree
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            visible: root.currentGoal !== null && root.currentGoal.nodes.length > 0
                            goal: root.currentGoal
                            revision: root.revision
                            glassSurface: root.glassSurface
                            glassRaised: root.glassRaised
                            glassBorder: root.glassBorder
                            glassShadow: root.glassShadow
                            accentWash: root.accentWash
                            cardRadius: root.cardRadius
                            motionDuration: root.motionDuration
                            denseMode: Plasmoid.configuration.denseMode
                            onToggleNode: function(nodeId) { root.toggleGoalNode(nodeId) }
                            onEditNode: function(nodeId) { root.openEditGoalNode(nodeId) }
                            onDeleteNode: function(nodeId) { root.requestDeleteGoalNode(nodeId) }
                        }

                        Kirigami.PlaceholderMessage {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            visible: root.currentGoal !== null && root.currentGoal.nodes.length === 0
                            icon.name: "node-add"
                            text: "这个目标还没有路线节点"
                            explanation: "先添加不需要前置条件的起始小目标。"
                            helpfulAction: Kirigami.Action {
                                text: "添加起始节点"
                                icon.name: "list-add"
                                onTriggered: root.openNewGoalNode()
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            visible: root.currentGoal !== null
                            PlasmaComponents3.Label {
                                Layout.fillWidth: true
                                text: root.currentGoal && root.currentGoal.completed_at !== null
                                      ? "路线已全部完成" : "完成前置节点后会解锁后续节点"
                                color: root.currentGoal && root.currentGoal.completed_at !== null
                                       ? Kirigami.Theme.highlightColor : Kirigami.Theme.disabledTextColor
                                font: Kirigami.Theme.smallFont
                                elide: Text.ElideRight
                            }
                            PlasmaComponents3.Button {
                                text: "添加小目标"
                                icon.name: "list-add"
                                onClicked: root.openNewGoalNode()
                            }
                        }
                    }
                }
            }
        }
    }

    QQC2.Dialog {
        id: goalDialog
        property double goalId: 0
        parent: root
        anchors.centerIn: parent
        width: Math.min(root.width - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 27)
        modal: true
        title: goalId > 0 ? "编辑大目标" : "新建大目标"
        standardButtons: QQC2.Dialog.Cancel

        contentItem: ColumnLayout {
            PlasmaComponents3.Label { text: "目标名称" }
            PlasmaComponents3.TextField {
                id: goalTitleField
                Layout.fillWidth: true
                placeholderText: "例如：发布 LiteList 1.0"
                maximumLength: 200
                Keys.onReturnPressed: root.saveGoal()
            }
            PlasmaComponents3.Label { text: "说明（可选）" }
            PlasmaComponents3.TextArea {
                id: goalDescriptionField
                Layout.fillWidth: true
                Layout.preferredHeight: Kirigami.Units.gridUnit * 5
                placeholderText: "写下完成标准或为什么要做这件事"
                wrapMode: TextEdit.Wrap
            }
            PlasmaComponents3.Button {
                Layout.alignment: Qt.AlignRight
                text: goalDialog.goalId > 0 ? "保存" : "创建目标"
                icon.name: "document-save"
                highlighted: true
                enabled: goalTitleField.text.trim().length > 0
                onClicked: root.saveGoal()
            }
        }
    }

    QQC2.Dialog {
        id: goalNodeDialog
        property double goalId: 0
        property var requirementIds: []
        parent: root
        anchors.centerIn: parent
        width: Math.min(root.width - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 29)
        height: Math.min(root.height - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 28)
        modal: true
        title: "添加小目标"
        standardButtons: QQC2.Dialog.Cancel

        contentItem: ColumnLayout {
            PlasmaComponents3.Label { text: "小目标名称" }
            PlasmaComponents3.TextField {
                id: nodeTitleField
                Layout.fillWidth: true
                placeholderText: "例如：完成交互原型"
                maximumLength: 200
            }
            PlasmaComponents3.Label { text: "说明（可选）" }
            PlasmaComponents3.TextArea {
                id: nodeDescriptionField
                Layout.fillWidth: true
                Layout.preferredHeight: Kirigami.Units.gridUnit * 4
                placeholderText: "简要说明该节点的完成标准"
                wrapMode: TextEdit.Wrap
            }
            PlasmaComponents3.Label {
                text: "前置条件（可多选）"
                font.weight: Font.DemiBold
            }
            PlasmaComponents3.Label {
                visible: root.currentGoal && root.currentGoal.nodes.length === 0
                text: "第一个节点无需前置条件。"
                opacity: 0.65
                font: Kirigami.Theme.smallFont
            }
            QQC2.ScrollView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: root.currentGoal && root.currentGoal.nodes.length > 0
                clip: true

                ColumnLayout {
                    width: goalNodeDialog.availableWidth - Kirigami.Units.largeSpacing
                    Repeater {
                        model: root.currentGoal ? root.currentGoal.nodes : []
                        delegate: PlasmaComponents3.CheckBox {
                            required property var modelData
                            Layout.fillWidth: true
                            text: modelData.title + (modelData.completed_at !== null ? " · 已完成" : "")
                            checked: goalNodeDialog.requirementIds.indexOf(modelData.id) >= 0
                            onToggled: {
                                const next = goalNodeDialog.requirementIds.slice()
                                const index = next.indexOf(modelData.id)
                                if (checked && index < 0)
                                    next.push(modelData.id)
                                else if (!checked && index >= 0)
                                    next.splice(index, 1)
                                goalNodeDialog.requirementIds = next
                            }
                        }
                    }
                }
            }
            PlasmaComponents3.Button {
                Layout.alignment: Qt.AlignRight
                text: "添加到路线"
                icon.name: "list-add"
                highlighted: true
                enabled: nodeTitleField.text.trim().length > 0
                onClicked: root.saveGoalNode()
            }
        }
    }

    QQC2.Dialog {
        id: editGoalNodeDialog
        property double nodeId: 0
        parent: root
        anchors.centerIn: parent
        width: Math.min(root.width - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 27)
        modal: true
        title: "编辑小目标"
        standardButtons: QQC2.Dialog.Cancel

        contentItem: ColumnLayout {
            PlasmaComponents3.TextField {
                id: editNodeTitleField
                Layout.fillWidth: true
                placeholderText: "小目标名称"
                maximumLength: 200
            }
            PlasmaComponents3.TextArea {
                id: editNodeDescriptionField
                Layout.fillWidth: true
                Layout.preferredHeight: Kirigami.Units.gridUnit * 5
                placeholderText: "说明（可选）"
                wrapMode: TextEdit.Wrap
            }
            PlasmaComponents3.Label {
                text: "为避免意外形成循环，编辑时不修改前置条件；如需重建关系，请先删除后重新添加。"
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                opacity: 0.65
                font: Kirigami.Theme.smallFont
            }
            PlasmaComponents3.Button {
                Layout.alignment: Qt.AlignRight
                text: "保存"
                icon.name: "document-save"
                highlighted: true
                enabled: editNodeTitleField.text.trim().length > 0
                onClicked: root.saveEditedGoalNode()
            }
        }
    }

    QQC2.Dialog {
        id: deleteGoalDialog
        property double goalId: 0
        property string goalTitle: ""
        parent: root
        anchors.centerIn: parent
        modal: true
        title: "删除大目标"
        standardButtons: QQC2.Dialog.Cancel
        contentItem: ColumnLayout {
            PlasmaComponents3.Label {
                Layout.fillWidth: true
                text: "确定删除“" + deleteGoalDialog.goalTitle + "”及其全部路线节点吗？此操作无法撤销。"
                wrapMode: Text.Wrap
            }
            PlasmaComponents3.Button {
                Layout.alignment: Qt.AlignRight
                text: "确认删除"
                icon.name: "edit-delete"
                onClicked: {
                    root.deleteGoal(deleteGoalDialog.goalId)
                    deleteGoalDialog.close()
                }
            }
        }
    }

    QQC2.Dialog {
        id: deleteGoalNodeDialog
        property double nodeId: 0
        property string nodeTitle: ""
        parent: root
        anchors.centerIn: parent
        modal: true
        title: "删除小目标"
        standardButtons: QQC2.Dialog.Cancel
        contentItem: ColumnLayout {
            PlasmaComponents3.Label {
                Layout.fillWidth: true
                text: "确定删除“" + deleteGoalNodeDialog.nodeTitle + "”吗？"
                wrapMode: Text.Wrap
            }
            PlasmaComponents3.Button {
                Layout.alignment: Qt.AlignRight
                text: "确认删除"
                icon.name: "edit-delete"
                onClicked: {
                    root.deleteGoalNode(deleteGoalNodeDialog.nodeId)
                    deleteGoalNodeDialog.close()
                }
            }
        }
    }

    QQC2.Dialog {
        id: editDialog
        property double taskId: 0
        parent: root
        anchors.centerIn: parent
        width: Math.min(root.width - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 24)
        modal: true
        title: "编辑待办"
        standardButtons: QQC2.Dialog.Ok | QQC2.Dialog.Cancel
        onAccepted: {
            const value = editArea.text.replace(/\u0000/g, "").trim()
            const task = Store.findTask(root.document, taskId)
            if (task && value.length > 0) {
                root.rememberTasks()
                task.text = value.slice(0, 10000)
                root.scheduleSave("已更新待办")
            }
        }

        contentItem: PlasmaComponents3.TextArea {
            id: editArea
            implicitHeight: Kirigami.Units.gridUnit * 7
            wrapMode: TextEdit.Wrap
        }
    }

    QQC2.Dialog {
        id: reminderDialog
        property double taskId: 0
        property string taskTitle: ""
        parent: root
        anchors.centerIn: parent
        width: Math.min(root.width - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 27)
        modal: true
        title: "提醒设置"
        standardButtons: QQC2.Dialog.Cancel

        contentItem: ColumnLayout {
            spacing: Kirigami.Units.smallSpacing
            PlasmaComponents3.Label {
                Layout.fillWidth: true
                text: reminderDialog.taskTitle
                wrapMode: Text.Wrap
                font.weight: Font.DemiBold
            }
            PlasmaComponents3.CheckBox {
                id: reminderEnabled
                text: "启用提醒"
            }
            PlasmaComponents3.Label { text: "到点提醒（本地时间）" }
            PlasmaComponents3.TextField {
                id: dueField
                Layout.fillWidth: true
                enabled: reminderEnabled.checked
                placeholderText: "2026-09-09 19:00"
            }
            PlasmaComponents3.Label { text: "提前提醒（可留空）" }
            PlasmaComponents3.TextField {
                id: earlyField
                Layout.fillWidth: true
                enabled: reminderEnabled.checked
                placeholderText: "2026-09-09 14:00"
            }
            PlasmaComponents3.Label { text: "提前提醒备注（可选）" }
            PlasmaComponents3.TextField {
                id: earlyNoteField
                Layout.fillWidth: true
                enabled: reminderEnabled.checked
                placeholderText: "例如：提前打印简历"
            }
            PlasmaComponents3.Label {
                id: reminderError
                Layout.fillWidth: true
                color: Kirigami.Theme.negativeTextColor
                wrapMode: Text.Wrap
            }
            PlasmaComponents3.Button {
                Layout.alignment: Qt.AlignRight
                text: "保存提醒"
                icon.name: "document-save"
                onClicked: root.saveReminder()
            }
        }
    }

    QQC2.Dialog {
        id: notificationDialog
        parent: root
        anchors.centerIn: parent
        width: Math.min(root.width - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 27)
        height: Math.min(root.height - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 24)
        modal: true
        title: "提醒记录"
        standardButtons: QQC2.Dialog.Close
        onClosed: root.acknowledgeNotifications()

        contentItem: QQC2.ScrollView {
            clip: true
            ColumnLayout {
                width: notificationDialog.availableWidth
                Kirigami.PlaceholderMessage {
                    Layout.fillWidth: true
                    visible: root.document.notifications.length === 0
                    text: "还没有提醒记录"
                    icon.name: "notifications-disabled"
                }
                Repeater {
                    model: {
                        root.revision
                        return root.document.notifications.slice().reverse()
                    }
                    delegate: Kirigami.AbstractCard {
                        required property var modelData
                        Layout.fillWidth: true
                        contentItem: ColumnLayout {
                            PlasmaComponents3.Label {
                                Layout.fillWidth: true
                                text: modelData.title
                                font.weight: Font.DemiBold
                            }
                            PlasmaComponents3.Label {
                                Layout.fillWidth: true
                                text: modelData.text
                                wrapMode: Text.Wrap
                            }
                        }
                    }
                }
            }
        }
    }

    QQC2.Dialog {
        id: dataDialog
        parent: root
        anchors.centerIn: parent
        width: Math.min(root.width - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 29)
        height: Math.min(root.height - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 25)
        modal: true
        title: "JSON 导入 / 导出"
        standardButtons: QQC2.Dialog.Close

        contentItem: ColumnLayout {
            PlasmaComponents3.Label {
                id: dataMessage
                Layout.fillWidth: true
                wrapMode: Text.Wrap
            }
            PlasmaComponents3.TextArea {
                id: dataArea
                Layout.fillWidth: true
                Layout.fillHeight: true
                wrapMode: TextEdit.NoWrap
                font.family: "monospace"
            }
            RowLayout {
                Layout.fillWidth: true
                PlasmaComponents3.Button {
                    text: "恢复导入前备份"
                    enabled: Plasmoid.configuration.backupJson.length > 0
                    onClicked: {
                        dataArea.text = Plasmoid.configuration.backupJson
                        dataMessage.text = "已载入导入前备份；点击“从上方 JSON 导入”确认恢复。"
                    }
                }
                Item { Layout.fillWidth: true }
                PlasmaComponents3.Button {
                    text: "从上方 JSON 导入"
                    icon.name: "document-import"
                    onClicked: {
                        const loaded = Store.load(dataArea.text)
                        if (loaded.error.length > 0) {
                            dataMessage.text = "无法导入：" + loaded.error
                            return
                        }
                        Plasmoid.configuration.backupJson = JSON.stringify(root.document)
                        root.document = loaded.document
                        root.history = []
                        root.undoAvailable = false
                        root.scheduleSave("已导入 JSON 数据")
                        dataMessage.text = "导入成功，原数据已保留为导入前备份。"
                    }
                }
            }
        }
    }

    QQC2.Dialog {
        id: helpDialog
        parent: root
        anchors.centerIn: parent
        width: Math.min(root.width - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 26)
        modal: true
        title: "关于轻单"
        standardButtons: QQC2.Dialog.Close
        contentItem: PlasmaComponents3.Label {
            wrapMode: Text.Wrap
            text: "LiteList 0.5 · KDE Plasma 6\n\n"
                + "输入待办后按回车添加；任务菜单中可编辑、排序、设置提醒或删除。"
                + "“已完成”页面保留完成记录，删除的任务可从右上角菜单恢复。\n\n"
                + "目标路线可把大目标拆为带前置条件的小目标，并以科技树方式显示解锁关系。\n\n"
                + "便签与清单保存在当前 Plasma 小部件的配置中，不需要登录或联网。"
                + "提醒由 KDE 通知系统显示；Plasma 必须保持运行。"
        }
    }

    QQC2.Dialog {
        id: errorDialog
        property string message: ""
        parent: root
        anchors.centerIn: parent
        width: Math.min(root.width - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 24)
        modal: true
        title: "LiteList 数据错误"
        standardButtons: QQC2.Dialog.Close
        contentItem: PlasmaComponents3.Label {
            text: errorDialog.message
            wrapMode: Text.Wrap
        }
    }
}
