# claude-history

[![Rust](https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white)](#)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

[**English**](README.md)

一个极简、快速的终端 TUI 工具，用于浏览和恢复 [Claude Code](https://docs.anthropic.com/en/docs/claude-code) 对话。

运行 `claude-history` → 模糊搜索**所有项目**的对话 → 按 Enter 直接恢复对话。

## 特性

- **全局历史** — 搜索所有项目的 Claude Code 对话记录
- **即时模糊搜索** — 大小写不敏感、前缀匹配、多关键词 AND 逻辑
- **一键恢复** — 按 Enter 直接跳回 Claude Code 继续对话
- **高性能** — rayon 并行加载、流式返回结果、预计算搜索索引
- **跨平台** — 支持 Windows、macOS 和 Linux

## 安装

```sh
cargo install --path .
```

或者从仓库安装：

```sh
cargo install --git https://github.com/chuilishi/claude-history
```

## 使用

```sh
claude-history
```

就这么简单。不需要任何参数。TUI 界面会列出所有对话（按时间倒序），输入关键词搜索：

| 按键 | 功能 |
|---|---|
| `↑` / `↓` | 移动选择 |
| `Page Up` / `Page Down` | 整页跳转 |
| `Home` / `End` | 跳到第一个 / 最后一个 |
| `Enter` | 恢复选中的对话 |
| `Ctrl+W` | 删除光标前的单词 |
| `Esc` / `Ctrl+C` | 退出 |

### 搜索

- **大小写不敏感**：`config` 匹配 `CONFIG`
- **下划线视为分隔符**：`api key` 匹配 `API_KEY`
- **前缀匹配**：`auth` 匹配 `authentication`
- **多关键词**：所有关键词必须同时匹配（AND 逻辑）

## 致谢

基于 [raine/claude-history](https://github.com/raine/claude-history) 精简而来，仅保留对话恢复功能。

## 许可证

MIT
