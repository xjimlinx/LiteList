import QtQuick
import QtQuick.Controls as QQC2

QQC2.ApplicationWindow {
    id: root
    visible: false

    property double selectedGoalId: 1
    property var goals: [
        { "id": 1, "title": "第一个大目标" },
        { "id": 2, "title": "第二个大目标" }
    ]

    QQC2.Menu {
        id: goalSwitchMenu

        Instantiator {
            model: root.goals
            delegate: QQC2.MenuItem {
                required property var modelData
                text: modelData.title
                checkable: true
                checked: modelData.id === root.selectedGoalId
                onTriggered: root.selectedGoalId = modelData.id
            }
            onObjectAdded: function(index, object) {
                goalSwitchMenu.insertItem(index, object)
            }
            onObjectRemoved: function(index, object) {
                goalSwitchMenu.removeItem(object)
            }
        }
    }

    Timer {
        interval: 0
        running: true
        onTriggered: {
            if (goalSwitchMenu.count !== 2) {
                console.error("Expected two goal menu entries, got", goalSwitchMenu.count)
                Qt.exit(1)
                return
            }
            if (goalSwitchMenu.itemAt(0).text !== "第一个大目标"
                    || goalSwitchMenu.itemAt(1).text !== "第二个大目标") {
                console.error("Goal menu titles were not registered in order")
                Qt.exit(2)
                return
            }
            Qt.quit()
        }
    }
}
