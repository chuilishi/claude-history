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
- **鼠标操作** — 单击选中、双击恢复、滚轮上下移动
- **隐藏对话** — 连按两次 Ctrl+D 将对话从列表中隐藏（不会改动 Claude Code 的文件；从 `~/.claude-history/hidden` 中删除对应 ID 即可恢复）
- **高性能** — rayon 并行加载、流式返回结果、预计算搜索索引
- **跨平台** — 支持 Windows、macOS 和 Linux

## 安装

macOS / Linux：

```sh
curl -fsSL https://raw.githubusercontent.com/chuilishi/claude-history/main/scripts/install.sh | bash
```

Windows（PowerShell）：

```powershell
irm https://raw.githubusercontent.com/chuilishi/claude-history/main/scripts/install.ps1 | iex
```

两种方式都会从 [Releases 页面](https://github.com/chuilishi/claude-history/releases)下载对应平台的预编译二进制，并校验 checksum。再运行一次同样的命令就能升级。设置 `CLAUDE_HISTORY_VERSION`（如 `v0.1.30`）可以锁定版本，`CLAUDE_HISTORY_INSTALL_DIR` 可以指定安装目录。

装了 Rust 的话也可以从源码编译：

```sh
cargo install claude-history-resume
```

crate 名为 `claude-history-resume`（`claude-history` 已被上游项目占用），安装后的命令仍是 `claude-history`。

## 使用

```sh
claude-history
```

## 让 Claude 帮你找对话

`claude-history search <关键词>` 和 `claude-history show <id>` 以纯文本输出结果。`find-conversation` skill 会让 Claude 用这两个命令检索，而不是直接读原始日志。这样在任何 Claude Code 会话里都可以问「之前修代理那个对话是哪次？」。安装：

```sh
mkdir -p ~/.claude/skills/find-conversation
cp skill/find-conversation/SKILL.md ~/.claude/skills/find-conversation/
```

## 致谢

基于 [raine/claude-history](https://github.com/raine/claude-history) 精简而来，仅保留对话恢复功能。

## 许可证

MIT
