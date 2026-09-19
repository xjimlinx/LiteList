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
    property int nodeShape: 0
    property int nodeSize: 1
    property bool autoCompact: true

    signal toggleNode(double nodeId)
    signal editNode(double nodeId)
    signal deleteNode(double nodeId)

    readonly property int nodeCount: goal ? goal.nodes.length : 0
    readonly property bool compactLayout: denseMode || (autoCompact && nodeCount > 8)
    readonly property int effectiveNodeSize: compactLayout ? 0 : Math.max(0, Math.min(2, nodeSize))
    readonly property bool terminated: Store.goalTerminated(goal)
    readonly property real nodeWidth: effectiveNodeSize === 0 ? Kirigami.Units.gridUnit * 6.3
                                            : effectiveNodeSize === 2 ? Kirigami.Units.gridUnit * 10.8
                                                                      : Kirigami.Units.gridUnit * 8.2
    readonly property real nodeHeight: effectiveNodeSize === 0 ? Kirigami.Units.gridUnit * 4.6
                                             : effectiveNodeSize === 2 ? Kirigami.Units.gridUnit * 7.2
                                                                       : Kirigami.Units.gridUnit * 5.7
    readonly property real horizontalGap: Kirigami.Units.gridUnit
                                          * (compactLayout ? 0.95 : 1.35)
    readonly property real verticalGap: Kirigami.Units.gridUnit
                                        * (compactLayout ? 1.35 : 2.15)
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
        if (terminated)
            return "目标已终止"
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
        interactive: false
        contentWidth: Math.max(width, root.layoutData.width + Kirigami.Units.largeSpacing * 2)
        contentHeight: Math.max(height, root.layoutData.height + Kirigami.Units.largeSpacing)

        QQC2.ScrollBar.horizontal: QQC2.ScrollBar {}

        Item {
            id: treeContent
            width: treeScroll.contentWidth
            height: treeScroll.contentHeight

            MouseArea {
                id: canvasPanArea
                anchors.fill: parent
                enabled: treeScroll.contentWidth > treeScroll.width + 1
                acceptedButtons: Qt.LeftButton | Qt.MiddleButton
                hoverEnabled: true
                cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor

                property real pressViewportX: 0
                property real pressContentX: 0

                onPressed: function(mouse) {
                    const point = mapToItem(root, mouse.x, mouse.y)
                    pressViewportX = point.x
                    pressContentX = treeScroll.contentX
                }
                onPositionChanged: function(mouse) {
                    if (!pressed)
                        return
                    const point = mapToItem(root, mouse.x, mouse.y)
                    const maximumX = Math.max(0, treeScroll.contentWidth - treeScroll.width)
                    treeScroll.contentX = Math.max(0, Math.min(maximumX,
                        pressContentX - (point.x - pressViewportX)))
                }
            }

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
                        if (node.requires.length > 0)
                            continue
                        const targetX = offsetX + entry.x + root.nodeWidth / 2
                        const targetY = entry.y
                        const unlocked = Store.nodeUnlocked(root.goal, node)
                        context.strokeStyle = node.completed_at !== null
                                              ? Qt.rgba(Kirigami.Theme.highlightColor.r,
                                                        Kirigami.Theme.highlightColor.g,
                                                        Kirigami.Theme.highlightColor.b, 0.68)
                                              : unlocked ? root.glassBorder
                                                         : Qt.rgba(Kirigami.Theme.textColor.r,
                                                                   Kirigami.Theme.textColor.g,
                                                                   Kirigami.Theme.textColor.b, 0.12)

                        context.beginPath()
                        context.moveTo(treeContent.width / 2, 2)
                        context.lineTo(treeContent.width / 2, targetY / 2)
                        context.lineTo(targetX, targetY / 2)
                        context.lineTo(targetX, targetY)
                        context.stroke()
                    }

                    const edges = root.layoutData.edges || []
                    for (let index = 0; index < edges.length; ++index) {
                        const edge = edges[index]
                        const source = root.entryFor(edge.sourceId)
                        const target = root.entryFor(edge.targetId)
                        if (!source || !target)
                            continue
                        const sourceX = offsetX + source.x + root.nodeWidth / 2
                        const sourceY = source.y + root.nodeHeight
                        const targetX = offsetX + target.x + root.nodeWidth / 2
                        const targetY = target.y
                        const unlocked = Store.nodeUnlocked(root.goal, target.node)
                        context.strokeStyle = target.node.completed_at !== null
                                              ? Qt.rgba(Kirigami.Theme.highlightColor.r,
                                                        Kirigami.Theme.highlightColor.g,
                                                        Kirigami.Theme.highlightColor.b, 0.68)
                                              : unlocked ? root.glassBorder
                                                         : Qt.rgba(Kirigami.Theme.textColor.r,
                                                                   Kirigami.Theme.textColor.g,
                                                                   Kirigami.Theme.textColor.b, 0.12)
                        context.beginPath()
                        context.moveTo(sourceX, sourceY)
                        if (edge.sameRank) {
                            const laneY = source.y + root.nodeHeight
                                          + root.verticalGap * (0.24 + (edge.lane % 3) * 0.13)
                            context.lineTo(sourceX, laneY)
                            context.lineTo(targetX, laneY)
                        } else if (edge.waypoints && edge.waypoints.length > 0) {
                            let currentX = sourceX
                            for (let pointIndex = 0; pointIndex < edge.waypoints.length; ++pointIndex) {
                                const point = edge.waypoints[pointIndex]
                                const pointX = offsetX + point.x
                                const aboveY = point.y - root.verticalGap * 0.45
                                const belowY = point.y + root.nodeHeight + root.verticalGap * 0.45
                                context.lineTo(currentX, aboveY)
                                context.lineTo(pointX, aboveY)
                                context.lineTo(pointX, belowY)
                                currentX = pointX
                            }
                            const targetGapY = targetY - root.verticalGap * 0.45
                            context.lineTo(currentX, targetGapY)
                            context.lineTo(targetX, targetGapY)
                        } else {
                            const middleY = sourceY + (targetY - sourceY) / 2
                            context.lineTo(sourceX, middleY)
                            context.lineTo(targetX, middleY)
                        }
                        context.lineTo(targetX, targetY)
                        context.stroke()
                    }

                    // The cards intentionally keep a glass-like background. Remove
                    // line pixels inside their exact outlines so transparent cards do
                    // not make a connector appear to pass through a node.
                    context.save()
                    context.globalCompositeOperation = "destination-out"
                    for (let index = 0; index < root.layoutData.nodes.length; ++index) {
                        const entry = root.layoutData.nodes[index]
                        const x = offsetX + entry.x
                        const y = entry.y
                        const sourceShape = Number(entry.node.shape)
                        const shape = Number.isInteger(sourceShape)
                                      && sourceShape >= 0 && sourceShape <= 2
                                      ? sourceShape : root.nodeShape
                        const radius = shape === 1 ? 0
                                       : shape === 2 ? root.nodeHeight / 2 : root.cardRadius
                        context.beginPath()
                        if (radius <= 1) {
                            context.rect(x, y, root.nodeWidth, root.nodeHeight)
                        } else {
                            const clampedRadius = Math.min(radius, root.nodeWidth / 2,
                                                           root.nodeHeight / 2)
                            context.moveTo(x + clampedRadius, y)
                            context.lineTo(x + root.nodeWidth - clampedRadius, y)
                            context.quadraticCurveTo(x + root.nodeWidth, y,
                                                     x + root.nodeWidth, y + clampedRadius)
                            context.lineTo(x + root.nodeWidth, y + root.nodeHeight - clampedRadius)
                            context.quadraticCurveTo(x + root.nodeWidth, y + root.nodeHeight,
                                                     x + root.nodeWidth - clampedRadius,
                                                     y + root.nodeHeight)
                            context.lineTo(x + clampedRadius, y + root.nodeHeight)
                            context.quadraticCurveTo(x, y + root.nodeHeight,
                                                     x, y + root.nodeHeight - clampedRadius)
                            context.lineTo(x, y + clampedRadius)
                            context.quadraticCurveTo(x, y, x + clampedRadius, y)
                            context.closePath()
                        }
                        context.fill()
                    }
                    context.restore()
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
                    readonly property bool showDescription: !root.compactLayout
                                                            && node.description.length > 0
                    readonly property int effectiveShape: Number.isInteger(node.shape)
                                                                  && node.shape >= 0 && node.shape <= 2
                                                          ? node.shape : root.nodeShape
                    readonly property bool textTruncated: nodeTitleLabel.truncated
                                                          || (showDescription
                                                              && nodeDescriptionLabel.truncated)
                                                          || (!showDescription
                                                              && node.description.length > 0)

                    x: (treeContent.width - root.layoutData.width) / 2 + modelData.x
                    y: modelData.y
                    width: root.nodeWidth
                    height: root.nodeHeight
                    radius: effectiveShape === 1 ? 1
                              : effectiveShape === 2 ? height / 2 : root.cardRadius
                    color: completed ? Qt.rgba(Kirigami.Theme.highlightColor.r,
                                               Kirigami.Theme.highlightColor.g,
                                               Kirigami.Theme.highlightColor.b, 0.12)
                                     : nodeMouse.containsMouse && unlocked ? root.glassRaised : root.glassSurface
                    border.width: nodeMouse.containsMouse ? 2 : 1
                    border.color: completed ? Qt.rgba(Kirigami.Theme.highlightColor.r,
                                                      Kirigami.Theme.highlightColor.g,
                                                      Kirigami.Theme.highlightColor.b, 0.68)
                                           : nodeMouse.containsMouse && unlocked
                                             ? Qt.rgba(Kirigami.Theme.highlightColor.r,
                                                       Kirigami.Theme.highlightColor.g,
                                                       Kirigami.Theme.highlightColor.b, 0.62)
                                             : root.glassBorder
                    opacity: unlocked || completed ? 1 : 0.56
                    shadow.size: nodeMouse.containsMouse && unlocked ? Kirigami.Units.smallSpacing : 0
                    shadow.color: root.glassShadow
                    shadow.yOffset: 2

                    PlasmaComponents3.ToolTip.visible: nodeMouse.containsMouse && textTruncated
                    PlasmaComponents3.ToolTip.text: node.title
                        + (node.description.length > 0 ? "\n\n" + node.description : "")

                    Behavior on color {
                        ColorAnimation { duration: root.motionDuration }
                    }
                    Behavior on opacity {
                        NumberAnimation { duration: root.motionDuration }
                    }

                    Rectangle {
                        anchors.top: parent.top
                        anchors.horizontalCenter: parent.horizontalCenter
                        anchors.topMargin: nodeCard.effectiveShape === 2
                                           ? Kirigami.Units.smallSpacing : 2
                        width: nodeCard.effectiveShape === 2
                               ? parent.width * 0.28 : parent.width - 4
                        height: root.compactLayout ? 2 : 3
                        radius: 2
                        color: nodeCard.completed ? Kirigami.Theme.highlightColor
                                                  : nodeCard.unlocked ? root.accentWash : "transparent"
                    }

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.topMargin: Kirigami.Units.smallSpacing
                        anchors.bottomMargin: Kirigami.Units.smallSpacing
                        anchors.leftMargin: nodeCard.effectiveShape === 2
                                            ? Kirigami.Units.gridUnit * (root.compactLayout ? 1.2 : 1.6)
                                            : Kirigami.Units.smallSpacing * (root.compactLayout ? 1 : 1.4)
                        anchors.rightMargin: anchors.leftMargin
                        spacing: 1
                        z: 1

                        RowLayout {
                            Layout.fillWidth: true
                            PlasmaComponents3.Label {
                                id: nodeTitleLabel
                                Layout.fillWidth: true
                                text: nodeCard.node.title
                                wrapMode: Text.Wrap
                                maximumLineCount: root.effectiveNodeSize === 0 ? 1
                                                    : root.effectiveNodeSize === 2 ? 3 : 2
                                elide: Text.ElideRight
                                font.pixelSize: root.compactLayout
                                                ? Kirigami.Theme.smallFont.pixelSize
                                                : Kirigami.Theme.defaultFont.pixelSize
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
                            id: nodeDescriptionLabel
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            visible: nodeCard.showDescription
                            text: nodeCard.node.description
                            wrapMode: Text.Wrap
                            maximumLineCount: root.effectiveNodeSize === 0 ? 1
                                                : root.effectiveNodeSize === 2 ? 4 : 2
                            elide: Text.ElideRight
                            opacity: 0.72
                            font: Kirigami.Theme.smallFont
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Kirigami.Units.smallSpacing

                            PlasmaComponents3.Label {
                                Layout.fillWidth: true
                                text: nodeCard.completed ? "已完成"
                                                        : root.requirementText(nodeCard.node)
                                color: nodeCard.completed || nodeCard.unlocked
                                       ? Kirigami.Theme.highlightColor
                                       : Kirigami.Theme.disabledTextColor
                                font: Kirigami.Theme.smallFont
                                elide: Text.ElideRight
                            }
                            PlasmaComponents3.ToolButton {
                                id: completionButton
                                Layout.preferredWidth: Kirigami.Units.gridUnit
                                                       * (root.compactLayout ? 1.3 : 1.55)
                                Layout.preferredHeight: width
                                Layout.rightMargin: nodeCard.effectiveShape === 2
                                                    ? Kirigami.Units.gridUnit
                                                      * (root.compactLayout ? 0.45 : 0.7) : 0
                                icon.name: root.terminated ? "process-stop"
                                           : nodeCard.completed ? "edit-undo"
                                           : nodeCard.unlocked ? "checkmark" : "lock"
                                text: root.terminated ? "目标已终止"
                                      : nodeCard.completed ? "撤回完成"
                                      : nodeCard.unlocked ? "完成小目标" : "前置条件尚未完成"
                                display: QQC2.AbstractButton.IconOnly
                                enabled: !root.terminated && (nodeCard.completed || nodeCard.unlocked)
                                onClicked: root.toggleNode(nodeCard.node.id)
                                PlasmaComponents3.ToolTip.text: text

                                background: Rectangle {
                                    radius: root.cardRadius
                                    color: completionButton.enabled ? root.accentWash : "transparent"
                                    border.width: 1
                                    border.color: completionButton.enabled
                                                  ? Qt.rgba(Kirigami.Theme.highlightColor.r,
                                                            Kirigami.Theme.highlightColor.g,
                                                            Kirigami.Theme.highlightColor.b, 0.68)
                                                  : root.glassBorder
                                }
                            }
                        }
                    }

                    MouseArea {
                        id: nodeMouse
                        anchors.fill: parent
                        acceptedButtons: Qt.RightButton
                        hoverEnabled: true
                        cursorShape: Qt.ArrowCursor
                        z: 0
                        onClicked: function(mouse) {
                            if (mouse.button === Qt.RightButton)
                                nodeMenu.popup()
                        }
                    }

                    QQC2.Menu {
                        id: nodeMenu
                        QQC2.MenuItem {
                            text: nodeCard.completed ? "标记为未完成" : "完成小目标"
                            icon.name: "checkmark"
                            enabled: !root.terminated && (nodeCard.completed || nodeCard.unlocked)
                            onTriggered: root.toggleNode(nodeCard.node.id)
                        }
                        QQC2.MenuItem {
                            text: "编辑"
                            icon.name: "document-edit"
                            enabled: !root.terminated
                            onTriggered: root.editNode(nodeCard.node.id)
                        }
                        QQC2.MenuSeparator {}
                        QQC2.MenuItem {
                            text: "删除"
                            icon.name: "edit-delete"
                            enabled: !root.terminated
                            onTriggered: root.deleteNode(nodeCard.node.id)
                        }
                    }
                }
            }
        }
    }
}
