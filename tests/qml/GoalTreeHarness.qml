import QtQuick
import QtQuick.Window

import org.kde.kirigami as Kirigami

import "../../package/contents/ui"

Window {
    id: window
    width: 760
    height: 560
    visible: true
    color: Kirigami.Theme.backgroundColor

    GoalTree {
        anchors.fill: parent
        anchors.margins: 12
        goal: ({
            id: 1,
            title: "发布 Plasma 版本",
            description: "目标树运行测试",
            nodes: [
                { id: 1, title: "完成整体交互与视觉设计", description: "确定目标路线、依赖连线和长文字展示规则", requires: [], shape: 0, completed_at: Date.now() },
                { id: 2, title: "实现 Plasma 小部件核心功能", description: "编写并验证 QML 组件", requires: [1], shape: 1, completed_at: null },
                { id: 3, title: "编写完整安装与使用文档", description: "整理更新、配置和故障排查说明", requires: [1], shape: 2, completed_at: null },
                { id: 4, title: "发布首个公开版本", description: "合并、打包并发布", requires: [2, 3], shape: -1, completed_at: null }
            ]
        })
        revision: 1
        glassSurface: Qt.rgba(0.12, 0.14, 0.18, 0.68)
        glassRaised: Qt.rgba(0.18, 0.21, 0.27, 0.86)
        glassBorder: Qt.rgba(1, 1, 1, 0.16)
        glassShadow: Qt.rgba(0, 0, 0, 0.22)
        accentWash: Qt.rgba(0.3, 0.62, 1, 0.18)
        nodeSize: 1
    }

    Timer {
        interval: 800
        running: true
        onTriggered: window.contentItem.grabToImage(function(result) {
            result.saveToFile("/tmp/litelist-goal-tree.png")
            Qt.quit()
        })
    }
}
