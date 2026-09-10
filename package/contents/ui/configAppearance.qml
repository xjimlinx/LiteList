import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import org.kde.kcmutils as KCM
import org.kde.kirigami as Kirigami

KCM.SimpleKCM {
    id: page

    property alias cfg_glassOpacity: glassOpacity.value
    property alias cfg_cardOpacity: cardOpacity.value
    property alias cfg_accentStrength: accentStrength.value
    property alias cfg_showDecorativeGlow: showDecorativeGlow.checked
    property alias cfg_decorativeGlowSize: decorativeGlowSize.value
    property alias cfg_goalNodeShape: goalNodeShape.currentIndex
    property alias cfg_borderStrength: borderStrength.value
    property alias cfg_shadowStrength: shadowStrength.value
    property alias cfg_cornerScale: cornerScale.value
    property alias cfg_denseMode: denseMode.checked
    property alias cfg_animationsEnabled: animationsEnabled.checked
    property alias cfg_showBrand: showBrand.checked
    property alias cfg_showStatus: showStatus.checked

    function alphaColor(color, alpha) {
        return Qt.rgba(color.r, color.g, color.b, alpha)
    }

    function applyPreset(glass, card, accent, border, shadow, corner) {
        glassOpacity.value = glass
        cardOpacity.value = card
        accentStrength.value = accent
        borderStrength.value = border
        shadowStrength.value = shadow
        cornerScale.value = corner
    }

    Kirigami.FormLayout {
        anchors.left: parent.left
        anchors.right: parent.right

        Kirigami.ShadowedRectangle {
            Kirigami.FormData.isSection: true
            Layout.fillWidth: true
            Layout.preferredHeight: Kirigami.Units.gridUnit * 7
            Layout.bottomMargin: Kirigami.Units.largeSpacing
            radius: Kirigami.Units.cornerRadius * cornerScale.value / 100
            color: page.alphaColor(Kirigami.Theme.backgroundColor, glassOpacity.value / 100)
            border.width: 1
            border.color: page.alphaColor(Kirigami.Theme.textColor, borderStrength.value / 100)
            shadow.size: Kirigami.Units.largeSpacing
            shadow.color: Qt.rgba(0, 0, 0, shadowStrength.value / 100)
            shadow.yOffset: 3
            clip: true

            Rectangle {
                width: parent.width * decorativeGlowSize.value / 100
                height: width
                radius: width / 2
                x: parent.width - width * 0.58
                y: -height * 0.56
                color: page.alphaColor(Kirigami.Theme.highlightColor, accentStrength.value / 100)
                visible: showDecorativeGlow.checked && accentStrength.value > 0
            }

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: Kirigami.Units.largeSpacing
                spacing: Kirigami.Units.smallSpacing

                Label {
                    text: showBrand.checked ? "L I T E L I S T" : "外观预览"
                    color: Kirigami.Theme.highlightColor
                    font.pixelSize: Kirigami.Theme.smallFont.pixelSize
                    font.weight: Font.DemiBold
                }
                Label {
                    text: "近期要做"
                    font.pixelSize: Kirigami.Theme.defaultFont.pixelSize * 1.35
                    font.weight: Font.DemiBold
                }
                Rectangle {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    radius: Kirigami.Units.cornerRadius * cornerScale.value / 125
                    color: page.alphaColor(Kirigami.Theme.backgroundColor, cardOpacity.value / 100)
                    border.width: 1
                    border.color: page.alphaColor(Kirigami.Theme.textColor, borderStrength.value / 100)

                    Label {
                        anchors.centerIn: parent
                        text: denseMode.checked ? "紧凑任务卡片" : "普通任务卡片"
                        opacity: 0.8
                    }
                }
            }
        }

        RowLayout {
            Kirigami.FormData.label: "风格预设："
            Layout.fillWidth: true

            Button {
                Layout.fillWidth: true
                text: "清透"
                icon.name: "weather-clear"
                onClicked: page.applyPreset(38, 58, 9, 20, 14, 125)
            }
            Button {
                Layout.fillWidth: true
                text: "平衡"
                icon.name: "adjustlevels"
                onClicked: page.applyPreset(58, 78, 13, 16, 22, 110)
            }
            Button {
                Layout.fillWidth: true
                text: "浓郁"
                icon.name: "weather-clouds"
                onClicked: page.applyPreset(82, 92, 19, 12, 28, 95)
            }
        }

        RowLayout {
            Kirigami.FormData.label: "玻璃不透明度："
            Slider {
                id: glassOpacity
                Layout.fillWidth: true
                from: 25
                to: 95
                stepSize: 1
            }
            Label {
                Layout.minimumWidth: Kirigami.Units.gridUnit * 2.5
                horizontalAlignment: Text.AlignRight
                text: Math.round(glassOpacity.value) + "%"
            }
        }

        RowLayout {
            Kirigami.FormData.label: "卡片透明度："
            Slider {
                id: cardOpacity
                Layout.fillWidth: true
                from: 35
                to: 100
                stepSize: 1
            }
            Label {
                Layout.minimumWidth: Kirigami.Units.gridUnit * 2.5
                horizontalAlignment: Text.AlignRight
                text: Math.round(cardOpacity.value) + "%"
            }
        }

        RowLayout {
            Kirigami.FormData.label: "强调色强度："
            Slider {
                id: accentStrength
                Layout.fillWidth: true
                from: 0
                to: 45
                stepSize: 1
            }
            Label {
                Layout.minimumWidth: Kirigami.Units.gridUnit * 2.5
                horizontalAlignment: Text.AlignRight
                text: Math.round(accentStrength.value) + "%"
            }
        }

        CheckBox {
            id: showDecorativeGlow
            Kirigami.FormData.label: "右上角装饰："
            text: "显示光晕"
        }

        RowLayout {
            Kirigami.FormData.label: "光晕大小："
            Layout.fillWidth: true
            enabled: showDecorativeGlow.checked
            Slider {
                id: decorativeGlowSize
                Layout.fillWidth: true
                from: 25
                to: 110
                stepSize: 1
            }
            Label {
                Layout.minimumWidth: Kirigami.Units.gridUnit * 2.5
                horizontalAlignment: Text.AlignRight
                text: Math.round(decorativeGlowSize.value) + "%"
            }
        }

        ComboBox {
            id: goalNodeShape
            Kirigami.FormData.label: "目标节点形状："
            Layout.fillWidth: true
            model: ["圆角卡片", "直角卡片", "胶囊卡片"]
        }

        RowLayout {
            Kirigami.FormData.label: "边框强度："
            Slider {
                id: borderStrength
                Layout.fillWidth: true
                from: 0
                to: 50
                stepSize: 1
            }
            Label {
                Layout.minimumWidth: Kirigami.Units.gridUnit * 2.5
                horizontalAlignment: Text.AlignRight
                text: Math.round(borderStrength.value) + "%"
            }
        }

        RowLayout {
            Kirigami.FormData.label: "阴影强度："
            Slider {
                id: shadowStrength
                Layout.fillWidth: true
                from: 0
                to: 50
                stepSize: 1
            }
            Label {
                Layout.minimumWidth: Kirigami.Units.gridUnit * 2.5
                horizontalAlignment: Text.AlignRight
                text: Math.round(shadowStrength.value) + "%"
            }
        }

        RowLayout {
            Kirigami.FormData.label: "圆角大小："
            Slider {
                id: cornerScale
                Layout.fillWidth: true
                from: 60
                to: 180
                stepSize: 5
            }
            Label {
                Layout.minimumWidth: Kirigami.Units.gridUnit * 2.5
                horizontalAlignment: Text.AlignRight
                text: Math.round(cornerScale.value) + "%"
            }
        }

        CheckBox {
            id: denseMode
            Kirigami.FormData.label: "布局："
            text: "使用紧凑间距"
        }

        CheckBox {
            id: animationsEnabled
            text: "启用悬停与颜色动画"
        }

        CheckBox {
            id: showBrand
            Kirigami.FormData.label: "内容："
            text: "显示 LITELIST 品牌标题"
        }

        CheckBox {
            id: showStatus
            text: "显示底部保存状态"
        }
    }
}
