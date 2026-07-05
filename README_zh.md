<div align="center">

# DocAgent

[![Windows](https://img.shields.io/badge/platform-Windows-blue?logo=windows)](https://github.com/user-attachments/docagent)
[![Tauri 2](https://img.shields.io/badge/Tauri-2.x-orange?logo=tauri)](https://v2.tauri.app/)
[![React 19](https://img.shields.io/badge/React-19-61dafb?logo=react)](https://react.dev/)
[![Rust](https://img.shields.io/badge/Rust-1.80+-000000?logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-green)](./LICENSE)

[简体中文](./README_zh.md) | [English](./README.md)

<img src="assets/screenshots/Chinese.png" alt="DocAgent Screenshot" width="800" />

</div>

## 安装

从 [GitHub Releases](https://github.com/XuMingKe-06/DocAgent/releases) 下载最新版本 Windows 安装包，运行即可完成安装。

国内用户可访问 [Gitee 镜像](https://gitee.com/xumingke-06/doc-agent/releases) 下载。

## 核心功能

### AI 对话与文档处理
- 多轮对话式文档操作，AI 自主调用工具完成你的需求
- 实时流式输出 AI 的思考过程和结果
- 工作流时间线可视化，清晰展示 AI 的每一步操作
- 代码执行过程实时预览

### 支持多种 AI 模型
- OpenAI 兼容接口、Anthropic Claude、Google Gemini、Ollama 本地模型
- 支持自定义接口地址
- AI 服务健康状态实时检测，断线自动恢复
- Token 用量实时监控

### 工作区管理
- 支持多个工作区，每个工作区对应电脑上的一个目录
- 文件树浏览、文件搜索
- 可直接在工作区内创建、删除、重命名文件
- 目录被删除时自动检测并清理

### 文档处理
- Word（.docx）：读取、创建、编辑、转换格式、分析结构
- Excel（.xlsx）：读取、创建、编辑、提取数据
- PPT（.pptx）：读取、创建、编辑、提取幻灯片
- PDF：文字提取
- Markdown / 纯文本：读取与转换
- Python 代码：在沙箱环境中安全执行，支持绘图和数据分析

### 版本管理
- 文件修改时自动保存历史版本快照
- 可设置保留策略（按数量或天数）
- 查看版本历史、对比不同版本的差异
- 一键回滚到历史版本

### 会话管理
- 多会话切换，互不干扰
- 切换会话后 AI 仍在后台运行
- AI 自动为会话生成标题

### 提示词模板
- 内置多种常用模板
- 支持自定义模板和变量
- 按分类管理

### 界面与体验
- 深色模式 / 浅色模式 / 跟随系统
- 中文 / 英文界面
- 全局快捷键（Ctrl+N 新建会话、Ctrl+W 关闭、Ctrl+B 切换侧栏、Ctrl+, 设置）
- 支持上传图片、文档等附件
- 自动检测更新并安装

