<div align="center">

# DocAgent

**AI 驱动的文档处理 Agent，用对话搞定一切文档工作**

[![Windows](https://img.shields.io/badge/platform-Windows-blue?logo=windows)](https://github.com/user-attachments/docagent)
[![Version](https://img.shields.io/badge/version-0.2.0-4ccd24)](https://github.com/XuMingKe-06/DocAgent/releases)
[![Tauri 2](https://img.shields.io/badge/Tauri-2.x-orange?logo=tauri)](https://v2.tauri.app/)
[![React 19](https://img.shields.io/badge/React-19-61dafb?logo=react)](https://react.dev/)
[![Rust](https://img.shields.io/badge/Rust-1.80+-000000?logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-green)](./LICENSE)

[中文](./README_zh.md) | [English](./README.md)

</div>

---

> 本项目目前处于早期开发阶段，可能存在一些不足之处，敬请谅解。我们诚挚欢迎志同道合的开发者加入，共同完善这个项目！

> [点击此处下载 Windows 发行版本](https://github.com/XuMingKe-06/DocAgent/releases/latest)

## DocAgent 是什么？

DocAgent 是一款**本地优先**的 AI 文档处理桌面应用。你只需用自然语言描述需求，AI Agent 就会自动完成文档的生成、读取、修改、格式转换等操作。

不再在 Word、Excel、PPT、PDF 之间来回切换工具 -- 一个对话窗口，覆盖所有文档格式。

---

## 为什么选择 DocAgent？

### 本地优先，数据安全

所有文档处理和文件操作都在你的机器上完成，只有 LLM API 调用需要联网。你的文档内容不会上传到任何第三方服务器。

### 多 LLM Provider 支持

灵活接入 OpenAI、Anthropic Claude、Google Gemini、Ollama 等主流 LLM 服务，自动健康检查与故障切换，你不会被单一供应商锁定。

### 代码解释器 —— 文档生成全新范式

v0.2.0 核心亮点：AI 编写并执行 Python 代码，直接操作文档对象模型。从"描述式生成"升级为"代码级控制"——生成结果更精准，复杂格式不再是难题。

- **读取-修改-保存工作流**：先读取现有文档结构，再编写代码精准修改，最后保存——全程在本地沙箱中安全执行
- **全格式覆盖**：Word、Excel、PPT、PDF、Markdown，一个引擎全搞定
- **复杂操作不再难**：表格合并、条件格式、图表嵌入，通过代码执行一次性完成

### 常驻文档处理器

5 个内置文档类型处理器始终启用，无需开关管理：

| 处理器 | 能力 |
|------|------|
| **Word (docx)** | 生成、读取、修改、转换、分析 .docx 文档 |
| **Excel (xlsx)** | 生成、读取、修改、转换、分析 .xlsx 表格 |
| **PowerPoint (pptx)** | 生成、读取、修改、转换、分析 .pptx 演示文稿 |
| **PDF** | 生成、读取、转换、分析 PDF 文档 |
| **代码解释器** | 执行 Python 代码，实现复杂文档的生成与修改 |

### 安全可控的操作确认

高风险操作（删除、修改、批量处理）需用户确认后才执行，支持三级确认策略：始终确认 / 仅编辑确认 / 从不确认。

### 版本快照与回滚

每次文档修改自动创建版本快照，一键回滚到任意历史版本，再也不怕改错。

### 实时文件监听

工作区文件变更实时同步到界面，配合内置文件树浏览，文档状态一目了然。

### 智能 LLM 缓存

多 Provider 缓存支持，智能缓存重复的上下文请求，显著降低 API 调用成本并提升响应速度。界面直观显示缓存命中率，使用情况一目了然。

### 可靠的工具执行

LLM 响应在工具调用期间被截断时自动重试，确保操作指令完整性。网络波动或 API 限制不会悄悄导致文档操作失败。

### 代码预览流式输出

AI 生成的代码实时逐行展示——看着文档逻辑每一步"生长"，无需等待整个过程完成后才能查看。

---

## 技术亮点

- **Tauri 2.x** -- Rust 后端 + Web 前端，安装包小、启动快、内存占用低
- **代码解释器** -- AI 编写并执行 Python 代码，在本地沙箱中精准生成和修改文档
- **Rust Agent 引擎** -- 异步 Tool Calling 循环，流式输出，增量持久化防崩溃丢失
- **Python Sidecar** -- 专业文档处理（python-docx / openpyxl / python-pptx / PyMuPDF / reportlab），进程级隔离，崩溃自动重启
- **多 LLM 路由** -- Provider 健康检查、延迟追踪、自动 Fallback，智能缓存命中率显示
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

### 处理器管理

5 个内置文档处理器始终启用，即开即用。设置页的处理器标签页可查看各处理器的能力与状态。

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
