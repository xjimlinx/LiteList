# LiteList

LiteList 是一个轻量、纯本地运行的 Windows 桌面待办和便签工具。它适合把近期要做的事情放在桌面边缘，随手添加、完成、编辑和排序；也可以创建可拖动的桌面便签，用来记录灵感、会议要点和购物清单。

## 先看这里：当前如何获得并运行

目前仓库还没有发布 GitHub Release，Releases 页面为空。当前最可靠的使用方式是从源码构建。LiteList 暂时也没有安装程序，构建出的 `.exe` 可以直接双击运行。

如果电脑已经安装 Rust 和 Visual Studio 的 MSVC 编译环境，完整流程如下：

```powershell
git clone https://github.com/Baozixu99/LiteList.git
cd LiteList
cargo build --release
.\target\release\litelist.exe
```

`cargo build --release` 成功后，真正需要双击的文件是：

```text
LiteList\target\release\litelist.exe
```

也可以在资源管理器中打开这个目录并双击 `litelist.exe`。程序启动后主要显示在 Windows 通知区域，第一次使用时请查看右下角托盘；右键托盘图标可以打开菜单、创建便签或退出程序。

如果只运行 `cargo build`，生成的是调试版：

```text
LiteList\target\debug\litelist.exe
```

调试版同样可以直接双击，但体积更大、运行优化较少。给普通使用者准备文件时建议使用 `--release`。

## 构建前需要什么

普通使用者不需要安装 Rust；开发者从源码构建时需要准备以下环境：

- Windows 10 或 Windows 11
- Rustup 和项目指定的 Rust 工具链
- `x86_64-pc-windows-msvc` 目标
- Visual Studio 2022 或 Build Tools 中的“使用 C++ 的桌面开发”组件
- Windows 10/11 SDK

安装 Rustup 后，在项目目录执行 `cargo build --release` 时，Rustup 会根据 `rust-toolchain.toml` 选择项目需要的工具链。第一次构建还需要从 crates.io 下载依赖，因此需要网络；以后可以使用 `--offline`，但前提是依赖已经缓存。

如果遇到 `link.exe`、MSVC 或 Windows SDK 相关错误，通常是 C++ 编译工具链没有安装完整。可以从 Visual Studio Installer 安装“使用 C++ 的桌面开发”，然后重新打开终端再执行构建。

LiteList 使用静态 CRT 链接，发布后的程序不要求使用者另外安装 Visual C++ 运行库。程序只支持 Windows，不能在 macOS 或 Linux 上直接运行。

## 功能

- 半透明悬浮待办面板，可展开、收起、移动和调整大小
- 回车快速添加待办，支持多行粘贴后批量创建
- 点击复选框完成或恢复任务
- 双击任务编辑，支持选中、复制和粘贴
- 拖动任务右侧手柄调整顺序
- 右键菜单提供编辑、复制、提醒、删除和撤销
- 桌面便签支持编辑标题和正文、自动保存、移动、缩放、置顶和删除
- 提醒默认关闭；启用后可设置到点提醒和可选的提前提醒
- 到点或提前提醒通过 Windows 弹窗通知
- 数据、备份和通知均保存在本机，不需要登录或联网服务
- 支持开机启动、浅色主题、背景透明度和 JSON 导入导出

## 快速使用

主面板底部是输入区域。输入一条任务后按回车即可添加；连续按回车可以继续添加。任务区域中的复选框用于完成或恢复任务。

双击任务可以编辑内容。编辑时可以像普通文本框一样拖选、复制和粘贴；右键任务也可以选择“编辑 / 选中文字”和“复制整条任务”。拖动任务右侧的点状手柄可以调整顺序。

拖动主面板顶部可以移动窗口，拖动右下角可以调整大小。点击右上角箭头可以展开或收起面板。快捷键 `Ctrl + Alt + Space` 可以唤出 LiteList；如果快捷键被其他程序占用，可以从托盘菜单使用 LiteList。

## 使用提醒

选中一个任务后右键选择“提醒设置…”。提醒默认未启用；不设置提醒时，任务列表不会显示提醒状态。

启用后：

1. 在“到点提醒”中选择日期和时间，这是正式提醒时间。
2. 如需提前准备，在“提前提醒”中选择日期和时间；该项可以留空。
3. 可以填写提前提醒备注，例如“提前打印简历”。
4. 点击“保存”。提前提醒时间必须早于到点时间。

例如，正式时间设置为 9 月 9 日 19:00，提前时间设置为 9 月 9 日 14:00，LiteList 会在两个时间分别弹窗提醒。程序需要保持运行；Windows 睡眠或程序退出期间无法实时弹窗，程序恢复运行后会补发已经错过的到点提醒。

## 桌面便签

在托盘菜单选择“新建桌面便签”。便签标题和正文会自动保存，窗口可以独立拖动、缩放，并通过“置顶”保持在其他窗口上方。关闭便签只会暂时隐藏它；再次从托盘菜单选择“显示全部便签”即可恢复。

点击便签右上角“删除”后会出现确认提示。确认删除会从本地数据中移除该便签，任务的删除则可以通过主菜单“恢复最近删除的任务”撤销。

## 数据和备份

默认数据目录为：

```text
%LOCALAPPDATA%\\LiteList
```

其中 `state.json` 是当前数据，`state.backup.json` 是最近一次成功保存前的备份。程序使用临时文件和原子替换保存，发现主文件损坏时会尝试从备份恢复，并保留损坏文件。

托盘菜单中的“打开数据文件夹”可以直接打开目录；“导出备份到文件夹”会生成带时间戳的 JSON 备份；“导入 JSON 备份…”可以恢复一份合法的 LiteList 数据。

如果希望使用便携模式：

1. 先执行 `cargo build --release`。
2. 在 `target\\release` 目录中创建一个空文件，文件名必须是 `portable.flag`。
3. 双击同目录下的 `litelist.exe`。

此时数据会保存到 `target\\release\\data`。如果把 exe 复制到其他目录，记得把 `portable.flag` 一起复制过去。

## 从源码开发

项目使用 Rust 和 Windows 原生 API 实现，当前目标平台是 `x86_64-pc-windows-msvc`。

常用命令：

```powershell
# Debug 构建
cargo build

# Release 构建
cargo build --release

# 运行程序
cargo run --release

# 运行测试
cargo test

# 检查格式
cargo fmt --check

# 运行 Clippy
cargo clippy --all-targets --all-features -- -D warnings
```

依赖下载完成后，也可以使用：

```powershell
cargo test --offline
cargo build --release --offline
```

构建产物位于 `target\\debug` 或 `target\\release`。仓库中的 `dist` 和 `target` 是本地构建目录，已加入 `.gitignore`，不会随源码提交。要制作一个简单的本地分发目录，可以复制 `target\\release\\litelist.exe`，不需要额外的 DLL；如果启用便携模式，则同时复制 `portable.flag`。

## 代码结构

```text
src/
├─ main.rs       程序入口和错误显示
├─ ui.rs         主窗口、托盘菜单、快捷键和事件循环
├─ render.rs     半透明待办面板的绘制与布局
├─ panels.rs     便签、提醒设置和提醒弹窗
├─ model.rs      待办、便签、提醒和撤销模型
├─ reminders.rs  本地时间转换和提醒调度
└─ storage.rs    JSON 存储、备份、恢复和原子写入
```

界面回调只负责收集 Windows 消息，数据模型在主事件循环中修改。新增功能时优先把业务规则放在 `model.rs` 或 `reminders.rs`，把窗口消息和控件布局留在 `ui.rs`、`panels.rs`，并为可测试的规则补充单元测试。

## 修改后的检查清单

提交前建议运行：

```powershell
cargo fmt --check
cargo test --offline
cargo build --release --offline
```

如果修改了窗口行为，还应在 Windows 桌面上手动检查：主面板展开和收起、任务编辑复制、便签移动缩放删除、提醒开关以及提前提醒时间校验。

## 许可

项目使用 MIT 许可证，具体声明见 `Cargo.toml`。Windows 系统组件和 Rust 依赖分别遵循其自身许可证。
