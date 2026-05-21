"""PDF 中文字体注册工具

提供跨平台的 reportlab 中文字体注册功能，供各 handler 共享使用。
避免在多个 handler 中重复相同的字体搜索与注册逻辑。
"""

import os
import logging

logger = logging.getLogger(__name__)

# 跨平台中文字体路径列表（按优先级排序）
_FONT_PATHS = [
    # Windows
    "C:/Windows/Fonts/msyh.ttc",
    "C:/Windows/Fonts/simsun.ttc",
    "C:/Windows/Fonts/simhei.ttf",
    # macOS
    "/System/Library/Fonts/PingFang.ttc",
    "/System/Library/Fonts/STHeiti Light.ttc",
    "/Library/Fonts/Arial Unicode.ttf",
    # Linux
    "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
    "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
    "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
]


def register_chinese_font() -> str:
    """注册 reportlab 中文字体，返回可用的字体名称

    按优先级尝试注册系统中的中文字体，若均不可用则回退到 Helvetica。

    Returns:
        str: 已注册的字体名称，中文字体可用时为 "ChineseFont"，否则为 "Helvetica"

    Raises:
        ImportError: reportlab 未安装时抛出
    """
    from reportlab.pdfbase import pdfmetrics
    from reportlab.pdfbase.ttfonts import TTFont

    font_name = "Helvetica"
    for fp in _FONT_PATHS:
        if os.path.exists(fp):
            try:
                pdfmetrics.registerFont(TTFont("ChineseFont", fp))
                font_name = "ChineseFont"
                break
            except Exception:
                continue
    return font_name
