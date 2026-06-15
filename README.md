<div align="center">

# DocAgent

**AI-Powered Document Agent - Handle All Document Tasks Through Conversation**

[![Windows](https://img.shields.io/badge/platform-Windows-blue?logo=windows)](https://github.com/user-attachments/docagent)
[![Version](https://img.shields.io/badge/version-0.2.0-4ccd24)](https://github.com/XuMingKe-06/DocAgent/releases)
[![Tauri 2](https://img.shields.io/badge/Tauri-2.x-orange?logo=tauri)](https://v2.tauri.app/)
[![React 19](https://img.shields.io/badge/React-19-61dafb?logo=react)](https://react.dev/)
[![Rust](https://img.shields.io/badge/Rust-1.80+-000000?logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-green)](./LICENSE)

[English](./README.md) | [中文](./README_zh.md)

</div>

---

> This project is currently in early development stage and may have some imperfections. We sincerely welcome like-minded developers to join us in improving this project!

> [Click here to download release for Windows](https://github.com/XuMingKe-06/DocAgent/releases/latest)

## What is DocAgent?

DocAgent is a **local-first** AI document processing desktop application. Simply describe your needs in natural language, and the AI Agent will automatically handle document generation, reading, modification, format conversion, and more.

No more switching between Word, Excel, PPT, and PDF tools -- one conversation window covers all document formats.

---

## Why Choose DocAgent?

### Local-First, Data Security

All document processing and file operations are completed on your machine. Only LLM API calls require internet access. Your document content is never uploaded to any third-party servers.

### Multi-LLM Provider Support

Flexible integration with mainstream LLM services like OpenAI, Anthropic Claude, Google Gemini, and Ollama. Automatic health checks and failover ensure you won't be locked into a single provider.

### Code Interpreter -- Document Generation Reimagined

The core highlight of v0.2.0: AI writes and executes Python code to directly manipulate document object models. From "descriptive generation" to "code-level control" -- results are more precise, complex formatting is no longer a struggle.

- **Read-modify-save workflow**: Read existing document structure, write code for precise modifications, save -- all executed in a local sandbox
- **Full format coverage**: Word, Excel, PPT, PDF, Markdown -- one engine handles them all
- **Complex operations made simple**: Table merges, conditional formatting, chart embedding -- achieve them with code execution in one shot

### Always-On Document Handlers

5 built-in document type handlers, always available with no enable/disable hassle:

| Handler | Capability |
|------|------|
| **Word (docx)** | Generate, read, modify, convert, analyze .docx documents |
| **Excel (xlsx)** | Generate, read, modify, convert, analyze .xlsx spreadsheets |
| **PowerPoint (pptx)** | Generate, read, modify, convert, analyze .pptx presentations |
| **PDF** | Generate, read, convert, analyze PDF documents |
| **Code Interpreter** | Execute Python code for complex document generation and modification |

### Secure and Controllable Operation Confirmation

High-risk operations (delete, modify, batch processing) require user confirmation before execution. Supports three-level confirmation policy: Always confirm / Confirm edits only / Never confirm.

### Version Snapshots and Rollback

Automatic version snapshots are created for every document modification. One-click rollback to any historical version - no more worries about making mistakes.

### Real-time File Monitoring

Workspace file changes are synchronized to the interface in real-time, combined with built-in file tree browsing for clear document status visibility.

### Smart LLM Caching

Multi-provider cache support intelligently caches repeated context requests, significantly reducing API call costs and improving response speed. Cache hit rate is displayed in the interface for full transparency.

### Reliable Tool Execution

Automatic retry mechanism for truncated LLM responses during tool calls ensures operation integrity. Network fluctuations or API limitations won't silently break your document operations.

### Code Preview Streaming

AI-generated code is displayed in real-time as it's written -- watch your document logic unfold line by line instead of waiting for the entire process to complete.

---

## Technical Highlights

- **Tauri 2.x** -- Rust backend + Web frontend, small installation package, fast startup, low memory footprint
- **Code Interpreter** -- AI writes and executes Python code to precisely generate and modify documents in a local sandbox
- **Rust Agent Engine** -- Asynchronous Tool Calling loop, streaming output, incremental persistence to prevent crash-induced data loss
- **Python Sidecar** -- Professional document processing (python-docx / openpyxl / python-pptx / PyMuPDF / reportlab), process-level isolation, automatic restart on crash
- **Multi-LLM Routing** -- Provider health checks, latency tracking, automatic fallback, smart caching with hit rate display
- **React 19 + Zustand 5** -- Modern frontend architecture, virtual scrolling for optimized long list performance
- **PDF Canvas Rendering** -- High-performance PDF preview based on pdfjs-dist, supporting zoom and page navigation
- **Unified Error Code System** -- Segmented by module (LLM / Agent / Doc / DB / Config / FS / Runtime) for precise problem localization

---

## Interface Preview

### Main Interface

![DocAgent Main Interface](./assets/screenshots/main-interface.png)

### Document Generation Result

![Generated Word Document](./assets/screenshots/document-preview.png)

---

## Usage Examples

**Generate a project weekly report:**

> Help me generate a project weekly report Word document with three sections: completed items this week, next week's plan, and risk alerts

**Read and analyze Excel:**

> Read data/sales.xlsx, analyze sales data by region, and generate a statistical summary

**Batch format conversion:**

> Convert all Markdown files in the workspace/docs directory to PDF

**Modify existing document:**

> Insert a 3-row, 4-column table after the third paragraph in report.docx, with headers "Name, Department, Position, Hire Date"

---

## Configuration and Customization

### LLM Provider Configuration

Supports OpenAI, Anthropic, Gemini, Ollama, and any OpenAI API-compatible service. Simply add your API Key and model in the settings page to get started.

### Handler Management

5 built-in document handlers are always enabled and ready to use. The Handlers tab in settings displays their capabilities and status for reference.

### Prompt Templates

Built-in template management system to save common Prompts for one-click reuse.

### Keyboard Shortcuts

Customizable shortcuts: new session, close session, send message, toggle sidebar, quick prompt, etc.

---

## Contributing

Contributions, bug reports, and suggestions are welcome!

1. Fork this repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'feat: add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Create a Pull Request

---

## Technology Stack

| Category | Technology |
|------|------|
| Desktop Framework | Tauri 2.x |
| Frontend | React 19 + TypeScript 5 + Vite 6 |
| UI | Shadcn/ui + Radix + Tailwind CSS 4 |
| State Management | Zustand 5 |
| Backend | Rust (Tokio async runtime) |
| Database | SQLite (rusqlite, bundled) |
| Document Processing | Python Sidecar (python-docx / openpyxl / python-pptx / PyMuPDF / reportlab) |
| PDF Preview | pdfjs-dist |
| Charts | Recharts |
| Auto Update | tauri-plugin-updater |

---

## License

This project is open-sourced under the [MIT License](./LICENSE).
