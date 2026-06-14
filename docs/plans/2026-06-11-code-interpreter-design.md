# Code Interpreter 文档生成模式设计（精简 Sidecar + Code Interpreter）

> **注意**: 本文档中提到的 "Skill" 已重命名为 "Handler"，相关工具名如 `docx_skill` 已更改为 `docx_handler`。

## 一、背景与问题

### 1.1 当前架构

DocAgent 当前采用 **Sidecar 文档引擎**架构，文档操作的完整链路为：

```
用户请求 → LLM → 调用 Handler (docx_handler/xlsx_handler/pptx_handler/pdf_handler)
  → Rust 后端构造 JSON 请求 → stdin 发送给 Python Sidecar
  → Sidecar Handler 解析参数 → 调用 python-docx/openpyxl/python-pptx 等库
  → 返回 JSON 响应 → 文件生成到磁盘
```

每个 Handler 支持 5 种action：`generate`/`read`/`modify`/`convert`/`analyze`。

### 1.2 Sidecar 模式的局限

#### 局限 1：generate/modify 的 API 表面固定，无法表达复杂排版

LLM 只能调用预定义的参数结构。以下场景无法实现：

- **多栏布局**：python-docx 支持分栏，但 Sidecar API 未暴露此参数
- **复杂表格**：合并单元格、嵌套表格、条件格式等无法通过 JSON 参数描述
- **图表嵌入**：matplotlib/plotly 生成的图表无法插入 Word/PPT
- **SmartArt/形状**：python-pptx 支持形状和 SmartArt，但 API 未暴露
- **自定义样式**：无法精细控制段落间距、缩进、边框等样式细节

#### 局限 2：无动态计算能力

LLM 无法运行代码来处理数据后再生成文档：

- "读取这份 Excel，计算各产品同比增长率，生成一份带图表的分析报告" — 不可能
- "从 CSV 文件中提取数据，生成带数据透视表的 Excel" — 不可能
- "用 matplotlib 生成图表，插入 Word 文档" — 不可能

#### 局限 3：无法迭代优化

如果文档效果不理想，LLM 只能用 `modify` 操作的有限操作类型（replace/add_paragraph/add_table 等），无法编写自定义代码精细调整。

#### 局限 4：Prompt 膨胀

`document_design.rs` 中的设计指导占据了大量 token（约 4000+ tokens），但仍然无法覆盖所有场景。每增加一种能力，就需要增加更多 Prompt 指导。

### 1.3 Sidecar read/convert/analyze 的优势

与 generate/modify 不同，read/convert/analyze 三种操作具有以下特点：

| 特点 | 说明 |
|------|------|
| 输入输出固定 | 读取返回结构化内容，转换返回文件路径，分析返回统计信息 |
| 无创造性 | 不需要 LLM "设计"文档，只是提取/转换/统计 |
| 高频使用 | 读取文档内容是最常见的操作之一 |
| 低 token 消耗 | 一次 tool call 约 50 tokens 参数 |
| 高可靠性 | 固定 API，99%+ 成功率 |

用 Code Interpreter 替代这些操作，token 消耗增加 5-10 倍，延迟增加 2-4 倍，可靠性下降，得不偿失。

### 1.4 行业对比

| 产品 | 文档生成方式 | 灵活性 |
|------|-------------|--------|
| OpenAI ChatGPT | Code Interpreter (Python 沙箱) | 极高 |
| Anthropic Claude | Artifacts (代码生成 + 预览) | 高 |
| Google Gemini | Code Execution | 高 |
| 微软 Copilot | 插件 + 代码执行 | 高 |
| **DocAgent (当前)** | **Sidecar 结构化 API** | **低** |

几乎所有主流 AI Agent 产品都采用"代码生成 + 沙箱执行"的方式生成文档，而非固定的结构化 API。

---

## 二、方案设计

### 2.1 核心思路：精简 Sidecar + Code Interpreter

**将 Sidecar 从"全能文档引擎"精简为"文档工具层"，只保留 read/convert/analyze 三种操作。generate 和 modify 全部由 Code Interpreter 承担。**

```
┌──────────────────────────────────────────────────────────────┐
│                      Agent (LLM)                             │
│                                                              │
│  读取/转换/分析 → docx_handler / xlsx_handler / pptx_handler / ...│
│                   (精简 Sidecar，快速可靠)                   │
│                   action: read / convert / analyze           │
│                                                              │
│  生成/修改     → code_interpreter_handler                      │
│                   (Code Interpreter，灵活强大)               │
│                   编写 Python 代码，自由调用文档库            │
└──────────────────────┬───────────────────────────────────────┘
                       │
          ┌────────────┴────────────┐
          │                         │
    ┌─────▼─────┐          ┌───────▼────────┐
    │  精简版   │          │ Code Interpreter│
    │  Sidecar  │          │                 │
    │           │          │ 生成: 自由代码  │
    │ read      │          │ 修改: 自由代码  │
    │ convert   │          │                 │
    │ analyze   │          │ 可用库+helpers  │
    └───────────┘          └─────────────────┘
```

### 2.2 职责划分

| 操作 | 执行方式 | 理由 |
|------|---------|------|
| **generate**（生成文档） | `code_interpreter_handler` | 需要灵活性：图表、数据处理、自定义排版 |
| **modify**（修改文档） | `code_interpreter_handler` | 需要灵活性：精细调整、复杂修改 |
| **read**（读取文档） | `docx_handler`/`xlsx_handler`/... | 固定 API 足够，快速可靠，低 token 消耗 |
| **convert**（格式转换） | `docx_handler`/`xlsx_handler`/... | 固定 API 足够，转换逻辑标准 |
| **analyze**（文档分析） | `docx_handler`/`xlsx_handler`/... | 固定 API 足够，统计信息标准 |

### 2.3 设计原则

1. **职责清晰**：Sidecar 只做"读取/转换/分析"，Code Interpreter 只做"生成/修改"，LLM 无需纠结选择
2. **安全优先**：代码执行在受限命名空间中运行，多层安全防护
3. **Helper 优先**：提供封装好的 helper 函数，降低 LLM 编写文档代码的难度
4. **用户可控**：Code Interpreter 可在设置中启用/禁用（Handler 禁用机制），始终需用户确认
5. **错误可恢复**：代码执行失败时，LLM 可根据错误信息修改代码重试
6. **渐进迁移**：先新增 Code Interpreter，再精简 Sidecar，可分步上线

---

## 三、详细设计

### 3.1 Sidecar 精简：移除 generate 和 modify

#### 3.1.1 Handler 精简

每个 Handler（Word/Excel/PPT/PDF/Markdown）删除 `generate()` 和 `modify()` 方法，只保留 `read()`/`convert()`/`analyze()`。

以 `word_handler.py` 为例：

```python
# 修改前：5 个方法
class WordHandler:
    def generate(self, params): ...   # 删除
    def read(self, params): ...       # 保留
    def modify(self, params): ...     # 删除
    def convert(self, params): ...    # 保留
    def analyze(self, params): ...    # 保留

# 修改后：3 个方法
class WordHandler:
    def read(self, params): ...
    def convert(self, params): ...
    def analyze(self, params): ...
```

所有 5 个 Handler 均按此精简。预计代码量减少约 60%。

#### 3.1.2 Handler action 精简

Rust 端 `builtin.rs` 中各 Handler 的 `execute()` 方法移除 `generate` 和 `modify` 分支。

注意：当前代码中各 Handler 的 `execute()` 方法使用共享函数（如 `execute_generate(&self.doc_service, "docx", params).await`），而非实例方法（如 `self.execute_generate(params).await`）。精简时需删除对应的 `match` 分支和共享函数。

```rust
// 修改前（实际代码使用共享函数，非实例方法）
match action {
    "generate" => execute_generate(&self.doc_service, "docx", params).await,
    "read" => execute_read(&self.doc_service, "docx", params).await,
    "modify" => execute_modify(&self.doc_service, "docx", params).await,
    "convert" => execute_convert(&self.doc_service, "docx", params).await,
    "analyze" => execute_analyze(&self.doc_service, "docx", params).await,
    _ => ...
}

// 修改后
match action {
    "read" => execute_read(&self.doc_service, "docx", params).await,
    "convert" => execute_convert(&self.doc_service, "docx", params).await,
    "analyze" => execute_analyze(&self.doc_service, "docx", params).await,
    _ => ...
}
```

同时删除 `execute_generate()` 和 `execute_modify()` 两个共享函数，以及 `extract_content()`、`resolve_operation_paths()` 等仅被这两个函数使用的辅助函数。

#### 3.1.3 Handler parameters 精简

各 Handler 的 `parameters()` 中 `action` 的描述从 5 种缩减为 3 种：

```rust
"action": {
    "type": "string",
    "enum": ["read", "convert", "analyze"],
    "description": "操作类型: read=读取内容, convert=格式转换, analyze=文档分析"
}
```

#### 3.1.4 document_design.rs 精简

`document_design.rs` 中关于 generate 和 modify 的设计指导（占 Prompt 大部分内容）可以大幅精简或移除，因为生成和修改的指导已转移到 `code_interpreter.rs` 中。预计 token 消耗减少 60-70%。

### 3.2 Sidecar 新增 Code Interpreter Handler

在 `sidecar/main.py` 的 `HANDLERS` 注册表中新增 `CodeHandler`。Sidecar 的 `handle_request()` 通过 `action` 路由到 handler 的同名方法，因此 `CodeHandler` 需要实现 `execute` 方法，Rust 端调用时 action 为 `"execute"`、type 为 `"code"`。

```python
# sidecar/handlers/code_handler.py

class CodeHandler:
    """代码执行处理器 - 让 Agent 自由编写 Python 代码生成/修改文档"""

    # 允许的 Python 模块白名单
    ALLOWED_MODULES = {
        # 文档处理库
        "docx", "openpyxl", "pptx", "reportlab", "fpdf",
        # 数据处理库
        "pandas", "numpy", "csv", "json", "math", "statistics",
        # 图表库
        "matplotlib", "plotly",
        # 图像处理库
        "PIL", "pillow",
        # 日期时间
        "datetime", "dateutil",
        # 正则表达式
        "re",
        # 路径处理（受限）
        "os.path", "pathlib",
        # 类型相关
        "typing", "collections", "copy",
        # 编码
        "base64", "hashlib",
        # 项目内部 helper
        "doc_helpers",
    }

    # 禁止的模块黑名单（即使白名单中未列出也做二次拦截）
    BLOCKED_MODULES = {
        "subprocess", "socket", "http", "urllib",
        "shutil", "signal", "ctypes", "multiprocessing",
        "webbrowser", "telnetlib", "ftplib", "smtplib",
        "xmlrpc", "pickle", "shelve", "marshal",
    }

    # 禁止的代码模式（正则表达式）
    # 注意：不禁止 exec/eval/compile，因为 _execute_with_timeout 内部
    # 使用 exec() 执行用户代码，静态检查只拦截用户代码中的嵌套调用
    BLOCKED_PATTERNS = [
        r'__import__\s*\(',          # 禁止直接调用 __import__
        r'os\.system\s*\(',          # 禁止 os.system
        r'subprocess\.',             # 禁止 subprocess 模块
        r'import\s+os\b(?!\.path)',  # 禁止 import os（允许 import os.path）
    ]

    def execute(self, params: dict) -> dict:
        """执行 Python 代码生成/修改文档

        对应 Sidecar 请求: {"action": "execute", "type": "code", "params": {...}}

        params:
            code: Python 代码字符串
            working_dir: 工作目录（文件输出目录）
            timeout: 执行超时时间（秒），默认 60
        """
        code = params.get("code", "")
        working_dir = params.get("working_dir", "")
        timeout = params.get("timeout", 60)

        if not code:
            return {"error": "缺少代码内容"}

        # 安全检查
        security_check = self._check_security(code)
        if not security_check["safe"]:
            return {"error": f"代码安全检查未通过: {security_check['reason']}"}

        # 构建受限执行环境
        namespace = self._build_namespace(working_dir)

        # 执行代码（带超时）
        try:
            result = self._execute_with_timeout(code, namespace, timeout, working_dir)
            return result
        except TimeoutError:
            return {"error": f"代码执行超时（{timeout}秒）"}
        except Exception as e:
            return {"error": f"代码执行失败: {type(e).__name__}: {e}"}
```

### 3.3 安全机制设计

#### 3.3.1 多层安全防护

| 层级 | 机制 | 实现位置 | 说明 |
|------|------|---------|------|
| L1 | 代码静态检查 | `CodeHandler._check_security()` | 正则匹配禁止模式（不拦截 exec/eval，因为内部用 exec 执行用户代码） |
| L2 | 模块导入限制 | `CodeHandler._build_namespace()` | `__builtins__` 中移除 `__import__`，提供自定义 `__import__` 白名单校验 |
| L3 | 受限命名空间 | `CodeHandler._build_namespace()` | 只暴露白名单库和 helper 函数，移除 `exec`/`eval`/`compile` 等危险内建 |
| L4 | 文件系统隔离 | `CodeHandler._build_namespace()` | `open()` 被替换为受限版本，只允许写入工作区目录 |
| L5 | 执行超时 | `CodeHandler._execute_with_timeout()` | 默认 60 秒，防止死循环 |
| L6 | 用户确认 | `AgentExecutor.needs_confirmation()` | `code_interpreter_handler` 加入 `HIGH_RISK_HANDLERS` 常量，始终需确认 |
| L7 | 输出大小限制 | `CodeHandler.execute()` | stdout/stderr 截断，防止内存溢出 |

#### 3.3.2 受限命名空间构建

```python
def _build_namespace(self, working_dir: str) -> dict:
    """构建受限执行命名空间"""
    import builtins

    # 复制内建函数，移除危险项
    # 注意：不从 safe_builtins 中移除 exec，因为 _execute_with_timeout
    # 内部使用 exec(code, namespace) 执行用户代码，exec 需要在 __builtins__ 中可用
    # 但用户代码无法直接调用 exec/eval/compile，因为它们不在命名空间顶层
    safe_builtins = {k: v for k, v in builtins.__dict__.items()
                     if k not in ('__import__', 'breakpoint', 'exit', 'quit')}

    # 自定义安全导入函数
    def safe_import(name, *args, **kwargs):
        if name in self.BLOCKED_MODULES:
            raise ImportError(f"模块 '{name}' 被禁止导入")
        # 允许白名单中的模块
        if name in self.ALLOWED_MODULES or any(
            name.startswith(m + '.') for m in self.ALLOWED_MODULES
        ):
            return builtins.__import__(name, *args, **kwargs)
        raise ImportError(f"模块 '{name}' 不在允许列表中")

    safe_builtins['__import__'] = safe_import

    # 受限的 open() 函数：只允许写入工作区目录
    def safe_open(file, mode='r', *args, **kwargs):
        file_str = str(file)
        # 只允许读取操作和写入工作区目录
        if 'w' in mode or 'a' in mode:
            # 使用 os.path.abspath 规范化路径后比较，避免路径遍历攻击
            # Windows 下路径不区分大小写，使用 os.path.normcase 规范化
            abs_path = os.path.abspath(file_str)
            norm_working_dir = os.path.normcase(os.path.abspath(working_dir))
            norm_file = os.path.normcase(abs_path)
            if not norm_file.startswith(norm_working_dir):
                raise PermissionError(f"只允许写入工作区目录: {working_dir}")
        return builtins.open(file, mode, *args, **kwargs)

    safe_builtins['open'] = safe_open

    # 初始化命名空间
    namespace = {
        '__builtins__': safe_builtins,
        'working_dir': working_dir,
    }

    # 预导入常用文档处理库
    try:
        from docx import Document
        namespace['Document'] = Document
    except ImportError:
        pass

    try:
        import openpyxl
        namespace['openpyxl'] = openpyxl
    except ImportError:
        pass

    try:
        from pptx import Presentation
        namespace['Presentation'] = Presentation
    except ImportError:
        pass

    try:
        import matplotlib.pyplot as plt
        namespace['plt'] = plt
    except ImportError:
        pass

    try:
        import pandas as pd
        namespace['pd'] = pd
    except ImportError:
        pass

    # 导入项目 helper 函数
    try:
        import doc_helpers
        namespace['doc_helpers'] = doc_helpers
        # 将常用 helper 直接暴露在命名空间顶层，降低 LLM 编码难度
        for name in ['create_word_doc', 'save_word_doc',
                     'create_excel_doc', 'save_excel_doc',
                     'create_ppt_doc', 'save_ppt_doc',
                     'create_pdf_doc', 'save_pdf_doc',
                     'create_chart', 'save_chart',
                     'add_styled_table', 'apply_theme']:
            if hasattr(doc_helpers, name):
                namespace[name] = getattr(doc_helpers, name)
    except ImportError:
        pass

    return namespace
```

#### 3.3.3 超时执行机制

```python
import threading

def _execute_with_timeout(self, code: str, namespace: dict, timeout: int, working_dir: str) -> dict:
    """带超时的代码执行"""
    import sys
    from io import StringIO

    # 捕获 stdout
    old_stdout = sys.stdout
    captured_output = StringIO()
    sys.stdout = captured_output

    result = {"success": False, "output": "", "files": [], "error": None}

    # 执行前记录工作目录中的文件集合（用于追踪生成的文件）
    before_files = set()
    if working_dir and os.path.isdir(working_dir):
        for root, dirs, files in os.walk(working_dir):
            for f in files:
                before_files.add(os.path.join(root, f))

    try:
        exec_result = [None]
        exec_error = [None]

        def run_code():
            try:
                exec(code, namespace)
                exec_result[0] = True
            except Exception as e:
                exec_error[0] = e

        thread = threading.Thread(target=run_code)
        thread.daemon = True
        thread.start()
        thread.join(timeout=timeout)

        if thread.is_alive():
            raise TimeoutError(f"代码执行超时（{timeout}秒）")

        if exec_error[0]:
            raise exec_error[0]

        # 执行后比较工作目录文件变化，追踪新生成的文件
        generated_files = []
        if working_dir and os.path.isdir(working_dir):
            for root, dirs, files in os.walk(working_dir):
                for f in files:
                    full_path = os.path.join(root, f)
                    if full_path not in before_files:
                        generated_files.append(full_path)

        result["success"] = True
        result["output"] = captured_output.getvalue()[:10000]  # 限制输出大小
        result["files"] = generated_files

    except TimeoutError:
        raise
    except Exception as e:
        result["error"] = f"{type(e).__name__}: {e}"
        result["output"] = captured_output.getvalue()[:10000]
    finally:
        sys.stdout = old_stdout

    return result
```

文件追踪采用"执行前后工作目录文件对比"的方式，而非依赖 helper 函数主动报告。这种方式更可靠，不依赖 `inspect` 模块或全局变量传递，能捕获所有通过 `safe_open()` 或文档库 `save()` 方法写入的文件。

### 3.4 Helper 函数库 (`doc_helpers`)

为降低 LLM 编写文档代码的难度，提供封装好的 helper 函数。这些函数内置了当前 Sidecar generate/modify 中的专业配色方案和样式规范，将原有 Sidecar 的专业能力以代码库形式保留。

```python
# sidecar/handlers/doc_helpers/__init__.py

"""DocAgent 文档生成 Helper 函数库
提供封装好的文档生成函数，内置专业配色方案和样式规范
"""

from .word_helpers import create_word_doc, save_word_doc
from .excel_helpers import create_excel_doc, save_excel_doc
from .ppt_helpers import create_ppt_doc, save_ppt_doc
from .pdf_helpers import create_pdf_doc, save_pdf_doc
from .chart_helpers import create_chart, save_chart
from .common import (
    THEME_COLORS,      # 专业配色方案
    apply_theme,       # 应用配色方案
    add_styled_table,  # 添加专业样式表格
)

__all__ = [
    'create_word_doc', 'save_word_doc',
    'create_excel_doc', 'save_excel_doc',
    'create_ppt_doc', 'save_ppt_doc',
    'create_pdf_doc', 'save_pdf_doc',
    'create_chart', 'save_chart',
    'THEME_COLORS', 'apply_theme', 'add_styled_table',
]
```

#### 3.4.1 Word Helper 示例

```python
# sidecar/handlers/doc_helpers/word_helpers.py

"""Word 文档生成 Helper 函数
封装 python-docx 常用操作，内置专业配色方案
"""

from docx import Document
from docx.shared import Inches, Pt, Cm, RGBColor
from docx.enum.text import WD_ALIGN_PARAGRAPH
import os

# 专业配色方案（与原 document_design.rs 保持一致）
THEME = {
    "heading1": RGBColor(0x1F, 0x4E, 0x79),  # 深蓝色
    "heading2": RGBColor(0x2E, 0x75, 0xB6),  # 中蓝色
    "heading3": RGBColor(0x5B, 0x9B, 0xD5),  # 浅蓝色
    "table_header_bg": "D6E4F0",
    "table_alt_row_bg": "EDF2F9",
    "table_border": "B4C6E7",
    "accent": RGBColor(0x2E, 0x75, 0xB6),
}

EAST_ASIAN_FONT = "微软雅黑"
LATIN_FONT = "Arial"


def create_word_doc(title=None, page_size="a4", author=""):
    """创建一个预配置好专业样式的 Word 文档对象

    Args:
        title: 文档标题（可选）
        page_size: 页面尺寸 "a4" 或 "letter"
        author: 文档作者

    Returns:
        Document: 预配置好的 python-docx Document 对象

    示例:
        doc = create_word_doc(title="季度报告", author="张三")
        doc.add_paragraph("这是正文内容")
        save_word_doc(doc, "季度报告.docx")
    """
    doc = Document()

    # 设置页面尺寸和边距
    section = doc.sections[0]
    section.top_margin = Cm(2.54)
    section.bottom_margin = Cm(2.54)
    section.left_margin = Cm(2.54)
    section.right_margin = Cm(2.54)

    if page_size == "letter":
        section.page_width = Inches(8.5)
        section.page_height = Inches(11)

    # 设置默认字体
    style = doc.styles["Normal"]
    style.font.name = LATIN_FONT
    style.font.size = Pt(12)
    style.paragraph_format.line_spacing = 1.5

    # 设置标题样式
    for level, (size, color) in enumerate([
        (Pt(22), THEME["heading1"]),
        (Pt(16), THEME["heading2"]),
        (Pt(14), THEME["heading3"]),
    ], start=1):
        heading_style = doc.styles[f"Heading {level}"]
        heading_style.font.name = LATIN_FONT
        heading_style.font.size = size
        heading_style.font.bold = True
        heading_style.font.color.rgb = color

    # 添加标题
    if title:
        doc.core_properties.title = title
        title_para = doc.add_paragraph()
        title_run = title_para.add_run(title)
        title_run.font.size = Pt(26)
        title_run.font.bold = True
        title_run.font.color.rgb = THEME["heading1"]
        title_para.alignment = WD_ALIGN_PARAGRAPH.CENTER

    if author:
        doc.core_properties.author = author

    return doc


def save_word_doc(doc, filename, working_dir=""):
    """保存 Word 文档到工作目录

    Args:
        doc: python-docx Document 对象
        filename: 文件名（如 "报告.docx"）
        working_dir: 工作目录路径

    Returns:
        str: 保存的文件绝对路径
    """
    output_path = os.path.join(working_dir, filename) if working_dir else filename
    os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)
    doc.save(output_path)

    return output_path
```

#### 3.4.2 Chart Helper 示例

```python
# sidecar/handlers/doc_helpers/chart_helpers.py

"""图表生成 Helper 函数
封装 matplotlib 常用图表，自动保存为图片文件
"""

import matplotlib
matplotlib.use('Agg')  # 无头模式，不弹出窗口
import matplotlib.pyplot as plt
import os


def create_chart(chart_type="bar", data=None, title="", xlabel="", ylabel="",
                 filename="chart.png", working_dir="", **kwargs):
    """创建图表并保存为图片文件

    Args:
        chart_type: 图表类型 "bar"|"line"|"pie"|"scatter"|"area"|"hist"
        data: 图表数据（dict 或 DataFrame）
        title: 图表标题
        xlabel: X 轴标签
        ylabel: Y 轴标签
        filename: 输出文件名
        working_dir: 工作目录

    Returns:
        str: 图片文件绝对路径

    示例:
        chart_path = create_chart(
            chart_type="bar",
            data={"x": ["Q1", "Q2", "Q3", "Q4"], "y": [100, 150, 120, 180]},
            title="季度销售额",
            ylabel="万元",
            filename="sales_chart.png"
        )
    """
    fig, ax = plt.subplots(figsize=kwargs.get("figsize", (8, 5)))

    # 设置中文字体
    plt.rcParams['font.sans-serif'] = ['Microsoft YaHei', 'SimHei', 'Arial']
    plt.rcParams['axes.unicode_minus'] = False

    if chart_type == "bar":
        ax.bar(data["x"], data["y"], color=kwargs.get("color", "#2E75B6"))
    elif chart_type == "line":
        ax.plot(data["x"], data["y"], marker='o', color=kwargs.get("color", "#2E75B6"))
    elif chart_type == "pie":
        ax.pie(data["values"], labels=data["labels"], autopct='%1.1f%%')
    elif chart_type == "scatter":
        ax.scatter(data["x"], data["y"], color=kwargs.get("color", "#2E75B6"))
    elif chart_type == "area":
        ax.fill_between(data["x"], data["y"], alpha=0.3, color=kwargs.get("color", "#2E75B6"))
        ax.plot(data["x"], data["y"], color=kwargs.get("color", "#2E75B6"))
    elif chart_type == "hist":
        ax.hist(data["values"], bins=kwargs.get("bins", 10), color=kwargs.get("color", "#2E75B6"))

    if title:
        ax.set_title(title, fontsize=14, fontweight='bold')
    if xlabel:
        ax.set_xlabel(xlabel)
    if ylabel:
        ax.set_ylabel(ylabel)

    plt.tight_layout()

    output_path = os.path.join(working_dir, filename) if working_dir else filename
    os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)
    fig.savefig(output_path, dpi=150, bbox_inches='tight')
    plt.close(fig)

    return output_path
```

### 3.5 Rust 端：新增 `code_interpreter_handler` Handler

在 `src-tauri/src/services/handler/builtin.rs` 中注册新的 Handler。命名遵循现有规范（`docx_handler`/`xlsx_handler`/`pptx_handler`/`pdf_handler`），使用 `code_interpreter_handler`。

```rust
// ============================================================================
// CodeInterpreterHandler - 代码解释器处理器
// ============================================================================

/// 代码解释器处理器
/// 让 Agent 自由编写 Python 代码生成/修改文档
/// 承担原有 generate 和 modify 操作的全部职责
struct CodeInterpreterHandler {
    doc_service: Arc<DocumentService>,
}

impl CodeInterpreterHandler {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Handler for CodeInterpreterHandler {
    fn handler_name(&self) -> &str { "code_interpreter_handler" }
    fn description(&self) -> &str {
        "代码解释器，通过编写和执行 Python 代码生成和修改文档。所有文档生成和修改操作都通过此处理器完成。可用库: python-docx, openpyxl, python-pptx, reportlab, matplotlib, pandas, numpy, Pillow。可用 helper: create_word_doc(), save_word_doc() 等。"
    }
    fn category(&self) -> &str { "document" }
    fn is_builtin(&self) -> bool { true }
    fn supported_types(&self) -> Vec<String> {
        vec!["docx".into(), "xlsx".into(), "pptx".into(), "pdf".into()]
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "code": {
                    "type": "string",
                    "description": "要执行的 Python 代码。可用库: python-docx, openpyxl, python-pptx, reportlab, matplotlib, pandas, numpy, Pillow。可用 helper: create_word_doc(), save_word_doc(), create_excel_doc(), save_excel_doc(), create_ppt_doc(), save_ppt_doc(), create_pdf_doc(), save_pdf_doc(), create_chart(), save_chart()。工作目录变量: working_dir"
                },
                "description": {
                    "type": "string",
                    "description": "代码功能的简要描述，用于用户确认时展示"
                },
                "timeout": {
                    "type": "integer",
                    "description": "执行超时时间（秒），默认 60，最大 120",
                    "default": 60
                },
                "expected_files": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "预期生成的文件名列表（如 [\"报告.docx\", \"chart.png\"]）"
                }
            },
            "required": ["code", "description"]
        })
    }
    async fn execute(&self, params: Value) -> HandlerResult {
        let start = Instant::now();
        let code = params["code"].as_str().unwrap_or("");
        let description = params["description"].as_str().unwrap_or("");
        let timeout = params["timeout"].as_u64().unwrap_or(60).min(120);
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        if code.is_empty() {
            return HandlerResult {
                success: false,
                output: None,
                error: Some("缺少代码内容".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }

        // 调用 Sidecar：action="execute", type="code"
        // Sidecar handle_request() 通过 getattr(handler, action) 路由
        // CodeHandler 实现了 execute() 方法
        let sidecar_params = json!({
            "code": code,
            "working_dir": workspace_root,
            "timeout": timeout,
        });

        match self.doc_service.process("execute", "code", sidecar_params).await {
            Ok(data) => {
                let mut output = data;
                output["description"] = json!(description);
                HandlerResult {
                    success: true,
                    output: Some(output),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                }
            }
            Err(e) => HandlerResult {
                success: false,
                output: None,
                error: Some(e.message),
                duration_ms: start.elapsed().as_millis() as u64,
            },
        }
    }
}
```

注册时修改 `register_builtin_handlers()` 函数：

```rust
pub fn register_builtin_handlers(
    registry: &mut super::registry::HandlerRegistry,
    doc_service: Arc<DocumentService>,
) {
    log::info!("开始注册内置处理器");
    registry.register(Box::new(DocxHandler::new(doc_service.clone())));
    registry.register(Box::new(XlsxHandler::new(doc_service.clone())));
    registry.register(Box::new(PptxHandler::new(doc_service.clone())));
    registry.register(Box::new(PdfHandler::new(doc_service.clone())));
    registry.register(Box::new(CodeInterpreterHandler::new(doc_service)));
    log::info!("内置处理器注册完成, 共注册 5 个处理器");
}
```

### 3.6 确认机制与 Executor 集成

Code Interpreter 执行动态代码，始终需要用户确认。需要修改 `executor.rs` 中的五处：

#### 3.6.1 加入高风险 Handler 常量

```rust
// executor.rs 顶部常量
/// 始终需要确认的高风险 Handler 列表
const HIGH_RISK_HANDLERS: &[&str] = &["delete_file", "code_interpreter_handler"];
```

这样 `needs_confirmation()` 的 `EditOnly` 分支会自动将 `code_interpreter_handler` 视为高风险，无需修改 `needs_confirmation()` 方法本身。

注意：精简后文档 Handler 不再有 `modify` action，`needs_confirmation()` 中 `matches!(name, "docx_handler" | "xlsx_handler" | "pptx_handler" | "pdf_handler") && params["action"].as_str() == Some("modify")` 这段检查将永远不会触发，可以在 Phase 2 中清理移除。

#### 3.6.2 注入 workspace_root

在 executor 的 `needs_workspace_root` 匹配列表中添加 `code_interpreter_handler`：

```rust
let needs_workspace_root = matches!(
    tool_call.name.as_str(),
    "list_directory" | "search_files" | "read_file" | "file_info"
    | "file_exists" | "delete_file" | "create_directory" | "write_text_file"
    | "docx_handler" | "xlsx_handler" | "pptx_handler" | "pdf_handler"
    | "code_interpreter_handler"  // 新增：需要 workspace_root 作为 working_dir
);
```

#### 3.6.3 确认描述增强

在 `request_confirmation()` 的 `description` 匹配中添加 `code_interpreter_handler`：

```rust
let description = match tool_name {
    "delete_file" => format!("删除文件: {}", arguments["path"].as_str().unwrap_or("未知")),
    "docx_handler" | "xlsx_handler" | "pptx_handler" | "pdf_handler" => {
        let action = arguments["action"].as_str().unwrap_or("未知操作");
        let path = arguments["path"].as_str().unwrap_or("未知文件");
        format!("{} 文档 - {}: {}", tool_name, action, path)
    }
    "code_interpreter_handler" => {
        // 展示代码描述和代码摘要
        let desc = arguments["description"].as_str().unwrap_or("执行代码");
        let code_preview: String = arguments["code"].as_str()
            .map(|c| if c.len() > 200 { format!("{}...", &c[..200]) } else { c.to_string() })
            .unwrap_or_default();
        format!("执行代码: {}\n{}", desc, code_preview)
    }
    _ => format!("执行操作: {}", tool_name),
};
```

#### 3.6.4 快照路径提取

`code_interpreter_handler` 的生成和修改操作都会修改文件，需要在操作前创建版本快照。修改 `extract_snapshot_paths()` 方法：

```rust
fn extract_snapshot_paths(&self, handler_name: &str, params: &serde_json::Value) -> Vec<String> {
    match handler_name {
        "delete_file" => {
            vec![params["path"].as_str().unwrap_or("").to_string()]
        }
        "docx_handler" | "xlsx_handler" | "pptx_handler" | "pdf_handler" => {
            // 仅在修改操作时创建快照
            if params["action"].as_str() == Some("modify") {
                vec![params["path"].as_str().unwrap_or("").to_string()]
            } else {
                Vec::new()
            }
        }
        "code_interpreter_handler" => {
            // Code Interpreter 可能修改多个文件，提取预期文件列表
            // 1. 优先从 expected_files 参数提取（LLM 声明的预期输出文件）
            // 2. 如果没有 expected_files，则不创建快照（因为无法预知文件路径）
            if let Some(files) = params["expected_files"].as_array() {
                files.iter()
                    .filter_map(|f| f.as_str().map(|s| s.to_string()))
                    .collect()
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}
```

#### 3.6.5 确认风险等级增强

在 `request_confirmation()` 的 `risk_level` 匹配中添加 `code_interpreter_handler`：

```rust
let risk_level = match self.confirmation_level {
    ConfirmationLevel::Always => {
        if tool_name == "delete_file" {
            "critical"
        } else if tool_name == "code_interpreter_handler" {
            "high"  // 代码执行始终为高风险
        } else if matches!(tool_name, "docx_handler" | "xlsx_handler" | "pptx_handler" | "pdf_handler")
            && arguments["action"].as_str() == Some("modify")
        {
            "high"
        } else {
            "normal"
        }
    }
    _ => {
        if tool_name == "delete_file" {
            "critical"
        } else {
            "high"
        }
    }
};
```

### 3.7 Sidecar main.py 修改

```python
from handlers.code_handler import CodeHandler

HANDLERS = {
    "docx": WordHandler(),
    "xlsx": ExcelHandler(),
    "pptx": PptHandler(),
    "pdf": PdfHandler(),
    "md": MarkdownHandler(),
    "markdown": MarkdownHandler(),
    "txt": MarkdownHandler(),
    "code": CodeHandler(),  # 新增
}
```

`handle_request()` 函数无需修改，因为它是通用的 action/type 路由。

### 3.8 Prompt 设计

在 `src-tauri/src/services/agent/prompts/` 中新增 `code_interpreter.rs`：

**注意**：需要在 `mod.rs` 中添加 `pub mod code_interpreter;`。

Prompt 加载采用分层架构（见 `prompt_loader.rs`），Code Interpreter 的指导应集成到 `tool_strategy` 层中。具体方式是在 `default_tool_strategy()` 方法中添加 Code Interpreter 的选择策略和示例代码。

同时，`document_design.rs` 中关于 generate/modify 的设计指导应大幅精简（减少 60-70% token），只保留 read/convert/analyze 的指导。

```rust
/// Code Interpreter 使用指导（将集成到 tool_strategy 层）
pub const CODE_INTERPRETER_GUIDE: &str = r#"
### 文档生成与修改 -> code_interpreter_handler

所有文档的**生成**和**修改**操作都通过 `code_interpreter_handler` 完成，编写 Python 代码执行。

#### 何时使用 code_interpreter_handler
- 生成任何文档（Word/Excel/PPT/PDF）
- 修改任何文档（调整样式、添加内容、替换文本等）
- 需要图表（matplotlib）
- 需要数据处理（pandas）
- 需要自定义排版
- 需要计算后生成报告

#### 何时使用文档 Handler（docx_handler/xlsx_handler/pptx_handler/pdf_handler）
- 读取文档内容 -> action="read"
- 格式转换 -> action="convert"
- 文档分析统计 -> action="analyze"

#### 代码编写规范

1. **使用 helper 函数**：优先使用 `create_word_doc()`、`save_word_doc()` 等 helper，它们内置了专业配色方案
2. **保存到 working_dir**：所有输出文件保存到 `working_dir` 变量指定的目录
3. **中文支持**：matplotlib 使用 `plt.rcParams['font.sans-serif'] = ['Microsoft YaHei']`
4. **错误处理**：代码应有基本的 try/except，避免因小错误导致整体失败
5. **代码简洁**：一次只做一件事，避免过长的代码

#### 示例：生成带图表的 Word 报告

    doc = create_word_doc(title="销售分析报告", author="DocAgent")
    doc.add_heading('季度销售概览', level=1)
    doc.add_paragraph('本报告分析了2024年各季度的销售数据。')
    chart_path = create_chart(
        chart_type="bar",
        data={"x": ["Q1", "Q2", "Q3", "Q4"], "y": [120, 150, 135, 180]},
        title="季度销售额（万元）",
        filename="sales_chart.png",
        working_dir=working_dir
    )
    doc.add_picture(chart_path, width=Inches(5))
    save_word_doc(doc, "销售分析报告.docx", working_dir=working_dir)

#### 示例：修改现有文档

    from docx import Document
    doc = Document(working_dir + "/报告.docx")
    # 修改标题
    doc.paragraphs[0].runs[0].text = "2024年度销售分析报告"
    # 添加新章节
    doc.add_heading('结论与建议', level=1)
    doc.add_paragraph('基于以上分析，我们建议...')
    doc.save(working_dir + "/报告.docx")
"#;
```

集成方式：修改 `prompt_loader.rs` 的 `default_tool_strategy()` 方法，在现有工具策略末尾追加 `CODE_INTERPRETER_GUIDE` 内容。同时需要将 `default_tool_strategy()` 中"写入操作"部分的 generate/modify 指引改为指向 `code_interpreter_handler`：

```rust
// 修改前
### 写入操作
- 纯文本文件 -> write_text_file
- 生成Word文档 -> docx_handler，action="generate"
- 生成Excel文档 -> xlsx_handler，action="generate"
- 生成PPT文档 -> pptx_handler，action="generate"
- 生成PDF文档 -> pdf_handler，action="generate"
- 修改已有文档 -> 对应 Handler 的 action="modify"

// 修改后
### 写入操作
- 纯文本文件 -> write_text_file
- 生成/修改文档 -> code_interpreter_handler（编写 Python 代码生成或修改任意文档）
```

### 3.9 前端变更

#### 3.9.1 设置页面

当前 `HandlersTab.tsx` 中所有内置 Handler 都显示"始终启用"（`tool-always-on`），没有启用/禁用开关。需要新增开关功能。

当前 `HandlerInfo` 类型（与 Rust 端 `models/handler.rs` 对齐，使用 camelCase）包含 `id`/`name`/`description`/`category`/`isBuiltin`/`enabled`/`version`/`paramsSchema`/`supportedTypes` 字段，其中 `enabled` 字段已存在但前端未使用。

修改方案：

```typescript
// HandlersTab.tsx 修改
// 1. 将内置 Handler 的"始终启用"改为可切换的 Switch 开关
// 2. code_interpreter_handler 标注为"高级功能"，启用时显示安全提示
// 3. 调用 toggle_handler 命令切换启用/禁用状态

const isCodeInterpreter = s.id === "code_interpreter_handler";

<div className="handler-item-info">
  <div className="handler-name-row">
    <span className="handler-name">{s.name}</span>
    <span className="handler-badge">{t('settings.handlers.skillBadge')}</span>
    {isCodeInterpreter && (
      <span className="handler-advanced-badge">高级</span>
    )}
  </div>
  <div className="handler-desc">{s.description}</div>
</div>
<div className="handler-toggle">
  <Switch
    checked={s.enabled}
    onCheckedChange={(checked) => handleToggleHandler(s.id, checked)}
  />
</div>
```

注意：需要确认 `toggle_handler` Tauri 命令是否已存在。如果不存在，需要在 Rust 端 `commands/handler.rs` 中新增该命令，调用 `HandlerRegistry` 的禁用/启用方法，并持久化到 `AppSettings.disabled_handlers`。

#### 3.9.2 确认弹窗

当前 `ConfirmPayload` 结构（定义在 `src/services/event.ts`）包含 `sessionId`/`operationId`/`operationType`/`description`/`details`/`riskLevel` 字段。确认数据来自 Rust 端 `request_confirmation()` 构造的 `ConfirmPayload`，其中 `details` 字段包含完整参数（含 `code`）。

前端在 `useAgent.ts` 中接收 `ConfirmPayload` 后，将其映射为 `ConfirmNodeData`（`title`/`description`/`confirmLabel`/`cancelLabel`/`confirmed`）。当前映射逻辑在 `useWorkflowStore.ts` 中处理。

对于 `code_interpreter_handler`，Rust 端 `request_confirmation()` 的 `description` 已包含代码摘要（见 3.6.3），前端无需额外处理。`ConfirmNode` 组件本身无需修改，因为 `description` 字段已包含代码摘要信息。如果需要更丰富的代码展示（语法高亮、折叠），可在 `ConfirmNode` 中根据 `operationType === "code_interpreter_handler"` 条件渲染。

#### 3.9.3 工作流展示

当前 `ToolNodeData` 结构（定义在 `src/types/workflow.ts`）包含 `toolName`/`briefDescription`/`input`/`callId`/`success`/`error` 字段，没有 `result` 字段。`briefDescription` 由 `src/utils/format.ts` 中的 `generateToolBrief()` 函数生成，该函数根据 `toolName` 和 `input` 参数生成简短描述。

需要修改 `generateToolBrief()` 函数，添加 `code_interpreter_handler` 的分支：

```typescript
// src/utils/format.ts - generateToolBrief() 新增分支
case "code_interpreter_handler":
  return `${i18n.t('toolBrief.executeCode')} ${f("description") || ""}`;
```

同时需要在国际化文件中添加 `toolBrief.executeCode` 翻译键。

### 3.10 错误处理与重试

Code Interpreter 的错误处理遵循以下策略：

1. **代码语法错误**：返回错误信息，LLM 可根据错误修改代码重试
2. **运行时错误**：返回异常信息和 traceback，LLM 可据此修复
3. **超时错误**：返回超时提示，LLM 可简化代码或增加超时时间
4. **安全检查失败**：返回禁止原因，LLM 需修改代码移除危险操作
5. **文件写入失败**：返回权限错误，LLM 可修改输出路径

Agent 的迭代循环（`AgentExecutor`）天然支持重试：LLM 收到错误后会尝试修改代码并重新调用 `code_interpreter_handler`。

---

## 四、文件变更清单

### 4.1 新增文件

| 文件路径 | 说明 |
|---------|------|
| `sidecar/handlers/code_handler.py` | Code Interpreter 核心处理器 |
| `sidecar/handlers/doc_helpers/__init__.py` | Helper 函数库入口 |
| `sidecar/handlers/doc_helpers/word_helpers.py` | Word 文档 Helper（从原 WordHandler.generate/modify 迁移） |
| `sidecar/handlers/doc_helpers/excel_helpers.py` | Excel 文档 Helper（从原 ExcelHandler.generate/modify 迁移） |
| `sidecar/handlers/doc_helpers/ppt_helpers.py` | PPT 文档 Helper（从原 PptHandler.generate/modify 迁移） |
| `sidecar/handlers/doc_helpers/pdf_helpers.py` | PDF 文档 Helper（从原 PdfHandler.generate/modify 迁移） |
| `sidecar/handlers/doc_helpers/chart_helpers.py` | 图表生成 Helper |
| `sidecar/handlers/doc_helpers/common.py` | 公共样式和工具函数（从原 document_design.rs 配色方案迁移） |
| `src-tauri/src/services/agent/prompts/code_interpreter.rs` | Code Interpreter Prompt 指导 |

### 4.2 修改文件

| 文件路径 | 变更内容 |
|---------|---------|
| `sidecar/main.py` | HANDLERS 注册表新增 `"code": CodeHandler()` |
| `sidecar/handlers/word_handler.py` | 删除 `generate()` 和 `modify()` 方法 |
| `sidecar/handlers/excel_handler.py` | 删除 `generate()` 和 `modify()` 方法 |
| `sidecar/handlers/ppt_handler.py` | 删除 `generate()` 和 `modify()` 方法 |
| `sidecar/handlers/pdf_handler.py` | 删除 `generate()` 和 `modify()` 方法 |
| `sidecar/handlers/markdown_handler.py` | 删除 `generate()` 和 `modify()` 方法 |
| `sidecar/requirements.txt` | 新增 `matplotlib`, `pandas`, `numpy` 依赖 |
| `src-tauri/src/services/handler/builtin.rs` | 新增 `CodeInterpreterHandler`；各 Handler 删除 `execute_generate()` 和 `execute_modify()`；`register_builtin_handlers()` 注册新 Handler |
| `src-tauri/src/services/agent/executor.rs` | `HIGH_RISK_HANDLERS` 新增 `code_interpreter_handler`；`needs_workspace_root` 新增 `code_interpreter_handler`；`request_confirmation()` description 新增 `code_interpreter_handler` 分支；`extract_snapshot_paths()` 新增 `code_interpreter_handler` 分支；`risk_level` 匹配新增 `code_interpreter_handler` |
| `src-tauri/src/services/agent/prompts/mod.rs` | 新增 `pub mod code_interpreter;` |
| `src-tauri/src/services/agent/prompts/prompt_loader.rs` | `default_tool_strategy()` 末尾追加 Code Interpreter 指导 |
| `src-tauri/src/services/agent/prompts/document_design.rs` | 大幅精简，移除 generate/modify 相关指导，只保留 read/convert/analyze 指导 |
| `src/components/settings/HandlersTab.tsx` | 新增 Handler 启用/禁用 Switch 开关，`code_interpreter_handler` 标注"高级" |
| `src/utils/format.ts` | `generateToolBrief()` 新增 `code_interpreter_handler` 分支 |
| `src/i18n/locales/zh-CN.json` | 新增翻译键（`toolBrief.executeCode` 等） |
| `src/i18n/locales/en-US.json` | 新增翻译键（`toolBrief.executeCode` 等） |

### 4.3 删除文件

无。原有 Handler 文件保留（精简后），不删除。

---

## 五、实施计划

### Phase 1：新增 Code Interpreter（预计 3-4 天）

1. **创建 `code_handler.py`**：实现安全检查、受限命名空间、超时执行
2. **创建 `doc_helpers` 基础模块**：`word_helpers.py` + `common.py`（先实现 Word，验证链路）
3. **修改 `sidecar/main.py`**：注册 CodeHandler
4. **创建 `code_interpreter.rs`**：Prompt 指导
5. **修改 `builtin.rs`**：注册 CodeInterpreterHandler
6. **修改 `executor.rs`**：HIGH_RISK_HANDLERS + workspace_root 注入 + 确认描述 + 快照路径 + 风险等级
7. **修改 `format.ts`**：`generateToolBrief()` 新增 `code_interpreter_handler` 分支
8. **端到端测试**：验证 Code Interpreter 完整链路

此阶段**不修改**现有 Handler 和 Sidecar Handler，确保现有功能不受影响。

### Phase 2：精简 Sidecar Handler（预计 2-3 天）

1. **删除各 Handler 的 `generate()` 方法**：Word/Excel/PPT/PDF/Markdown
2. **删除各 Handler 的 `modify()` 方法**：Word/Excel/PPT/PDF/Markdown
3. **删除各 Handler 的 `execute_generate()` 和 `execute_modify()`**
4. **精简 `document_design.rs`**：移除 generate/modify 指导
5. **完善 `doc_helpers`**：将原 Handler 中的专业样式逻辑迁移到 Helper
6. **实现 `excel_helpers.py`/`ppt_helpers.py`/`pdf_helpers.py`/`chart_helpers.py`**
7. **更新 `requirements.txt`**：新增依赖
8. **回归测试**：确保 read/convert/analyze 不受影响

### Phase 3：前端与体验优化（预计 2-3 天）

1. **修改 `HandlersTab.tsx`**：启用/禁用开关
2. **修改 `useAgent.ts`**：briefDescription 提取逻辑
3. **优化确认弹窗**：代码展示区域（可选增强）
4. **优化错误提示**：代码执行失败时的友好提示
5. **更新国际化文件**：新增翻译键

### Phase 4：安全加固（可选，预计 1-2 天）

1. **引入 `RestrictedPython`**：更严格的代码静态分析
2. **子进程沙箱**：在独立子进程中执行代码，避免主 Sidecar 进程崩溃
3. **资源限制**：内存使用限制、文件大小限制
4. **审计日志**：记录所有代码执行历史

---

## 六、风险评估

### 6.1 安全风险

| 风险 | 等级 | 缓解措施 |
|------|------|---------|
| LLM 生成恶意代码 | 高 | 多层安全防护 + 用户确认 |
| 代码执行导致 Sidecar 崩溃 | 中 | 超时机制 + 子进程隔离（Phase 4） |
| 文件系统越界访问 | 高 | 受限 `open()` + 路径校验 |
| 资源耗尽（死循环/内存溢出） | 中 | 超时 + 输出大小限制 |
| 代码注入攻击 | 低 | 受限命名空间中移除 exec/eval/compile，用户代码通过 CodeHandler 内部 exec 执行 |

### 6.2 功能风险

| 风险 | 等级 | 缓解措施 |
|------|------|---------|
| LLM 生成的代码质量差 | 中 | Helper 函数降低难度 + 错误重试 |
| Python 依赖缺失 | 低 | requirements.txt + 启动时检查 |
| 代码执行超时 | 低 | 默认 60 秒超时 + LLM 可调整 |
| 原 generate/modify 功能回退 | 中 | Helper 函数封装原有专业样式逻辑，确保文档质量不降级 |
| 精简 Sidecar 后 read/convert/analyze 回归 | 低 | Phase 2 专门回归测试 |

### 6.3 性能风险

| 风险 | 等级 | 缓解措施 |
|------|------|---------|
| matplotlib 首次导入慢 | 低 | 预导入到命名空间 |
| 大数据量处理慢 | 低 | 超时保护 + Prompt 引导分块处理 |
| Sidecar 进程内存增长 | 低 | 定期重启（已有健康检查机制） |
| 简单文档生成 token 消耗增加 | 中 | Helper 函数减少代码量；Prompt 引导简洁代码 |

### 6.4 迁移风险

| 风险 | 等级 | 缓解措施 |
|------|------|---------|
| Phase 1 和 Phase 2 之间功能不一致 | 低 | Phase 1 不修改现有功能，两套生成方式共存 |
| document_design.rs 精简后 LLM 仍尝试调用 generate/modify | 低 | Handler parameters 中 action enum 已移除，LLM 会收到错误并自动切换 |
| 用户习惯变化 | 中 | 更新 Prompt 和 UI 提示，引导用户理解新流程 |

---

## 七、测试策略

### 7.1 单元测试

| 测试项 | 位置 | 说明 |
|--------|------|------|
| 安全检查 | `sidecar/tests/test_code_handler.py` | 测试禁止模式检测 |
| 受限命名空间 | `sidecar/tests/test_code_handler.py` | 测试模块白名单/黑名单 |
| 文件系统隔离 | `sidecar/tests/test_code_handler.py` | 测试路径越界拦截 |
| 超时机制 | `sidecar/tests/test_code_handler.py` | 测试死循环检测 |
| Helper 函数 | `sidecar/tests/test_doc_helpers.py` | 测试各 Helper 输出正确性 |
| 精简后 Handler | `sidecar/tests/test_handlers.py` | 确保 read/convert/analyze 正常 |

### 7.2 集成测试

| 测试项 | 说明 |
|--------|------|
| Code Interpreter 完整链路 | LLM → code_interpreter_handler → Sidecar → 文件生成 |
| 错误重试 | 代码执行失败后 LLM 自动修复重试 |
| 确认流程 | 用户确认/拒绝 Code Interpreter 操作 |
| 精简后 Handler | read/convert/analyze 通过现有 Handler 正常工作 |
| 混合操作 | 同一会话中先读取文档，再用 Code Interpreter 修改 |

### 7.3 手工验证场景

1. 生成带 matplotlib 图表的 Word 报告
2. 从 CSV 读取数据生成 Excel 分析报告
3. 修改现有 Word 文档的标题和内容
4. 读取文档内容（验证精简后 read 正常）
5. 格式转换（验证精简后 convert 正常）
6. 代码语法错误时 LLM 自动修复
7. 安全检查拦截危险代码
8. 超时后 LLM 简化代码重试

---

## 八、方案对比总结

| 维度 | 方案 A（原设计：Sidecar 全保留 + CI） | 方案 B（全 CI） | 方案 C（推荐：精简 Sidecar + CI） |
|------|--------------------------------------|----------------|----------------------------------|
| 架构复杂度 | 高（两套完整系统） | 最低 | 中（Sidecar 精简 60%） |
| 简单操作效率 | 高 | 低（5-10x 开销） | 高 |
| 复杂操作灵活性 | 高 | 最高 | 最高 |
| 维护成本 | 高 | 低 | 中 |
| LLM 模式选择 | 困惑（5 种 action vs CI） | 无 | 清晰（3 种 action vs CI） |
| 迁移成本 | 低 | 高 | 中 |
| 文档质量保证 | 高（Sidecar 专业样式） | 低（依赖 LLM 代码质量） | 高（Helper 封装专业样式） |
| Prompt token 消耗 | 高（document_design.rs 完整保留） | 低 | 中（document_design.rs 精简 60-70%） |

---

## 九、未来扩展

### 9.1 短期（1-2 个月）

- **更多 Helper 函数**：根据实际使用反馈，提炼高频操作为 Helper
- **代码模板库**：预置常用文档生成代码模板，LLM 可直接引用
- **执行结果缓存**：相同代码不重复执行

### 9.2 中期（3-6 个月）

- **子进程沙箱**：独立子进程执行代码，避免主 Sidecar 崩溃
- **异步执行**：长时间代码执行支持异步等待 + 进度回调
- **代码审计日志**：记录所有代码执行历史，支持回溯
- **进一步精简 Sidecar**：评估 read/convert/analyze 是否也可以用 Code Interpreter 替代

### 9.3 长期（6 个月+）

- **容器沙箱**：接入 E2B/Cube Sandbox 等云沙箱方案，提供更强的隔离
- **多语言支持**：除了 Python，支持 R、JavaScript 等语言
- **可视化编辑**：代码执行结果的可视化预览和编辑
- **完全移除 Sidecar**：如果 Code Interpreter 的效率和可靠性达到足够水平
