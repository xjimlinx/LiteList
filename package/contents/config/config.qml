import QtQuick

import org.kde.plasma.configuration

ConfigModel {
    ConfigCategory {
        name: i18n("外观")
        icon: "preferences-desktop-color"
        source: "configAppearance.qml"
    }
}
