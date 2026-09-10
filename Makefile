SHELL := /bin/bash

PLUGIN_ID := io.github.baozixu99.litelist
PACKAGE_TYPE := Plasma/Applet
PACKAGE_DIR := $(CURDIR)/package

KPACKAGETOOL ?= kpackagetool6
QDBUS ?= qdbus6
PLASMAWINDOWED ?= plasmawindowed
QMLLINT ?= qmllint
NODE ?= node
CARGO ?= cargo

.DEFAULT_GOAL := help

.PHONY: help install update upgrade deploy reload restart uninstall preview test test-qml test-js test-rust

help:
	@printf '%s\n' \
		'LiteList Plasma 开发命令：' \
		'  make install   首次安装；已安装时自动升级' \
		'  make update    更新已安装的插件（install 的别名）' \
		'  make reload    请求 Plasma 轻量刷新，不重启桌面外壳' \
		'  make restart   重启 plasmashell，确保重新加载 QML' \
		'  make deploy    安装/更新并重启 plasmashell' \
		'  make preview   更新后用 plasmawindowed 调试预览' \
		'  make test      运行 QML、JavaScript 与 Rust 检查' \
		'  make uninstall 卸载 Plasmoid（不会主动删除 KDE 配置）'

install:
	@command -v "$(KPACKAGETOOL)" >/dev/null || { echo '缺少 kpackagetool6，请先安装 KDE Plasma 6 开发工具。'; exit 1; }
	@if "$(KPACKAGETOOL)" --type "$(PACKAGE_TYPE)" --show "$(PLUGIN_ID)" >/dev/null 2>&1; then \
		echo 'LiteList 已安装，正在升级插件文件……'; \
		"$(KPACKAGETOOL)" --type "$(PACKAGE_TYPE)" --upgrade "$(PACKAGE_DIR)"; \
	else \
		echo '正在首次安装 LiteList……'; \
		"$(KPACKAGETOOL)" --type "$(PACKAGE_TYPE)" --install "$(PACKAGE_DIR)"; \
	fi

update upgrade: install

reload:
	@command -v "$(QDBUS)" >/dev/null || { echo '缺少 qdbus6，无法请求 Plasma 刷新。'; exit 1; }
	@"$(QDBUS)" org.kde.plasmashell /PlasmaShell org.kde.PlasmaShell.refreshCurrentShell
	@echo '已请求 Plasma 刷新；若界面仍是旧版本，请运行 make restart。'

restart:
	@command -v systemctl >/dev/null || { echo '缺少 systemctl，无法重启 Plasma 6 用户服务。'; exit 1; }
	@echo '正在重启 plasmashell；桌面和面板会短暂消失后自动恢复……'
	@systemctl --user restart plasma-plasmashell.service

deploy:
	@$(MAKE) --no-print-directory install
	@$(MAKE) --no-print-directory restart

uninstall:
	@command -v "$(KPACKAGETOOL)" >/dev/null || { echo '缺少 kpackagetool6。'; exit 1; }
	@if "$(KPACKAGETOOL)" --type "$(PACKAGE_TYPE)" --show "$(PLUGIN_ID)" >/dev/null 2>&1; then \
		"$(KPACKAGETOOL)" --type "$(PACKAGE_TYPE)" --remove "$(PLUGIN_ID)"; \
	else \
		echo 'LiteList 当前未安装。'; \
	fi

preview:
	@$(MAKE) --no-print-directory install
	@command -v "$(PLASMAWINDOWED)" >/dev/null || { echo '缺少 plasmawindowed。'; exit 1; }
	@"$(PLASMAWINDOWED)" -p org.kde.plasma.desktop "$(PLUGIN_ID)"

test: test-qml test-js test-rust

test-qml:
	@command -v "$(QMLLINT)" >/dev/null || { echo '缺少 qmllint。'; exit 1; }
	@"$(QMLLINT)" \
		package/contents/code/Store.js \
		package/contents/ui/main.qml \
		package/contents/ui/GoalTree.qml \
		package/contents/ui/configAppearance.qml

test-js:
	@command -v "$(NODE)" >/dev/null || { echo '缺少 Node.js。'; exit 1; }
	@"$(NODE)" tests/store.test.js

test-rust:
	@command -v "$(CARGO)" >/dev/null || { echo '缺少 Cargo/Rust。'; exit 1; }
	@"$(CARGO)" test
