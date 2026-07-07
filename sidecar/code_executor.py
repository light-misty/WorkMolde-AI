"""Code Executor Subprocess - 在独立子进程中安全执行 Python 代码

由 CodeHandler 通过 subprocess 调用，通过 stdin/stdout JSON 协议通信。
提供进程级隔离，避免代码执行导致主 Sidecar 进程崩溃。

输入格式 (stdin JSON):
{
    "code": "Python 代码字符串",
    "working_dir": "工作目录",
    "timeout": 60,
    "max_memory_mb": 512,
    "max_file_size_mb": 50,
    "max_output_bytes": 10000,
    "max_files": 20
}

输出格式 (stdout JSON):
{
    "success": true/false,
    "output": "stdout 输出",
    "files": ["生成的文件列表"],
    "error": "错误信息（如果有）",
    "memory_used_mb": 123.4,
    "duration_ms": 5678
}
"""

import os
import re
import sys
import json
import threading
import tracemalloc
import time

# 添加 sidecar 目录到 sys.path，以便导入 handlers.doc_helpers
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))


# ============================================================================
# 安全配置
# ============================================================================

# 允许的 Python 模块白名单
ALLOWED_MODULES = {
    # 文档处理库
    "docx", "openpyxl", "pptx", "reportlab", "fpdf",
    # PDF 读取/修改库（扩展：让智能体可操作现有 PDF 的所有元素）
    "fitz",          # PyMuPDF - 读取/修改 PDF（文字/绘图/图片/链接/书签/注释等）
    "pymupdf",       # PyMuPDF 新版导入名
    "pypdf",         # pypdf - 读取/合并/拆分/加密/修改 PDF
    "pdfplumber",    # pdfplumber - 表格/文本提取
    "pdfminer",      # pdfminer.six - 文本提取（含子模块 pdfminer.high_level 等）
    # 数据处理库
    "pandas", "numpy", "csv", "json", "math", "statistics",
    # 图表库
    "matplotlib", "plotly",
    # 图像处理库
    "PIL", "pillow",
    # 日期时间
    "datetime", "dateutil", "time",
    # 正则表达式
    "re",
    # 路径和系统操作（受限，safe_open 控制文件写入）
    "os", "os.path", "pathlib", "sys",
    # 类型相关
    "typing", "collections", "copy",
    # 编码
    "base64", "hashlib",
    # 随机数
    "random",
    # 文件系统常用模块（智能体修改 PDF/文档时常用，safe_open 已限制写入工作区）
    "shutil",        # 文件复制/移动/删除（常用模块，不应禁止）
    "tempfile",      # 临时文件创建（PyMuPDF 保存策略常用）
    "io",            # StringIO/BytesIO 等流操作
    "inspect",       # 对象检查（PyMuPDF/reportlab 等库内部可能使用）
    # Python 内置模块（智能体解析 TTC/字体文件结构时使用）
    "struct",        # 二进制数据打包/解包（解析 TTC 字体文件头）
    "glob",          # 文件名模式匹配（查找字体文件）
    # 项目内部 helper
    "doc_helpers",
    "handlers.font_utils",  # 中文字体注册工具
}

# 禁止的模块黑名单（即使白名单中未列出也做二次拦截）
# 仅保留真正危险的模块：网络通信、进程执行、序列化攻击、低级内存操作
BLOCKED_MODULES = {
    "subprocess", "socket", "http", "urllib",
    "signal", "ctypes", "multiprocessing",
    "webbrowser", "telnetlib", "ftplib", "smtplib",
    "xmlrpc", "pickle", "shelve", "marshal",
}

# 禁止的代码模式（正则表达式）
# 覆盖常见逃逸和危险操作，与 BLOCKED_MODULES 形成纵深防御
# 注意：os.remove/os.unlink/os.rmdir 已解禁，safe_open 限制写入工作区
#        智能体修改 PDF 时常用 os.remove 删除临时文件
BLOCKED_PATTERNS = [
    # 基础导入逃逸
    r'__import__\s*\(',          # 禁止直接调用 __import__
    r'subprocess\.',             # 禁止 subprocess 模块

    # os 模块危险函数（仅禁止进程执行和权限修改，不禁止文件删除）
    r'os\.system\s*\(',          # 禁止 os.system 执行 shell 命令
    r'os\.popen\s*\(',           # 禁止 os.popen 执行 shell 命令
    r'os\.exec[a-z]*\s*\(',      # 禁止 os.exec/l/execv/execve 等进程替换
    r'os\.spawn[a-z]*\s*\(',     # 禁止 os.spawnl/spawnv 等进程创建
    r'os\.chmod\s*\(',           # 禁止修改文件权限
    r'os\.chown\s*\(',           # 禁止修改文件所有者
    r'os\.fork\s*\(',            # 禁止进程 fork（Unix）
    r'os\.kill\s*\(',            # 禁止发送信号

    # sys 模块危险操作
    r'sys\.path\.\w+',           # 禁止修改 sys.path（插入恶意路径）
    r'globals\s*\(\s*\)',        # 禁止访问 globals() 获取命名空间
    r'locals\s*\(\s*\)',         # 禁止访问 locals()
    r'vars\s*\(\s*\)',           # 禁止访问 vars()
    r'__builtins__',             # 禁止直接访问 __builtins__ 字典
    r'__subclasses__',           # 禁止通过 __subclasses__ 逃逸

    # eval/exec/compile 直接调用
    r'\beval\s*\(',              # 禁止 eval 执行字符串代码
    r'\bexec\s*\(',              # 禁止 exec 执行字符串代码
    r'\bcompile\s*\(',           # 禁止 compile 编译代码对象

    # ctypes 逃逸（虽在 BLOCKED_MODULES，正则作为二次拦截）
    r'ctypes\.',
]


# ============================================================================
# 安全检查
# ============================================================================

def check_security(code: str) -> dict:
    """代码静态安全检查

    使用正则表达式模式匹配检查代码中的危险模式。

    注意：不再使用 RestrictedPython AST 级别分析，因为它会过度拦截
    python-docx/openpyxl 等库的合法内部属性访问（如 _tc、_tbl）。
    安全保障由以下层提供：
    - 正则表达式模式匹配（本函数）
    - 受限命名空间（白名单导入 + safe_open）
    - 执行超时
    - 资源限制

    Returns:
        {"safe": bool, "reason": str, "layer": str}
    """
    # 正则表达式模式匹配
    for pattern in BLOCKED_PATTERNS:
        match = re.search(pattern, code)
        if match:
            return {
                "safe": False,
                "reason": f"代码包含禁止的模式: {match.group()}",
                "layer": "regex",
            }

    return {"safe": True, "reason": "", "layer": "regex"}



# ============================================================================
# 受限命名空间构建
# ============================================================================

def build_namespace(working_dir: str) -> dict:
    """构建受限执行命名空间"""
    import builtins

    # 复制内建函数，移除危险项
    # 从 safe_builtins 中移除 exec/eval/compile，防止用户代码通过 __builtins__ 访问
    # 内部执行用户代码时直接使用 builtins.exec 引用（在 execute_with_timeout 中）
    safe_builtins = {k: v for k, v in builtins.__dict__.items()
                     if k not in ('__import__', 'breakpoint', 'exit', 'quit',
                                   'exec', 'eval', 'compile')}

    # 自定义安全导入函数
    def safe_import(name, *args, **kwargs):
        if name in BLOCKED_MODULES:
            raise ImportError(f"模块 '{name}' 被禁止导入")
        # 允许白名单中的模块
        if name in ALLOWED_MODULES or any(
            name.startswith(m + '.') for m in ALLOWED_MODULES
        ):
            return builtins.__import__(name, *args, **kwargs)
        raise ImportError(f"模块 '{name}' 不在允许列表中")

    safe_builtins['__import__'] = safe_import

    # 受限的 open() 函数：只允许写入工作区目录
    def safe_open(file, mode='r', *args, **kwargs):
        file_str = str(file)
        # 只允许读取操作和写入工作区目录
        if 'w' in mode or 'a' in mode:
            # 使用 os.path.realpath 解析符号链接，防止符号链接逃逸攻击
            # （用户可能先 os.symlink('/etc/passwd', './evil') 再 safe_open('./evil', 'w')）
            abs_path = os.path.realpath(os.path.abspath(file_str))
            real_working_dir = os.path.realpath(os.path.abspath(working_dir))

            # 使用 os.path.commonpath 做组件级路径校验
            # 避免 startswith 的前缀碰撞风险（/tmp/work vs /tmp/work-secret）
            try:
                common = os.path.commonpath([abs_path, real_working_dir])
                # Windows 路径不区分大小写，normcase 后比较
                if os.path.normcase(common) != os.path.normcase(real_working_dir):
                    raise PermissionError(f"只允许写入工作区目录: {working_dir}")
            except ValueError:
                # commonpath 在不同驱动器（Windows）或绝对/相对路径混合时抛出 ValueError
                raise PermissionError(f"路径不在工作区内: {file_str}")
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

    # 预导入 PDF 读取/修改库，让智能体可直接使用 fitz/PdfReader/PdfWriter/pdfplumber
    # 覆盖读取现有 PDF、修改 PDF、合并/拆分、加密、提取元素等场景
    try:
        import fitz  # PyMuPDF
        namespace['fitz'] = fitz
    except ImportError:
        pass

    try:
        import pypdf
        namespace['pypdf'] = pypdf
        # PdfReader/PdfWriter 是 pypdf 最常用的两个类，直接暴露到顶层
        from pypdf import PdfReader, PdfWriter
        namespace['PdfReader'] = PdfReader
        namespace['PdfWriter'] = PdfWriter
    except ImportError:
        pass

    try:
        import pdfplumber
        namespace['pdfplumber'] = pdfplumber
    except ImportError:
        pass

    # 导入项目 helper 函数
    try:
        import handlers.doc_helpers as doc_helpers
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

    # 预导入中文字体注册工具，避免智能体生成 PDF 时中文显示为方块
    try:
        from handlers.font_utils import register_chinese_font, register_bold_font
        namespace['register_chinese_font'] = register_chinese_font
        namespace['register_bold_font'] = register_bold_font
    except ImportError:
        pass

    # 预导入 fitz 专用字体注册工具，降低智能体在 fitz 场景下的编码难度
    # fitz 不能用 fontname 直接引用系统字体，必须通过 fontbuffer 传入 TTF 字节数据
    # 智能体可直接调用 register_fitz_font(page) 完成注册
    try:
        from handlers.font_utils import register_fitz_font, create_fitz_font
        namespace['register_fitz_font'] = register_fitz_font
        namespace['create_fitz_font'] = create_fitz_font
    except ImportError:
        pass

    # 预导入 ReportLab 常用常量和单位，避免智能体忘记导入导致 NameError
    # 常见错误：NameError: name 'TA_RIGHT' is not defined（智能体忘记 from reportlab.lib.enums import TA_RIGHT）
    # 注意：reportlab.lib.units 只有 inch/mm/cm/pica，没有 pt（智能体常误用 from reportlab.lib.units import pt）
    try:
        from reportlab.lib.enums import TA_LEFT, TA_CENTER, TA_RIGHT, TA_JUSTIFY
        namespace['TA_LEFT'] = TA_LEFT
        namespace['TA_CENTER'] = TA_CENTER
        namespace['TA_RIGHT'] = TA_RIGHT
        namespace['TA_JUSTIFY'] = TA_JUSTIFY
    except ImportError:
        pass

    try:
        from reportlab.lib.units import inch, mm, cm, pica
        namespace['inch'] = inch
        namespace['mm'] = mm
        namespace['cm'] = cm
        namespace['pica'] = pica
    except ImportError:
        pass

    return namespace


# ============================================================================
# 代码执行（带超时和资源限制）
# ============================================================================

def execute_with_timeout(
    code: str,
    namespace: dict,
    timeout: int,
    working_dir: str,
    max_memory_mb: int = 512,
    max_file_size_mb: int = 50,
    max_output_bytes: int = 10000,
    max_files: int = 20,
) -> dict:
    """带超时和资源限制的代码执行

    Args:
        code: Python 代码字符串
        namespace: 受限命名空间
        timeout: 执行超时时间（秒）
        working_dir: 工作目录
        max_memory_mb: 最大内存使用量（MB）
        max_file_size_mb: 单个文件最大大小（MB）
        max_output_bytes: 输出最大字节数
        max_files: 最大生成文件数

    Returns:
        执行结果字典
    """
    from io import StringIO

    # 捕获 stdout
    old_stdout = sys.stdout
    captured_output = StringIO()
    sys.stdout = captured_output

    # 启动内存追踪
    tracemalloc.start()

    result = {
        "success": False,
        "output": "",
        "files": [],
        "error": None,
        "memory_used_mb": 0.0,
        "duration_ms": 0,
    }

    # 执行前记录工作目录中的文件集合（用于追踪生成的文件）
    before_files = set()
    if working_dir and os.path.isdir(working_dir):
        for root, dirs, files in os.walk(working_dir):
            for f in files:
                before_files.add(os.path.join(root, f))

    start_time = time.time()

    try:
        exec_result = [None]
        exec_error = [None]
        memory_exceeded = [False]

        def run_code():
            try:
                # 使用 builtins.exec 直接引用，避免依赖命名空间中的 exec
                # （safe_builtins 已移除 exec/eval/compile）
                import builtins
                builtins.exec(code, namespace)
                exec_result[0] = True
            except MemoryError:
                exec_error[0] = MemoryError("代码执行超出内存限制")
            except Exception as e:
                exec_error[0] = e

        # 内存监控线程
        def memory_monitor():
            """定期检查内存使用量，超出限制时设置标志"""
            while not memory_exceeded[0]:
                try:
                    current, peak = tracemalloc.get_traced_memory()
                    if peak > max_memory_mb * 1024 * 1024:
                        memory_exceeded[0] = True
                        break
                except Exception:
                    break
                time.sleep(0.5)

        # 启动代码执行线程
        thread = threading.Thread(target=run_code)
        thread.daemon = True
        thread.start()

        # 启动内存监控线程
        monitor_thread = threading.Thread(target=memory_monitor)
        monitor_thread.daemon = True
        monitor_thread.start()

        # 等待执行完成或超时
        thread.join(timeout=timeout)

        duration_ms = int((time.time() - start_time) * 1000)
        result["duration_ms"] = duration_ms

        if thread.is_alive():
            # 执行超时
            result["error"] = f"代码执行超时（{timeout}秒）"
            result["output"] = captured_output.getvalue()[:max_output_bytes]
            return result

        if memory_exceeded[0]:
            # 内存超限
            result["error"] = f"代码执行超出内存限制（{max_memory_mb}MB）"
            result["output"] = captured_output.getvalue()[:max_output_bytes]
            return result

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

        # 检查生成文件数量限制
        if len(generated_files) > max_files:
            # 超出文件数量限制，删除超出的文件
            excess_files = generated_files[max_files:]
            for f in excess_files:
                try:
                    os.remove(f)
                except Exception:
                    pass
            generated_files = generated_files[:max_files]
            result["error"] = f"生成的文件数量超出限制（{max_files}个），已删除多余文件"
            result["output"] = captured_output.getvalue()[:max_output_bytes]
            result["files"] = generated_files
            return result

        # 检查单个文件大小限制
        oversized_files = []
        for f in generated_files:
            try:
                file_size = os.path.getsize(f)
                if file_size > max_file_size_mb * 1024 * 1024:
                    oversized_files.append(f)
            except Exception:
                pass

        if oversized_files:
            # 删除超大的文件
            for f in oversized_files:
                try:
                    os.remove(f)
                except Exception:
                    pass
                generated_files.remove(f)
            result["error"] = f"以下文件超出大小限制（{max_file_size_mb}MB）已被删除: {', '.join(os.path.basename(f) for f in oversized_files)}"
            result["output"] = captured_output.getvalue()[:max_output_bytes]
            result["files"] = generated_files
            return result

        # 获取内存使用峰值
        try:
            current, peak = tracemalloc.get_traced_memory()
            result["memory_used_mb"] = round(peak / (1024 * 1024), 2)
        except Exception:
            pass

        result["success"] = True
        result["output"] = captured_output.getvalue()[:max_output_bytes]
        result["files"] = generated_files

    except TimeoutError:
        result["error"] = f"代码执行超时（{timeout}秒）"
        result["output"] = captured_output.getvalue()[:max_output_bytes]
        result["duration_ms"] = int((time.time() - start_time) * 1000)
    except Exception as e:
        result["error"] = f"{type(e).__name__}: {e}"
        result["output"] = captured_output.getvalue()[:max_output_bytes]
        result["duration_ms"] = int((time.time() - start_time) * 1000)
    finally:
        # 停止内存追踪
        try:
            tracemalloc.stop()
        except Exception:
            pass
        sys.stdout = old_stdout

    return result


# ============================================================================
# 进程级资源限制（P3-5 + P3-6）
# ============================================================================

def _apply_resource_limits(max_memory_mb: int, timeout: int):
    """设置进程级资源限制

    在代码执行前调用，限制子进程的内存和 CPU 时间。
    - RLIMIT_AS: 限制进程虚拟内存大小，覆盖 C 扩展（numpy/matplotlib/PIL）的内存分配
      tracemalloc 只能追踪 Python 对象分配，无法限制 C 扩展内存
    - RLIMIT_CPU: 限制进程 CPU 时间（秒），防止用户代码长时间占用 CPU
      超出 CPU 限制会触发 SIGXCPU 信号（Unix）

    Windows 平台不支持 resource 模块，回退到 tracemalloc + wall time 方案。

    Args:
        max_memory_mb: 最大内存限制（MB）
        timeout: 执行超时（秒），CPU 时间限制设为 timeout + 10 秒缓冲
    """
    try:
        import resource

        # RLIMIT_AS: 限制进程虚拟内存
        # 设置软限制和硬限制相同，超过时 malloc/mmap 返回 NULL，Python 抛出 MemoryError
        # 额外增加 256MB 缓冲，避免 Python 解释器自身开销导致误触发
        memory_limit_bytes = (max_memory_mb + 256) * 1024 * 1024
        try:
            resource.setrlimit(resource.RLIMIT_AS, (memory_limit_bytes, memory_limit_bytes))
            log_info = f"RLIMIT_AS 设置为 {max_memory_mb + 256}MB"
        except (ValueError, OSError):
            log_info = "RLIMIT_AS 设置失败（可能不支持）"

        # RLIMIT_CPU: 限制 CPU 时间
        # 软限制触发 SIGXCPU 信号（可捕获），硬限制触发 SIGKILL
        # CPU 时间设为 timeout + 10 秒，避免 wall time 超时前 CPU 限制误触发
        cpu_limit = timeout + 10
        try:
            resource.setrlimit(resource.RLIMIT_CPU, (cpu_limit, cpu_limit))
            log_info += f", RLIMIT_CPU 设置为 {cpu_limit}秒"
        except (ValueError, OSError):
            log_info += ", RLIMIT_CPU 设置失败（可能不支持）"

        sys.stderr.write(f"[code_executor] 资源限制: {log_info}\n")
    except ImportError:
        # Windows 平台没有 resource 模块，回退到 tracemalloc + wall time
        sys.stderr.write("[code_executor] Windows 平台不支持 resource 模块，使用 tracemalloc 回退方案\n")
    except Exception as e:
        sys.stderr.write(f"[code_executor] 资源限制设置异常: {type(e).__name__}: {e}\n")


# ============================================================================
# 主入口
# ============================================================================

def main():
    """从 stdin 读取 JSON 请求，执行代码，返回 JSON 结果到 stdout"""
    # Windows 管道模式下确保 UTF-8 编码
    sys.stdin.reconfigure(encoding='utf-8')
    sys.stdout.reconfigure(encoding='utf-8')
    sys.stderr.reconfigure(encoding='utf-8')

    try:
        # 读取输入
        input_line = sys.stdin.readline().strip()
        # 移除 UTF-8 BOM
        input_line = input_line.lstrip('\ufeff')
        if not input_line:
            result = {"success": False, "error": "输入为空", "output": "", "files": [], "memory_used_mb": 0, "duration_ms": 0}
            sys.stdout.write(json.dumps(result, ensure_ascii=False) + "\n")
            sys.stdout.flush()
            return

        request = json.loads(input_line)

        code = request.get("code", "")
        working_dir = request.get("working_dir", "")
        timeout = request.get("timeout", 60)
        max_memory_mb = request.get("max_memory_mb", 512)
        max_file_size_mb = request.get("max_file_size_mb", 50)
        max_output_bytes = request.get("max_output_bytes", 10000)
        max_files = request.get("max_files", 20)

        if not code:
            result = {"success": False, "error": "缺少代码内容", "output": "", "files": [], "memory_used_mb": 0, "duration_ms": 0}
            sys.stdout.write(json.dumps(result, ensure_ascii=False) + "\n")
            sys.stdout.flush()
            return

        # 安全检查（子进程内的第二层检查）
        security_result = check_security(code)
        if not security_result["safe"]:
            result = {
                "success": False,
                "error": f"代码安全检查未通过: {security_result['reason']}",
                "output": "",
                "files": [],
                "memory_used_mb": 0,
                "duration_ms": 0,
            }
            sys.stdout.write(json.dumps(result, ensure_ascii=False) + "\n")
            sys.stdout.flush()
            return

        # 设置进程级资源限制（Unix 平台）
        # RLIMIT_AS: 限制进程虚拟内存，覆盖 C 扩展（numpy/matplotlib/PIL）的内存分配
        # RLIMIT_CPU: 限制 CPU 时间，防止用户代码长时间占用 CPU
        # Windows 平台不支持 resource 模块，回退到 tracemalloc + wall time
        _apply_resource_limits(max_memory_mb, timeout)

        # 构建受限命名空间
        namespace = build_namespace(working_dir)

        # 切换工作目录到 working_dir，确保 os.getcwd() 和相对路径正确
        # 这是对 code_handler.py 中 subprocess.run(cwd=working_dir) 的双重保障
        if working_dir and os.path.isdir(working_dir):
            try:
                os.chdir(working_dir)
            except Exception:
                pass

        # 执行代码
        result = execute_with_timeout(
            code=code,
            namespace=namespace,
            timeout=timeout,
            working_dir=working_dir,
            max_memory_mb=max_memory_mb,
            max_file_size_mb=max_file_size_mb,
            max_output_bytes=max_output_bytes,
            max_files=max_files,
        )

        # 输出结果
        sys.stdout.write(json.dumps(result, ensure_ascii=False) + "\n")
        sys.stdout.flush()

    except json.JSONDecodeError as e:
        result = {"success": False, "error": f"JSON 解析错误: {e}", "output": "", "files": [], "memory_used_mb": 0, "duration_ms": 0}
        sys.stdout.write(json.dumps(result, ensure_ascii=False) + "\n")
        sys.stdout.flush()
    except Exception as e:
        result = {"success": False, "error": f"内部错误: {type(e).__name__}: {e}", "output": "", "files": [], "memory_used_mb": 0, "duration_ms": 0}
        sys.stdout.write(json.dumps(result, ensure_ascii=False) + "\n")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
