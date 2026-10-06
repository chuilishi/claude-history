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

```sh
cargo install --git https://github.com/chuilishi/claude-history
```

## 使用

```sh
claude-history
```

## 致谢

基于 [raine/claude-history](https://github.com/raine/claude-history) 精简而来，仅保留对话恢复功能。

## 许可证

MIT
