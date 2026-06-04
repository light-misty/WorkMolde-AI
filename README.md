<div align="center">

# DocAgent

**AI 驱动的文档处理 Agent，用对话搞定一切文档工作**

[![Windows](https://img.shields.io/badge/platform-Windows-blue?logo=windows)](https://github.com/user-attachments/docagent)
[![Tauri 2](https://img.shields.io/badge/Tauri-2.x-orange?logo=tauri)](https://v2.tauri.app/)
[![React 19](https://img.shields.io/badge/React-19-61dafb?logo=react)](https://react.dev/)
[![Rust](https://img.shields.io/badge/Rust-1.80+-000000?logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-green)](./LICENSE)

[中文](./README.md) | [English](./README_en.md)

</div>

---

## DocAgent 是什么？

DocAgent 是一款**本地优先**的 AI 文档处理桌面应用。你只需用自然语言描述需求，AI Agent 就会自动完成文档的生成、读取、修改、格式转换等操作。

不再在 Word、Excel、PPT、PDF 之间来回切换工具 -- 一个对话窗口，覆盖所有文档格式。

---

## 为什么选择 DocAgent？

### 本地优先，数据安全

所有文档处理和文件操作都在你的机器上完成，只有 LLM API 调用需要联网。你的文档内容不会上传到任何第三方服务器。

### 多 LLM Provider 支持

灵活接入 OpenAI、Anthropic Claude、Google Gemini、Ollama 等主流 LLM 服务，自动健康检查与故障切换，你不会被单一供应商锁定。

### 专业文档处理引擎

内置 6 大文档处理技能，覆盖从生成到分析的完整工作流：

| 技能 | 说明 |
|------|------|
| **generate_document** | 生成 Word / Excel / PPT / PDF / Markdown，支持公式、条件格式、颜色方案、水印等高级特性 |
| **read_document** | 读取文档结构与内容，支持格式信息提取 |
| **modify_document** | 30+ 修改操作：段落、表格、书签、超链接、页眉页脚、目录等 |
| **convert_format** | docx / pdf / md / txt / csv / html 等格式互转 |
| **analyze_document** | 文档结构分析与统计信息 |
| **batch_process** | 批量转换、修改、分析 |

### 安全可控的操作确认

高风险操作（删除、修改、批量处理）需用户确认后才执行，支持三级确认策略：始终确认 / 仅编辑确认 / 从不确认。

### 版本快照与回滚

每次文档修改自动创建版本快照，一键回滚到任意历史版本，再也不怕改错。

### 实时文件监听

工作区文件变更实时同步到界面，配合内置文件树浏览，文档状态一目了然。

---

## 技术亮点

- **Tauri 2.x** -- Rust 后端 + Web 前端，安装包小、启动快、内存占用低
- **Rust Agent 引擎** -- 异步 Tool Calling 循环，流式输出，增量持久化防崩溃丢失
- **Python Sidecar** -- 专业文档处理（python-docx / openpyxl / python-pptx / PyMuPDF / reportlab），进程级隔离，崩溃自动重启
- **多 LLM 路由** -- Provider 健康检查、延迟追踪、自动 Fallback
- **React 19 + Zustand 5** -- 现代前端架构，虚拟滚动优化长列表性能
- **PDF Canvas 渲染** -- 基于 pdfjs-dist 的高性能 PDF 预览，支持缩放与翻页
- **统一错误码体系** -- 按模块分段（LLM / Agent / Doc / DB / Config / FS / Runtime），精确定位问题

---

## 界面预览

### 主界面

![DocAgent 主界面](./assets/screenshots/main-interface.png)

### 文档生成效果

![生成的Word文档](./assets/screenshots/document-preview.png)

---

## 使用示例

**生成一份项目周报：**

> 帮我生成一份项目周报 Word 文档，包含本周完成事项、下周计划和风险提示三个部分

**读取并分析 Excel：**

> 读取 data/sales.xlsx，分析各区域的销售数据，生成一份统计摘要

**批量格式转换：**

> 把 workspace/docs 目录下所有 Markdown 文件转换为 PDF

**修改现有文档：**

> 在 report.docx 的第三段后面插入一个三行四列的表格，表头是"姓名、部门、职位、入职日期"

---

## 配置与自定义

### LLM Provider 配置

支持 OpenAI、Anthropic、Gemini、Ollama 及任何兼容 OpenAI API 的服务。在设置页添加你的 API Key 和模型即可开始使用。

### 技能管理

6 个内置文档处理技能可按需启用/禁用，在设置页的技能标签页管理。

### Prompt 模板

内置模板管理系统，保存常用 Prompt，一键复用。

### 快捷键

可自定义快捷键：新建会话、关闭会话、发送消息、切换侧栏、快速 Prompt 等。

---

## 贡献

欢迎贡献代码、报告问题或提出建议！

1. Fork 本仓库
2. 创建功能分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'feat: 添加某个很棒的功能'`)
4. 推送分支 (`git push origin feature/amazing-feature`)
5. 创建 Pull Request

---

## 技术栈一览

| 类别 | 技术 |
|------|------|
| 桌面框架 | Tauri 2.x |
| 前端 | React 19 + TypeScript 5 + Vite 6 |
| UI | Shadcn/ui + Radix + Tailwind CSS 4 |
| 状态管理 | Zustand 5 |
| 后端 | Rust (Tokio 异步运行时) |
| 数据库 | SQLite (rusqlite, bundled) |
| 文档处理 | Python Sidecar (python-docx / openpyxl / python-pptx / PyMuPDF / reportlab) |
| PDF 预览 | pdfjs-dist |
| 图表 | Recharts |
| 自动更新 | tauri-plugin-updater |

---

## 许可证

本项目基于 [MIT 许可证](./LICENSE) 开源。
