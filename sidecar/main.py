"""DocAgent Python Sidecar
文档处理引擎，通过 stdin/stdout JSON 协议与 Rust 后端通信
支持 Word、Excel、PPT、PDF、Markdown 等文档的生成、读取、修改、转换
"""

import sys
import os
import json
import logging
import traceback
from typing import Any

from handlers.word_handler import WordHandler
from handlers.excel_handler import ExcelHandler
from handlers.ppt_handler import PptHandler
from handlers.pdf_handler import PdfHandler
from handlers.markdown_handler import MarkdownHandler

logger = logging.getLogger(__name__)


def setup_logging():
    project_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    log_dir = os.path.join(project_root, "log")
    os.makedirs(log_dir, exist_ok=True)
    log_file = os.path.join(log_dir, "sidecar.log")

    formatter = logging.Formatter(
        fmt='%(asctime)s.%(msecs)03d [%(levelname)-5s] %(name)s - %(message)s',
        datefmt='%Y-%m-%d %H:%M:%S',
    )

    file_handler = logging.FileHandler(log_file, mode='w', encoding='utf-8')
    file_handler.setLevel(logging.DEBUG)
    file_handler.setFormatter(formatter)

    stderr_handler = logging.StreamHandler(sys.stderr)
    stderr_handler.setLevel(logging.DEBUG)
    stderr_handler.setFormatter(formatter)

    root_logger = logging.getLogger()
    root_logger.setLevel(logging.DEBUG)
    root_logger.addHandler(file_handler)
    root_logger.addHandler(stderr_handler)

    logger.info("Sidecar 日志系统初始化完成, 日志文件: %s", log_file)


# 文档处理器注册表
HANDLERS = {
    "docx": WordHandler(),
    "xlsx": ExcelHandler(),
    "pptx": PptHandler(),
    "pdf": PdfHandler(),
    "md": MarkdownHandler(),
    "markdown": MarkdownHandler(),
}


def handle_request(request: dict) -> dict:
    """处理文档操作请求

    请求格式:
    {
        "id": "请求唯一ID",
        "action": "generate|read|modify|delete|convert|analyze",
        "type": "docx|xlsx|pptx|pdf|md",
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

    handler = HANDLERS.get(doc_type)
    if handler is None:
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

    try:
        result = action_method(params)
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
    setup_logging()
    logger.info("Sidecar 启动, 等待输入...")

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
