"""PDF 中文字体注册工具

提供跨平台的 reportlab 中文字体注册功能，供各 handler 共享使用。
避免在多个 handler 中重复相同的字体搜索与注册逻辑。

主字体: 微软雅黑 (Microsoft YaHei)，Windows 系统自带
回退字体: 宋体、黑体、苹方、Noto Sans CJK 等
"""

import os
import logging

logger = logging.getLogger(__name__)

# 跨平台中文字体路径列表（按优先级排序）
_FONT_PATHS = [
    # Windows - 微软雅黑（主字体）
    ("MicrosoftYaHei", "C:/Windows/Fonts/msyh.ttc", 0),
    # Windows - 微软雅黑粗体
    ("MicrosoftYaHei-Bold", "C:/Windows/Fonts/msyhbd.ttc", 0),
    # Windows - 宋体
    ("ChineseFont", "C:/Windows/Fonts/simsun.ttc", 0),
    # Windows - 黑体
    ("ChineseFont", "C:/Windows/Fonts/simhei.ttf", 0),
    # macOS - 苹方
    ("ChineseFont", "/System/Library/Fonts/PingFang.ttc", 0),
    # macOS - 华文黑体
    ("ChineseFont", "/System/Library/Fonts/STHeiti Light.ttc", 0),
    # macOS - Arial Unicode
    ("ChineseFont", "/Library/Fonts/Arial Unicode.ttf", 0),
    # Linux - Noto Sans CJK
    ("ChineseFont", "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc", 0),
    ("ChineseFont", "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 0),
    # Linux - 文泉驿
    ("ChineseFont", "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc", 0),
    ("ChineseFont", "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc", 0),
    # Linux - DroidSans
    ("ChineseFont", "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf", 0),
]


def register_chinese_font() -> str:
    """注册 reportlab 中文字体，返回可用的字体名称

    按优先级尝试注册系统中的中文字体，优先注册微软雅黑 (Microsoft YaHei)。
    若均不可用则回退到 Helvetica。

    Returns:
        str: 已注册的字体名称，微软雅黑可用时为 "MicrosoftYaHei"，其他中文字体为 "ChineseFont"，否则为 "Helvetica"

    Raises:
        ImportError: reportlab 未安装时抛出
    """
    from reportlab.pdfbase import pdfmetrics
    from reportlab.pdfbase.ttfonts import TTFont

    font_name = "Helvetica"
    for name, fp, subfont_idx in _FONT_PATHS:
        if os.path.exists(fp):
            try:
                # TTC 字体需要指定 subfontIndex
                pdfmetrics.registerFont(TTFont(name, fp, subfontIndex=subfont_idx))
                # 第一个成功注册的字体作为返回值
                if font_name == "Helvetica":
                    font_name = name
                logger.debug("register_chinese_font: 成功注册字体 %s (%s)", name, fp)
            except Exception as e:
                logger.debug("register_chinese_font: 注册字体失败 %s (%s): %s", name, fp, e)
                continue

    # 全部字体注册失败时，记录警告而非静默回退
    # 返回 Helvetica 会导致 PDF 中文显示为方块，调用方应检查返回值并决定是否中止
    if font_name == "Helvetica":
        logger.warning("register_chinese_font: 所有中文字体注册失败，回退到 Helvetica（中文将显示为方块）")
    return font_name


def register_bold_font() -> str:
    """注册 reportlab 中文粗体字体

    Returns:
        str: 已注册的粗体字体名称，若不可用则返回空字符串
    """
    from reportlab.pdfbase import pdfmetrics
    from reportlab.pdfbase.ttfonts import TTFont

    bold_path = "C:/Windows/Fonts/msyhbd.ttc"
    if os.path.exists(bold_path):
        try:
            pdfmetrics.registerFont(TTFont("MicrosoftYaHei-Bold", bold_path, subfontIndex=0))
            return "MicrosoftYaHei-Bold"
        except Exception:
            pass
    return ""
