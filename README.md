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
- “目标路线”可同时展示多个可折叠的大目标，并支持多前置条件和科技树式分支连线
- 到点提醒和可选的提前提醒，使用 KDE 原生通知显示
- JSON 导入/导出，兼容原 Windows 版的版本 1/2 数据结构
- 所有内容保存到当前 Plasma 小部件实例的 KConfig 配置，不需要登录或联网
- 标准 Plasma 外观设置页，可调整玻璃和卡片不透明度、光晕、边框、阴影、圆角与目标节点形状

## 使用说明

在待办输入框中输入内容并按回车；任务右侧菜单提供编辑、提醒、排序和删除。右上角按钮可切换待办、便签、提醒记录和更多操作。

右键小部件选择“配置 LiteList”，或从右上角菜单进入“外观设置”，可以使用“清透 / 平衡 / 浓郁”预设，也可以逐项调整：

- 玻璃与任务卡片不透明度
- 强调色光晕的开关、大小与强度，以及边框和阴影强度
- 圆角大小与紧凑布局
- 目标节点的圆角、直角或胶囊形状
- 悬停动画、品牌标题和底部保存状态

设置修改后会由 Plasma 自动保存，并立即反映在小部件上。

### 大目标与目标路线

顶部“目标”分页进入“目标路线”。页面会纵向展示全部大目标，每个目标可以独立展开或折叠；大目标用于描述最终成果，小目标是路线中的节点：

1. 新建一个大目标，例如“发布 LiteList 1.0”。
2. 添加没有前置条件的起始节点。
3. 添加后续小目标时，可以勾选一个或多个已有节点作为前置条件。
4. 前置节点全部完成后，后续节点才会解锁。

路线会按依赖层级自动排列并绘制分支连线。节点层数增多时，路线会按实际高度展开并由目标页面统一滚动；横向分支超过可见宽度时，可拖动科技树空白处或使用底部滚动条浏览。一个节点被其他节点依赖时不能直接删除；已有完成节点依赖某项时，也不能贸然撤回该前置项。普通待办仍保持扁平、快速的操作方式，不会被强制放入目标树。

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

任务、完成状态、显式设置的提醒、便签和通知记录会被读取。导入旧数据时会自动补充空的目标路线字段。Windows 窗口坐标不会在 Plasma 中使用，因为桌面小部件的位置由 Plasma 自己管理。

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
    ├── config/
    │   ├── config.qml      Plasma 配置页注册
    │   └── main.xml        每实例 KConfig 存储定义
    └── ui/
        ├── configAppearance.qml  玻璃外观设置
        ├── GoalTree.qml          目标依赖布局、连线和节点卡片
        └── main.qml              Plasma 6 主界面与 KDE 通知集成
tests/store.test.js               数据模型测试
tests/qml/GoalTreeHarness.qml     目标树渲染测试
```

## 许可

项目使用 MIT 许可证。KDE、Qt 与 Rust 依赖遵循各自许可证。
