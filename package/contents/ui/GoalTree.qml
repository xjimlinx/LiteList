import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts

import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents3

import "../code/Store.js" as Store

Item {
    id: root

    required property var goal
    property int revision: 0
    property color glassSurface
    property color glassRaised
    property color glassBorder
    property color glassShadow
    property color accentWash
    property real cardRadius: Kirigami.Units.cornerRadius * 1.3
    property int motionDuration: Kirigami.Units.shortDuration
    property bool denseMode: false

    signal toggleNode(double nodeId)
    signal editNode(double nodeId)
    signal deleteNode(double nodeId)

    readonly property real nodeWidth: denseMode ? Kirigami.Units.gridUnit * 7.4
                                                : Kirigami.Units.gridUnit * 8.4
    readonly property real nodeHeight: denseMode ? Kirigami.Units.gridUnit * 4.2
                                                 : Kirigami.Units.gridUnit * 5.1
    readonly property real horizontalGap: Kirigami.Units.gridUnit * 1.6
    readonly property real verticalGap: Kirigami.Units.gridUnit * 2.7
    readonly property var layoutData: {
        revision
        return Store.goalLayout(goal, nodeWidth, nodeHeight, horizontalGap, verticalGap)
    }

    function entryFor(nodeId) {
        for (let index = 0; index < layoutData.nodes.length; ++index) {
            if (layoutData.nodes[index].node.id === nodeId)
                return layoutData.nodes[index]
        }
        return null
    }

    function requirementText(node) {
        if (node.requires.length === 0)
            return "起始节点"
        let remaining = 0
        for (let index = 0; index < node.requires.length; ++index) {
            const required = Store.findGoalNode(goal, node.requires[index])
            if (!required || required.completed_at === null)
                ++remaining
        }
        return remaining === 0 ? "前置已满足" : "尚需完成 " + remaining + " 项"
    }

    onLayoutDataChanged: connectionCanvas.requestPaint()
    onRevisionChanged: connectionCanvas.requestPaint()

    Flickable {
        id: treeScroll
        anchors.fill: parent
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        contentWidth: Math.max(width, root.layoutData.width + Kirigami.Units.largeSpacing * 2)
        contentHeight: Math.max(height, root.layoutData.height + Kirigami.Units.largeSpacing)

        QQC2.ScrollBar.horizontal: QQC2.ScrollBar {}
        QQC2.ScrollBar.vertical: QQC2.ScrollBar {}

        Item {
            id: treeContent
            width: treeScroll.contentWidth
            height: treeScroll.contentHeight

            Canvas {
                id: connectionCanvas
                anchors.fill: parent

                onPaint: {
                    const context = getContext("2d")
                    context.reset()
                    context.lineWidth = 2
                    context.lineCap = "round"

                    const offsetX = (treeContent.width - root.layoutData.width) / 2
                    for (let index = 0; index < root.layoutData.nodes.length; ++index) {
                        const entry = root.layoutData.nodes[index]
                        const node = entry.node
                        const targetX = offsetX + entry.x + root.nodeWidth / 2
                        const targetY = entry.y
                        const unlocked = Store.nodeUnlocked(root.goal, node)
                        context.strokeStyle = node.completed_at !== null
                                              ? Kirigami.Theme.highlightColor
                                              : unlocked ? root.glassBorder
                                                         : Qt.rgba(Kirigami.Theme.textColor.r,
                                                                   Kirigami.Theme.textColor.g,
                                                                   Kirigami.Theme.textColor.b, 0.12)

                        if (node.requires.length === 0) {
                            context.beginPath()
                            context.moveTo(treeContent.width / 2, 2)
                            context.lineTo(treeContent.width / 2, targetY / 2)
                            context.lineTo(targetX, targetY / 2)
                            context.lineTo(targetX, targetY)
                            context.stroke()
                            continue
                        }

                        for (let requirementIndex = 0; requirementIndex < node.requires.length; ++requirementIndex) {
                            const source = root.entryFor(node.requires[requirementIndex])
                            if (!source)
                                continue
                            const sourceX = offsetX + source.x + root.nodeWidth / 2
                            const sourceY = source.y + root.nodeHeight
                            const middleY = sourceY + (targetY - sourceY) / 2
                            context.beginPath()
                            context.moveTo(sourceX, sourceY)
                            context.lineTo(sourceX, middleY)
                            context.lineTo(targetX, middleY)
                            context.lineTo(targetX, targetY)
                            context.stroke()
                        }
                    }
                }
            }

            Repeater {
                model: root.layoutData.nodes

                delegate: Kirigami.ShadowedRectangle {
                    id: nodeCard
                    required property var modelData
                    readonly property var node: modelData.node
                    readonly property bool completed: node.completed_at !== null
                    readonly property bool unlocked: Store.nodeUnlocked(root.goal, node)

                    x: (treeContent.width - root.layoutData.width) / 2 + modelData.x
                    y: modelData.y
                    width: root.nodeWidth
                    height: root.nodeHeight
                    radius: root.cardRadius
                    color: completed ? Qt.rgba(Kirigami.Theme.highlightColor.r,
                                               Kirigami.Theme.highlightColor.g,
                                               Kirigami.Theme.highlightColor.b, 0.22)
                                     : nodeMouse.containsMouse && unlocked ? root.glassRaised : root.glassSurface
                    border.width: completed || nodeMouse.containsMouse ? 2 : 1
                    border.color: completed ? Kirigami.Theme.highlightColor
                                           : nodeMouse.containsMouse && unlocked
                                             ? Qt.rgba(Kirigami.Theme.highlightColor.r,
                                                       Kirigami.Theme.highlightColor.g,
                                                       Kirigami.Theme.highlightColor.b, 0.62)
                                             : root.glassBorder
                    opacity: unlocked || completed ? 1 : 0.56
                    shadow.size: nodeMouse.containsMouse && unlocked ? Kirigami.Units.smallSpacing : 0
                    shadow.color: root.glassShadow
                    shadow.yOffset: 2

                    Behavior on color {
                        ColorAnimation { duration: root.motionDuration }
                    }
                    Behavior on opacity {
                        NumberAnimation { duration: root.motionDuration }
                    }

                    Rectangle {
                        anchors.top: parent.top
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.margins: 2
                        height: 3
                        radius: 2
                        color: nodeCard.completed ? Kirigami.Theme.highlightColor
                                                  : nodeCard.unlocked ? root.accentWash : "transparent"
                    }

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: Kirigami.Units.smallSpacing * 1.4
                        spacing: 1
                        z: 1

                        RowLayout {
                            Layout.fillWidth: true
                            Kirigami.Icon {
                                Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
                                Layout.preferredHeight: width
                                source: nodeCard.completed ? "checkmark"
                                                         : nodeCard.unlocked ? "flag" : "lock"
                                color: nodeCard.completed || nodeCard.unlocked
                                       ? Kirigami.Theme.highlightColor : Kirigami.Theme.disabledTextColor
                            }
                            PlasmaComponents3.Label {
                                Layout.fillWidth: true
                                text: nodeCard.node.title
                                elide: Text.ElideRight
                                font.weight: Font.DemiBold
                            }
                            PlasmaComponents3.ToolButton {
                                id: nodeMenuButton
                                icon.name: "overflow-menu"
                                display: QQC2.AbstractButton.IconOnly
                                onClicked: nodeMenu.popup(nodeMenuButton, 0, height)
                            }
                        }

                        PlasmaComponents3.Label {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            visible: nodeCard.node.description.length > 0
                            text: nodeCard.node.description
                            wrapMode: Text.Wrap
                            maximumLineCount: root.denseMode ? 1 : 2
                            elide: Text.ElideRight
                            opacity: 0.72
                            font: Kirigami.Theme.smallFont
                        }
                        PlasmaComponents3.Label {
                            Layout.fillWidth: true
                            text: nodeCard.completed ? "已完成"
                                                    : root.requirementText(nodeCard.node)
                            color: nodeCard.completed || nodeCard.unlocked
                                   ? Kirigami.Theme.highlightColor : Kirigami.Theme.disabledTextColor
                            font: Kirigami.Theme.smallFont
                        }
                    }

                    MouseArea {
                        id: nodeMouse
                        anchors.fill: parent
                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                        hoverEnabled: true
                        cursorShape: nodeCard.unlocked || nodeCard.completed
                                     ? Qt.PointingHandCursor : Qt.ArrowCursor
                        z: 0
                        onClicked: function(mouse) {
                            if (mouse.button === Qt.RightButton)
                                nodeMenu.popup()
                            else if (nodeCard.unlocked || nodeCard.completed)
                                root.toggleNode(nodeCard.node.id)
                        }
                    }

                    QQC2.Menu {
                        id: nodeMenu
                        QQC2.MenuItem {
                            text: nodeCard.completed ? "标记为未完成" : "完成小目标"
                            icon.name: "checkmark"
                            enabled: nodeCard.completed || nodeCard.unlocked
                            onTriggered: root.toggleNode(nodeCard.node.id)
                        }
                        QQC2.MenuItem {
                            text: "编辑"
                            icon.name: "document-edit"
                            onTriggered: root.editNode(nodeCard.node.id)
                        }
                        QQC2.MenuSeparator {}
                        QQC2.MenuItem {
                            text: "删除"
                            icon.name: "edit-delete"
                            onTriggered: root.deleteNode(nodeCard.node.id)
                        }
                    }
                }
            }
        }
    }
}
