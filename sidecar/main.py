"""DocAgent Python Sidecar
文档处理引擎，通过 stdin/stdout JSON 协议与 Rust 后端通信
支持 Word、Excel、PPT、PDF、Markdown 等文档的读取、转换、分析，以及代码执行
"""

import sys
import os
import json
import logging
import datetime
import traceback
from typing import Any

# Python Embeddable Distribution 的 python312._pth 文件会完全覆盖默认的 sys.path 计算，
# 导致脚本所在目录不会被自动加入 sys.path，handlers 等业务模块无法被 import。
# 这里显式将脚本所在目录插入 sys.path[0]，确保开发和生产环境行为一致。
# 注意：必须放在所有 handlers.* 导入之前执行。
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

logger = logging.getLogger(__name__)


def setup_logging():
    """配置日志系统

    日志目录优先级：
    1. 环境变量 DOCAGENT_LOG_DIR（由 Rust 端注入，生产环境使用）
    2. 项目根目录的 log/ 子目录（开发环境回退，基于 __file__ 推导）

    每次启动生成带启动时间戳的独立日志文件（sidecar_YYYYMMDD_HHMMSS.log）
    不覆盖历史日志，历史日志由 Rust 端统一清理（保留 7 天）
    """
    # 日志目录：优先读取 Rust 端注入的环境变量，回退到项目根目录推导
    log_dir = os.environ.get("DOCAGENT_LOG_DIR") or os.path.join(
        os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
        "log",
    )
    os.makedirs(log_dir, exist_ok=True)

    # 生成带启动时间戳的日志文件名，每次运行生成独立文件
    timestamp = datetime.datetime.now().strftime("%Y%m%d_%H%M%S")
    log_file = os.path.join(log_dir, "sidecar_{}.log".format(timestamp))

    formatter = logging.Formatter(
        fmt='%(asctime)s.%(msecs)03d [%(levelname)-5s] %(name)s - %(message)s',
        datefmt='%Y-%m-%d %H:%M:%S',
    )

    # mode='w' 新文件每次启动创建（文件名已含时间戳，不会覆盖历史日志）
    # 文件 handler 保留 DEBUG 级别，本地 sidecar.log 仍可详细调试
    file_handler = logging.FileHandler(log_file, mode='w', encoding='utf-8')
    file_handler.setLevel(logging.DEBUG)
    file_handler.setFormatter(formatter)

    # stderr handler 提升到 INFO 级别，避免 pdfminer/PIL/openpyxl 等第三方库的
    # DEBUG 日志通过 stderr 传输到 Rust 端被记录为 INFO，污染主日志
    stderr_handler = logging.StreamHandler(sys.stderr)
    stderr_handler.setLevel(logging.INFO)
    stderr_handler.setFormatter(formatter)

    # root logger 设为 INFO，默认过滤第三方库 DEBUG 日志
    # 应用自身的 handlers.* logger 显式设为 DEBUG，保持业务日志详细度
    root_logger = logging.getLogger()
    root_logger.setLevel(logging.INFO)
    root_logger.addHandler(file_handler)
    root_logger.addHandler(stderr_handler)

    # 应用自身 logger 保持 DEBUG 级别（业务逻辑详细日志）
    logging.getLogger("handlers").setLevel(logging.DEBUG)
    logging.getLogger(__name__).setLevel(logging.DEBUG)

    # 已知噪声第三方库强制提升到 WARNING 级别（即使 root 设为 INFO，这里显式声明意图）
    for noisy_logger in ("pdfminer", "PIL", "openpyxl", "pptx", "docx", "urllib3"):
        logging.getLogger(noisy_logger).setLevel(logging.WARNING)

    logger.info("Sidecar 日志系统初始化完成, 日志文件: %s", log_file)


# 文档处理器注册表
# 每个 handler 独立 try/except 导入，避免缺少某个第三方库导致整个 Sidecar 无法启动
# txt 类型复用 MarkdownHandler，纯文本是 Markdown 的子集
HANDLERS = {}
# 记录因依赖缺失而无法加载的处理器及其错误信息
MISSING_DEPS = {}

try:
    from handlers.word_handler import WordHandler
    HANDLERS["docx"] = WordHandler()
except ImportError as e:
    MISSING_DEPS["docx"] = f"python-docx ({e})"

try:
    from handlers.excel_handler import ExcelHandler
    HANDLERS["xlsx"] = ExcelHandler()
except ImportError as e:
    MISSING_DEPS["xlsx"] = f"openpyxl ({e})"

try:
    from handlers.ppt_handler import PptHandler
    HANDLERS["pptx"] = PptHandler()
except ImportError as e:
    MISSING_DEPS["pptx"] = f"python-pptx ({e})"

try:
    from handlers.pdf_handler import PdfHandler
    HANDLERS["pdf"] = PdfHandler()
except ImportError as e:
    MISSING_DEPS["pdf"] = f"PyMuPDF/pdfminer ({e})"

try:
    from handlers.markdown_handler import MarkdownHandler
    _md_handler = MarkdownHandler()
    HANDLERS["md"] = _md_handler
    HANDLERS["markdown"] = _md_handler
    HANDLERS["txt"] = _md_handler
except ImportError as e:
    for key in ("md", "markdown", "txt"):
        MISSING_DEPS[key] = f"({e})"

try:
    from handlers.code_handler import CodeHandler
    HANDLERS["code"] = CodeHandler()
except ImportError as e:
    MISSING_DEPS["code"] = f"({e})"

# 文档验证器实例
try:
    from handlers.validator import DocumentValidator
    _validator = DocumentValidator()
except ImportError as e:
    _validator = None


def handle_request(request: dict) -> dict:
    """处理文档操作请求

    请求格式:
    {
        "id": "请求唯一ID",
        "action": "read|convert|analyze|execute|validate|ping",
        "type": "docx|xlsx|pptx|pdf|md|health",
        "params": { ... }
    }

    响应格式:
    {
        "id": "请求唯一ID",
        "success": true|false,
        "data": { ... },   # 成功时
        "error": "..."      # 失败时
    }
    """
    request_id = request.get("id", "")
    action = request.get("action", "")
    doc_type = request.get("type", "")
    params = request.get("params", {})

    logger.info("收到请求: id=%s, action=%s, type=%s", request_id, action, doc_type)

    # 健康检查请求，直接返回成功响应
    if action == "ping" or doc_type == "health":
        logger.debug("健康检查请求: id=%s", request_id)
        return {
            "id": request_id,
            "success": True,
            "data": {"status": "ok"},
        }

    # 验证请求，使用 DocumentValidator
    if action == "validate":
        logger.info("验证请求: id=%s, type=%s", request_id, doc_type)
        if _validator is None:
            return {
                "id": request_id,
                "success": False,
                "error": "验证器不可用（缺少依赖）",
            }
        file_path = params.get("path", "")
        if "input_path" in params and "path" not in params:
            file_path = params["input_path"]
        options = params.get("options", {})
        try:
            result = _validator.validate(file_path, doc_type, options)
            return {
                "id": request_id,
                "success": True,
                "data": result,
            }
        except Exception as e:
            logger.error("验证失败: id=%s, error=%s: %s", request_id, type(e).__name__, e)
            return {
                "id": request_id,
                "success": False,
                "error": f"验证失败: {type(e).__name__}: {e}",
            }

    handler = HANDLERS.get(doc_type)
    if handler is None:
        # 检查是否是因为依赖缺失导致处理器不可用
        if doc_type in MISSING_DEPS:
            logger.error("处理器 %s 不可用（缺少依赖: %s）", doc_type, MISSING_DEPS[doc_type])
            return {
                "id": request_id,
                "success": False,
                "error": f"文档类型 {doc_type} 的处理器不可用，缺少依赖: {MISSING_DEPS[doc_type]}。请运行: pip install -r sidecar/requirements.txt",
            }
        logger.error("不支持的文档类型: %s", doc_type)
        return {
            "id": request_id,
            "success": False,
            "error": f"不支持的文档类型: {doc_type}",
        }

    action_method = getattr(handler, action, None)
    if action_method is None:
        logger.error("不支持的操作: %s/%s", action, doc_type)
        return {
            "id": request_id,
            "success": False,
            "error": f"不支持的操作: {action}/{doc_type}",
        }

    # 将 Rust 端发送的 input_path 映射为 Python handler 期望的 path
    # 适用于所有需要文件路径的操作（read、convert、analyze 等）
    if "input_path" in params and "path" not in params:
        params["path"] = params["input_path"]

    try:
        result = action_method(params)
        # 检查结果中是否包含错误信息（handler 参数校验失败时返回含 error 键的字典而非抛出异常）
        # 注意：code_executor 等处理器成功时也包含 "error": None，需排除
        if isinstance(result, dict) and result.get("error") is not None:
            logger.warning("操作返回错误: id=%s, error=%s", request_id, result["error"])
            return {
                "id": request_id,
                "success": False,
                "error": result["error"],
            }
        logger.info("操作执行成功: id=%s, action=%s/%s", request_id, action, doc_type)
        return {
            "id": request_id,
            "success": True,
            "data": result,
        }
    except FileNotFoundError as e:
        logger.error("文件未找到: id=%s, error=%s", request_id, e)
        return {
            "id": request_id,
            "success": False,
            "error": f"文件未找到: {e}",
        }
    except PermissionError as e:
        logger.error("权限不足: id=%s, error=%s", request_id, e)
        return {
            "id": request_id,
            "success": False,
            "error": f"权限不足: {e}",
        }
    except Exception as e:
        logger.error("操作执行失败: id=%s, error=%s: %s", request_id, type(e).__name__, e)
        return {
            "id": request_id,
            "success": False,
            "error": f"{type(e).__name__}: {e}",
            "traceback": traceback.format_exc(),
        }


def main():
    """主循环：从 stdin 读取 JSON 请求，处理并输出到 stdout"""
    # Windows 管道模式下 stdin/stdout 默认使用系统编码（如 GBK/cp936），
    # 而 Rust 端发送 UTF-8 编码的 JSON，编码不匹配会导致 surrogate 字符产生，
    # 引发 UnicodeEncodeError。显式重新配置为 UTF-8 解决此问题。
    sys.stdin.reconfigure(encoding='utf-8')
    sys.stdout.reconfigure(encoding='utf-8')
    sys.stderr.reconfigure(encoding='utf-8')

    setup_logging()
    logger.info("Sidecar 启动, 等待输入...")
    # 输出处理器加载状态
    if HANDLERS:
        logger.info("已加载的处理器: %s", list(HANDLERS.keys()))
    if MISSING_DEPS:
        logger.warning("缺少依赖的处理器: %s", {k: v for k, v in MISSING_DEPS.items()})

    for line in sys.stdin:
        line = line.strip()
        # 移除 UTF-8 BOM（Windows 管道常见问题）
        line = line.lstrip('\ufeff')
        if not line:
            continue
        logger.debug("收到输入: %s", line[:200])
        try:
            request = json.loads(line)
            response = handle_request(request)
        except json.JSONDecodeError as e:
            logger.error("JSON 解析错误: %s", e)
            response = {"id": "", "success": False, "error": f"JSON 解析错误: {e}"}
        except Exception as e:
            logger.error("内部错误: %s: %s", type(e).__name__, e)
            response = {
                "id": "",
                "success": False,
                "error": f"内部错误: {e}",
                "traceback": traceback.format_exc(),
            }

        sys.stdout.write(json.dumps(response, ensure_ascii=False) + "\n")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
