# LiteList for KDE Plasma 6

LiteList（轻单）现在可以作为 KDE Plasma 6 原生小部件使用。它直接放在桌面或面板上，由 Plasma 管理位置、大小和启动，不再需要 Windows 托盘、Win32 窗口或独立后台进程。

## 安装

需要 KDE Plasma 6.0 或更高版本。仓库中的小部件本身是 QML，不需要编译 Rust。

```bash
git clone https://github.com/Baozixu99/LiteList.git
cd LiteList
kpackagetool6 --type Plasma/Applet --install package
```

然后右键桌面或面板，选择“进入编辑模式”→“添加小部件”，搜索 `LiteList` 或“轻单”，拖到桌面或面板即可。

更新已经安装的小部件：

```bash
kpackagetool6 --type Plasma/Applet --upgrade package
```

卸载：

```bash
kpackagetool6 --type Plasma/Applet --remove io.github.baozixu99.litelist
```

若升级后 Plasma 没有立即刷新界面，可移除后重新添加该小部件；通常不需要重启桌面。

## 功能

- 在桌面上完整显示，放入面板时自动使用紧凑图标和待办数量角标
- 原生 Plasma 磨砂玻璃背景、主题自适应配色、圆角卡片和悬停高光
- 回车快速添加待办，粘贴多行内容可一次创建多条
- 完成、恢复、编辑、删除、撤销，以及上移/下移排序
- 独立的已完成列表和最近删除恢复
- 小部件内便签，可创建、编辑和删除
- 到点提醒和可选的提前提醒，使用 KDE 原生通知显示
- JSON 导入/导出，兼容原 Windows 版的版本 1/2 数据结构
- 所有内容保存到当前 Plasma 小部件实例的 KConfig 配置，不需要登录或联网
- 标准 Plasma 外观设置页，可调整玻璃和卡片不透明度、光晕、边框、阴影与圆角

## 使用说明

在待办输入框中输入内容并按回车；任务右侧菜单提供编辑、提醒、排序和删除。右上角按钮可切换待办、便签、提醒记录和更多操作。

右键小部件选择“配置 LiteList”，或从右上角菜单进入“外观设置”，可以使用“清透 / 平衡 / 浓郁”预设，也可以逐项调整：

- 玻璃与任务卡片不透明度
- 强调色光晕、边框和阴影强度
- 圆角大小与紧凑布局
- 悬停动画、品牌标题和底部保存状态

设置修改后会由 Plasma 自动保存，并立即反映在小部件上。

提醒时间使用本地时间，格式为：

```text
2026-09-09 19:00
```

提醒由 `plasmashell` 内的定时器检查，因此用户需要保持 Plasma 会话运行。休眠或 Plasma 重启后，已经错过的到点提醒会在小部件恢复时补发；如果正式时间已到，只发送到点提醒，不重复发送过期的提前提醒。

每个小部件实例保存一份独立清单。Plasma 将配置写入自己的 KConfig 文件，通常位于：

```text
~/.config/plasma-org.kde.plasma.desktop-appletsrc
```

不建议直接编辑该文件。请用小部件右上角菜单中的“JSON 导入 / 导出”迁移数据。导入成功时，LiteList 会在该实例的 `backupJson` 配置项中保留导入前数据，可在同一窗口恢复。

### 从原 Windows 版迁移

1. 在 Windows 版中导出 JSON，或复制 `%LOCALAPPDATA%\LiteList\state.json`。
2. 在 Plasma 小部件中打开“更多”→“JSON 导入 / 导出”。
3. 用原 JSON 替换文本框内容，点击“从上方 JSON 导入”。

任务、完成状态、显式设置的提醒、便签和通知记录会被读取。Windows 窗口坐标不会在 Plasma 中使用，因为桌面小部件的位置由 Plasma 自己管理。

## 与原 Windows 版的差异

Plasma 小部件不需要托盘图标、全局快捷键或“开机启动”：登录 KDE 后，Plasma 会自动恢复桌面上的小部件。便签显示在 LiteList 小部件的便签页中，而不是创建绕过 Plasma 管理的独立顶层窗口。

磨砂模糊由 KDE 的桌面合成器提供。如果背景只有透明度而没有模糊，请在“系统设置 → 窗口管理 → 桌面特效”中启用“模糊”；禁用合成器时，小部件会使用较实的半透明底色保证文字清晰。

原 Win32/Rust 实现仍保留在 `src/`，用于读取业务规则和数据格式，也方便继续维护 Windows 版本；Linux/KDE 的实际入口是 `package/contents/ui/main.qml`。

## 开发与检查

静态检查：

```bash
qmllint package/contents/code/Store.js package/contents/ui/main.qml
```

运行数据模型测试（需要 Node.js）：

```bash
node tests/store.test.js
```

安装后，开发者可用 KDE 的 `plasmawindowed` 测试宿主预览。它只用于调试，正常使用时小部件由 `plasmashell` 直接加载，不会启动独立 LiteList 窗口或后台程序：

```bash
plasmawindowed -p org.kde.plasma.desktop io.github.baozixu99.litelist
```

小部件包结构：

```text
package/
├── metadata.json
└── contents/
    ├── code/Store.js       数据校验、迁移、时间和提醒规则
    ├── config/main.xml     每实例 KConfig 存储定义
    └── ui/main.qml         Plasma 6 界面与 KDE 通知集成
tests/store.test.js         数据模型测试
```

## 许可

项目使用 MIT 许可证。KDE、Qt 与 Rust 依赖遵循各自许可证。
