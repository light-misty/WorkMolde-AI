"""PDF 文档处理器
基于 reportlab + pypdf 实现 PDF 文档的生成、读取、修改、转换
遵循 pdf Skill 规范：Platypus 框架、下标上标 XML 标签、中文字体注册、合并/拆分/旋转/水印/加密
增强功能：Markdown 内容解析、专业配色样式、行内格式（粗体/斜体/代码）
"""

import os
import re
import json
import html
import io
import logging
from typing import Any, Optional

from reportlab.lib.pagesizes import A4, letter
from reportlab.lib.styles import getSampleStyleSheet, ParagraphStyle
from reportlab.lib.units import cm, inch
from reportlab.lib import colors
from reportlab.lib.enums import TA_LEFT, TA_CENTER, TA_JUSTIFY
from reportlab.platypus import (
    SimpleDocTemplate,
    Paragraph,
    Spacer,
    PageBreak,
    Table,
    TableStyle,
    HRFlowable,
)


class PdfHandler:
    """PDF 文档处理器"""

    logger = logging.getLogger(__name__)

    # 页面尺寸映射
    PAGE_SIZES = {
        "a4": A4,
        "letter": letter,
    }

    # 专业配色方案
    COLOR_HEADING1 = colors.HexColor("#1F4E79")  # 深蓝
    COLOR_HEADING2 = colors.HexColor("#2E75B6")  # 中蓝
    COLOR_HEADING3 = colors.HexColor("#5B9BD5")  # 浅蓝
    COLOR_TABLE_HEADER_BG = colors.HexColor("#2E75B6")  # 表头背景蓝
    COLOR_TABLE_HEADER_TEXT = colors.white  # 表头文字白色
    COLOR_TABLE_ALT_ROW = colors.HexColor("#D6E4F0")  # 交替行浅蓝
    COLOR_CODE_BG = colors.HexColor("#F5F5F5")  # 代码块背景灰
    COLOR_CODE_BORDER = colors.HexColor("#DDDDDD")  # 代码块边框
    COLOR_HR = colors.HexColor("#CCCCCC")  # 分割线颜色

    def generate(self, params: dict) -> dict:
        """生成 PDF 文档

        遵循 Skill 规范：
        - 使用 reportlab Platypus 框架（SimpleDocTemplate + Paragraph + Spacer + PageBreak）
        - 下标上标使用 <sub>/<super> XML 标签，不使用 Unicode 字符
        - 页面尺寸设置（letter / A4）
        - 中文字体注册（使用 font_utils.register_chinese_font()）
        - 支持 Markdown 内容自动解析为专业 PDF 元素

        params:
            path: 输出文件路径
            content: 文档内容（纯文本、Markdown 或结构化 JSON）
                    结构化格式: {"blocks": [{type, ...}]}
                    block 类型: heading/paragraph/table/list/spacer/pagebreak
            title: 文档标题
            author: 作者
            pageSize: 页面尺寸 "letter" | "a4"（默认 "a4"）
            margins: 边距 {"top", "right", "bottom", "left"}（单位: cm，默认 2.54cm）
            headerText: 页眉文本（可选）
            footerText: 页脚文本（可选）
            pageNumber: 是否显示页码（默认 true）
        """
        path = params.get("path", "")
        content = params.get("content", "")
        title = params.get("title", "")
        author = params.get("author", "")
        page_size_name = params.get("pageSize", "a4")
        margins = params.get("margins", {})
        header_text = params.get("headerText", None)
        footer_text = params.get("footerText", None)
        page_number = params.get("pageNumber", True)

        if not path:
            self.logger.error("generate: 缺少输出文件路径")
            return {"error": "缺少输出文件路径"}

        self.logger.info("generate: 开始生成 PDF 文档, path=%s", path)

        os.makedirs(os.path.dirname(path) or ".", exist_ok=True)

        # 注册中文字体
        from handlers.font_utils import register_chinese_font
        font_name = register_chinese_font()

        # 页面尺寸
        page_size = self.PAGE_SIZES.get(page_size_name.lower(), A4)

        # 边距（默认 2.54cm = 1 inch）
        margin_top = margins.get("top", 2.54) * cm
        margin_right = margins.get("right", 2.54) * cm
        margin_bottom = margins.get("bottom", 2.54) * cm
        margin_left = margins.get("left", 2.54) * cm

        # 创建文档
        doc = SimpleDocTemplate(
            path,
            pagesize=page_size,
            topMargin=margin_top,
            rightMargin=margin_right,
            bottomMargin=margin_bottom,
            leftMargin=margin_left,
        )

        # 设置文档元数据
        if title:
            doc.title = title
        if author:
            doc.author = author

        # 构建样式
        styles = getSampleStyleSheet()
        custom_styles = self._build_custom_styles(styles, font_name)

        # 构建内容元素
        elements = []

        # 添加标题
        if title:
            elements.append(Paragraph(self._escape_xml(title), custom_styles["title"]))
            elements.append(Spacer(1, 0.5 * cm))

        # 处理内容
        if isinstance(content, str):
            parsed_blocks = self._try_parse_json_content(content)
            if parsed_blocks is not None:
                self._process_structured_blocks(elements, parsed_blocks, custom_styles)
            elif self._looks_like_markdown(content):
                # Markdown 内容，解析为专业 PDF 元素
                self._process_markdown_content(elements, content, custom_styles)
            else:
                # 纯文本内容，按段落分割（支持行内格式）
                for line in content.split("\n"):
                    if line.strip():
                        formatted = self._parse_inline_formatting(line)
                        elements.append(Paragraph(formatted, custom_styles["body"]))
                        elements.append(Spacer(1, 0.3 * cm))
        elif isinstance(content, list):
            self._process_structured_blocks(elements, content, custom_styles)
        elif isinstance(content, dict):
            blocks = content.get("blocks", [])
            self._process_structured_blocks(elements, blocks, custom_styles)

        # 构建文档（含页眉页脚回调）
        def on_page(canvas, doc):
            """每页回调：绘制页眉和页脚"""
            canvas.saveState()
            page_w, page_h = page_size

            # 页眉
            if header_text:
                canvas.setFont(font_name, 9)
                canvas.setFillColor(colors.grey)
                canvas.drawString(margin_left, page_h - margin_top + 0.5 * cm, header_text)

            # 页脚（含页码）
            if footer_text or page_number:
                footer_parts = []
                if footer_text:
                    footer_parts.append(footer_text)
                if page_number:
                    footer_parts.append(str(canvas.getPageNumber()))
                footer_str = " - ".join(footer_parts) if footer_parts else ""
                if footer_str:
                    canvas.setFont(font_name, 9)
                    canvas.setFillColor(colors.grey)
                    canvas.drawCentredString(page_w / 2, margin_bottom - 0.8 * cm, footer_str)

            canvas.restoreState()

        doc.build(elements, onFirstPage=on_page, onLaterPages=on_page)

        self.logger.info("generate: PDF 文档已生成, path=%s", path)
        return {
            "path": path,
            "message": f"PDF 文档已生成: {path}",
        }

    def read(self, params: dict) -> dict:
        """读取 PDF 文档内容

        params:
            path: 文件路径
            pages: 要读取的页码列表（可选，默认读取所有）
        """
        path = params.get("path", "")
        pages = params.get("pages", None)
        if not path:
            self.logger.error("read: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("read: 开始读取 PDF 文档, path=%s", path)

        try:
            import pdfplumber
        except ImportError:
            self.logger.error("read: pdfplumber 未安装，无法读取 PDF")
            return {"error": "pdfplumber 未安装"}

        text_content = []
        with pdfplumber.open(path) as pdf:
            total_pages = len(pdf.pages)
            page_indices = range(total_pages)
            if pages:
                page_indices = [p - 1 for p in pages if 1 <= p <= total_pages]

            for idx in page_indices:
                page = pdf.pages[idx]
                page_text = page.extract_text() or ""
                text_content.append({
                    "page": idx + 1,
                    "text": page_text,
                })

        self.logger.info("read: PDF 文档读取完成, path=%s, 总页数=%d", path, len(text_content))
        return {
            "pages": text_content,
            "total_pages": len(text_content),
        }

    def modify(self, params: dict) -> dict:
        """修改 PDF 文档

        遵循 Skill 规范：
        - 合并 PDF（pypdf PdfWriter.add_page）
        - 拆分 PDF（按页码范围）
        - 旋转页面（page.rotate）
        - 添加水印（文字/图片，使用 reportlab+pypdf 叠加）
        - 加密（writer.encrypt）

        params:
            path: 源文件路径
            output_path: 输出文件路径（可选，默认覆盖源文件）
            operations: 操作列表
                - merge: 合并 PDF {type:"merge", files:["path1.pdf","path2.pdf"]}
                - split: 拆分 PDF {type:"split", ranges:[[1,3],[4,6]]}
                - rotate: 旋转页面 {type:"rotate", pages:[1,2], angle:90}
                - watermark: 添加水印 {type:"watermark", text:"DRAFT", fontSize:60,
                              color:"CCCCCC", opacity:0.3, angle:45}
                - watermark_image: 图片水印 {type:"watermark_image", image:"logo.png",
                              opacity:0.3, x:100, y:100}
                - encrypt: 加密 {type:"encrypt", password:"123456"}
        """
        path = params.get("path", "")
        output_path = params.get("output_path", "")
        operations = params.get("operations", [])

        if not path:
            self.logger.error("modify: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("modify: 开始修改 PDF 文档, path=%s, 操作数=%d", path, len(operations))

        from pypdf import PdfReader, PdfWriter

        modified_count = 0

        for op in operations:
            op_type = op.get("type", "")

            if op_type == "merge":
                # 合并 PDF
                files = op.get("files", [])
                if not files:
                    self.logger.warning("modify: merge 操作缺少 files 参数")
                    continue

                out = output_path or path
                os.makedirs(os.path.dirname(out) or ".", exist_ok=True)

                writer = PdfWriter()
                # 先添加源文件
                reader = PdfReader(path)
                for page in reader.pages:
                    writer.add_page(page)
                # 再添加其他文件
                for f in files:
                    if os.path.exists(f):
                        r = PdfReader(f)
                        for page in r.pages:
                            writer.add_page(page)
                        modified_count += 1
                    else:
                        self.logger.warning("modify: 合并文件不存在: %s", f)

                with open(out, "wb") as f:
                    writer.write(f)
                self.logger.info("modify: 合并完成, 输出=%s, 合并文件数=%d", out, modified_count)

            elif op_type == "split":
                # 拆分 PDF
                ranges = op.get("ranges", [])
                if not ranges:
                    self.logger.warning("modify: split 操作缺少 ranges 参数")
                    continue

                reader = PdfReader(path)
                total_pages = len(reader.pages)
                out_dir = os.path.dirname(output_path) if output_path else os.path.dirname(path)
                base_name = os.path.splitext(os.path.basename(path))[0]

                for rng in ranges:
                    start = rng[0] - 1 if len(rng) > 0 else 0
                    end = rng[1] if len(rng) > 1 else start + 1
                    start = max(0, start)
                    end = min(end, total_pages)

                    writer = PdfWriter()
                    for i in range(start, end):
                        writer.add_page(reader.pages[i])

                    split_path = os.path.join(out_dir or ".", f"{base_name}_p{start+1}-{end}.pdf")
                    with open(split_path, "wb") as f:
                        writer.write(f)
                    modified_count += 1
                    self.logger.info("modify: 拆分完成, 输出=%s, 页码范围=%d-%d", split_path, start+1, end)

            elif op_type == "rotate":
                # 旋转页面
                pages_to_rotate = op.get("pages", [])
                angle = op.get("angle", 90)
                out = output_path or path

                reader = PdfReader(path)
                writer = PdfWriter()
                for i, page in enumerate(reader.pages):
                    if (i + 1) in pages_to_rotate or not pages_to_rotate:
                        page.rotate(angle)
                    writer.add_page(page)

                with open(out, "wb") as f:
                    writer.write(f)
                modified_count += len(pages_to_rotate) if pages_to_rotate else len(reader.pages)
                self.logger.info("modify: 旋转完成, 输出=%s, 旋转页数=%d", out, modified_count)

            elif op_type == "watermark":
                # 添加文字水印
                text = op.get("text", "DRAFT")
                font_size = op.get("fontSize", 60)
                color_hex = op.get("color", "CCCCCC")
                opacity = op.get("opacity", 0.3)
                angle = op.get("angle", 45)
                out = output_path or path

                self._add_text_watermark(path, out, text, font_size, color_hex, opacity, angle)
                modified_count += 1
                self.logger.info("modify: 水印添加完成, 输出=%s, 文本=%s", out, text)

            elif op_type == "watermark_image":
                # 添加图片水印
                image_path = op.get("image", "")
                opacity = op.get("opacity", 0.3)
                x = op.get("x", None)
                y = op.get("y", None)
                out = output_path or path

                if not image_path or not os.path.exists(image_path):
                    self.logger.warning("modify: 水印图片不存在: %s", image_path)
                    continue

                self._add_image_watermark(path, out, image_path, opacity, x, y)
                modified_count += 1
                self.logger.info("modify: 图片水印添加完成, 输出=%s", out)

            elif op_type == "encrypt":
                # 加密 PDF
                password = op.get("password", "")
                if not password:
                    self.logger.warning("modify: encrypt 操作缺少 password 参数")
                    continue
                out = output_path or path

                reader = PdfReader(path)
                writer = PdfWriter()
                for page in reader.pages:
                    writer.add_page(page)
                writer.encrypt(password)

                with open(out, "wb") as f:
                    writer.write(f)
                modified_count += 1
                self.logger.info("modify: 加密完成, 输出=%s", out)

        self.logger.info("modify: PDF 文档修改完成, path=%s, 修改数=%d", path, modified_count)
        return {
            "path": output_path or path,
            "modified_count": modified_count,
            "message": f"已执行 {modified_count} 项修改",
        }

    def convert(self, params: dict) -> dict:
        """格式转换

        params:
            path: 源文件路径
            output_path: 输出文件路径（可选）
            format: 目标格式（txt, md, html）
        """
        path = params.get("path", "")
        output_path = params.get("output_path", "")
        target_format = params.get("format", "txt")

        if not path:
            self.logger.error("convert: 缺少源文件路径")
            return {"error": "缺少源文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("convert: 开始格式转换, path=%s, format=%s", path, target_format)

        # 读取 PDF 内容
        try:
            import pdfplumber
        except ImportError:
            self.logger.error("convert: pdfplumber 未安装，无法读取 PDF")
            return {"error": "pdfplumber 未安装"}

        pages_text = []
        with pdfplumber.open(path) as pdf:
            for page in pdf.pages:
                text = page.extract_text() or ""
                pages_text.append(text)

        # 转换为目标格式
        if target_format == "txt":
            content = "\n\n".join(pages_text)
        elif target_format in ("md", "markdown"):
            parts = []
            for i, text in enumerate(pages_text):
                parts.append(f"## 第 {i + 1} 页\n")
                parts.append(text)
                parts.append("")
            content = "\n".join(parts)
        elif target_format == "html":
            content = self._convert_pages_to_html(pages_text)
        else:
            self.logger.error("convert: 不支持的目标格式: %s", target_format)
            return {"error": f"不支持的目标格式: {target_format}"}

        if output_path:
            os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)
            with open(output_path, "w", encoding="utf-8") as f:
                f.write(content)
            self.logger.info("convert: 格式转换完成, output_path=%s, format=%s", output_path, target_format)
            return {
                "path": output_path,
                "format": target_format,
                "message": f"已转换为 {target_format} 格式",
            }
        else:
            return {
                "content": content,
                "format": target_format,
            }

    def analyze(self, params: dict) -> dict:
        """分析 PDF 文档"""
        path = params.get("path", "")
        if not path:
            self.logger.error("analyze: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("analyze: 开始分析 PDF 文档, path=%s", path)

        from pypdf import PdfReader

        reader = PdfReader(path)
        total_pages = len(reader.pages)

        # 提取元数据
        meta = reader.metadata
        metadata = {}
        if meta:
            metadata = {
                "title": meta.title or "",
                "author": meta.author or "",
                "subject": meta.subject or "",
                "creator": meta.creator or "",
                "producer": meta.producer or "",
            }

        # 统计文本
        total_chars = 0
        try:
            import pdfplumber
            with pdfplumber.open(path) as pdf:
                for page in pdf.pages:
                    text = page.extract_text() or ""
                    total_chars += len(text)
        except ImportError:
            self.logger.warning("analyze: pdfplumber 未安装，跳过文本统计")

        self.logger.info("analyze: PDF 文档分析完成, path=%s, 总页数=%d", path, total_pages)
        return {
            "file_size": os.path.getsize(path),
            "total_pages": total_pages,
            "total_chars": total_chars,
            "metadata": metadata,
        }

    # ------------------------------------------------------------------ #
    #  样式构建
    # ------------------------------------------------------------------ #

    def _build_custom_styles(self, styles, font_name: str) -> dict:
        """构建自定义样式集

        专业配色方案:
        - 标题1: 深蓝 (#1F4E79), 20pt
        - 标题2: 中蓝 (#2E75B6), 16pt
        - 标题3: 浅蓝 (#5B9BD5), 14pt
        - 正文: 12pt, 两端对齐, 1.5 倍行距
        - 代码块: 等宽字体, 灰色背景

        Args:
            styles: reportlab 样式集
            font_name: 已注册的中文字体名称

        Returns:
            样式字典 {name: ParagraphStyle}
        """
        custom = {}

        # 标题样式
        custom["title"] = ParagraphStyle(
            "CustomTitle",
            parent=styles["Title"],
            fontName=font_name,
            fontSize=24,
            spaceAfter=15,
            alignment=TA_CENTER,
            textColor=self.COLOR_HEADING1,
        )

        # 一级标题: 深蓝 20pt
        custom["heading1"] = ParagraphStyle(
            "CustomHeading1",
            parent=styles["Heading1"],
            fontName=font_name,
            fontSize=20,
            spaceBefore=20,
            spaceAfter=10,
            textColor=self.COLOR_HEADING1,
        )

        # 二级标题: 中蓝 16pt
        custom["heading2"] = ParagraphStyle(
            "CustomHeading2",
            parent=styles["Heading2"],
            fontName=font_name,
            fontSize=16,
            spaceBefore=15,
            spaceAfter=8,
            textColor=self.COLOR_HEADING2,
        )

        # 三级标题: 浅蓝 14pt
        custom["heading3"] = ParagraphStyle(
            "CustomHeading3",
            parent=styles["Heading3"],
            fontName=font_name,
            fontSize=14,
            spaceBefore=12,
            spaceAfter=6,
            textColor=self.COLOR_HEADING3,
        )

        # 正文样式: 12pt, 两端对齐, 1.5 倍行距
        custom["body"] = ParagraphStyle(
            "CustomBody",
            parent=styles["Normal"],
            fontName=font_name,
            fontSize=12,
            leading=18,
            spaceAfter=8,
            alignment=TA_JUSTIFY,
        )

        # 列表样式
        custom["list"] = ParagraphStyle(
            "CustomList",
            parent=styles["Normal"],
            fontName=font_name,
            fontSize=12,
            leading=18,
            spaceAfter=4,
            leftIndent=20,
            bulletIndent=10,
        )

        # 有序列表样式
        custom["ordered_list"] = ParagraphStyle(
            "CustomOrderedList",
            parent=styles["Normal"],
            fontName=font_name,
            fontSize=12,
            leading=18,
            spaceAfter=4,
            leftIndent=20,
            bulletIndent=10,
        )

        # 代码块样式: 等宽字体, 灰色背景
        custom["code_block"] = ParagraphStyle(
            "CustomCodeBlock",
            parent=styles["Normal"],
            fontName="Courier",
            fontSize=10,
            leading=14,
            spaceAfter=6,
            spaceBefore=6,
            leftIndent=15,
            rightIndent=15,
            backColor=self.COLOR_CODE_BG,
            borderColor=self.COLOR_CODE_BORDER,
            borderWidth=0.5,
            borderPadding=6,
        )

        # 行内代码样式
        custom["inline_code"] = ParagraphStyle(
            "CustomInlineCode",
            parent=styles["Normal"],
            fontName="Courier",
            fontSize=10,
        )

        # 注释样式
        custom["note"] = ParagraphStyle(
            "CustomNote",
            parent=styles["Normal"],
            fontName=font_name,
            fontSize=10,
            leading=16,
            spaceAfter=4,
            textColor=colors.grey,
        )

        return custom

    # ------------------------------------------------------------------ #
    #  结构化内容处理
    # ------------------------------------------------------------------ #

    def _try_parse_json_content(self, content: str):
        """尝试将字符串内容解析为结构化 JSON

        Returns:
            解析后的 blocks 列表，或 None（非 JSON 格式）
        """
        if not content or not content.strip():
            return None
        try:
            parsed = json.loads(content)
            if isinstance(parsed, dict):
                return parsed.get("blocks", [])
            elif isinstance(parsed, list):
                return parsed
            return None
        except (json.JSONDecodeError, TypeError):
            return None

    def _process_structured_blocks(self, elements: list, blocks: list, styles: dict):
        """处理结构化内容块列表

        支持的 block 类型:
        - heading: 标题 {type:"heading", level:1, text:"..."}
        - paragraph: 段落 {type:"paragraph", text:"..."}
        - table: 表格 {type:"table", headers:[], rows:[]}
        - list: 列表 {type:"list", items:[]}
        - spacer: 间距 {type:"spacer", height:0.5}
        - pagebreak: 分页 {type:"pagebreak"}
        """
        for block in blocks:
            if not isinstance(block, dict):
                continue
            self._add_block(elements, block, styles)

    def _add_block(self, elements: list, block: dict, styles: dict):
        """添加单个内容块"""
        block_type = block.get("type", "paragraph")

        if block_type == "heading":
            level = block.get("level", 1)
            text = block.get("text", "")
            # 支持 1-3 级标题，超过 3 级使用 heading3 样式
            style_key = f"heading{level}" if level <= 3 else "heading3"
            style = styles.get(style_key, styles["heading1"])
            # 标题文本支持行内格式
            formatted = self._parse_inline_formatting(text)
            elements.append(Paragraph(formatted, style))
            elements.append(Spacer(1, 0.3 * cm))

        elif block_type == "paragraph":
            text = block.get("text", "")
            # 段落文本支持行内格式
            formatted = self._parse_inline_formatting(text)
            elements.append(Paragraph(formatted, styles["body"]))
            elements.append(Spacer(1, 0.2 * cm))

        elif block_type == "table":
            headers = block.get("headers", [])
            rows = block.get("rows", [])
            self._add_table_block(elements, headers, rows, styles)

        elif block_type == "list":
            items = block.get("items", [])
            ordered = block.get("ordered", False)
            for idx, item in enumerate(items):
                item_text = str(item)
                formatted = self._parse_inline_formatting(item_text)
                if ordered:
                    bullet_text = f"{idx + 1}. {formatted}"
                else:
                    bullet_text = f"&bull; {formatted}"
                style = styles.get("ordered_list" if ordered else "list", styles["list"])
                elements.append(Paragraph(bullet_text, style))
            elements.append(Spacer(1, 0.2 * cm))

        elif block_type == "spacer":
            height = block.get("height", 0.5)
            elements.append(Spacer(1, height * cm))

        elif block_type == "pagebreak":
            elements.append(PageBreak())

    def _add_table_block(self, elements: list, headers: list, rows: list, styles: dict):
        """添加表格块（专业配色样式）

        样式规范:
        - 表头: 蓝色背景, 白色粗体文字
        - 数据行: 交替浅蓝背景
        - 边框: 灰色细线
        """
        table_data = []
        if headers:
            # 表头单元格使用 Paragraph 以支持行内格式
            header_row = [Paragraph(self._parse_inline_formatting(str(h)), styles["body"]) for h in headers]
            table_data.append(header_row)
        for row in rows:
            # 数据行也使用 Paragraph 以支持行内格式
            data_row = [Paragraph(self._parse_inline_formatting(str(cell)), styles["body"]) for cell in row]
            table_data.append(data_row)

        if not table_data:
            return

        # 获取字体名称
        font_name = styles["body"].fontName

        table = Table(table_data)
        style_commands = [
            ("GRID", (0, 0), (-1, -1), 0.5, colors.HexColor("#CCCCCC")),
            ("FONTNAME", (0, 0), (-1, -1), font_name),
            ("FONTSIZE", (0, 0), (-1, -1), 10),
            ("ALIGN", (0, 0), (-1, -1), "CENTER"),
            ("VALIGN", (0, 0), (-1, -1), "MIDDLE"),
            ("TOPPADDING", (0, 0), (-1, -1), 6),
            ("BOTTOMPADDING", (0, 0), (-1, -1), 6),
            ("LEFTPADDING", (0, 0), (-1, -1), 8),
            ("RIGHTPADDING", (0, 0), (-1, -1), 8),
        ]

        # 表头行样式: 蓝色背景, 白色粗体文字
        if headers:
            style_commands.extend([
                ("BACKGROUND", (0, 0), (-1, 0), self.COLOR_TABLE_HEADER_BG),
                ("TEXTCOLOR", (0, 0), (-1, 0), self.COLOR_TABLE_HEADER_TEXT),
                ("FONTNAME", (0, 0), (-1, 0), font_name),
                ("FONTSIZE", (0, 0), (-1, 0), 11),
                ("BOLD", (0, 0), (-1, 0), True),
            ])

        # 数据行交替颜色
        data_start = 1 if headers else 0
        for i in range(data_start, len(table_data)):
            row_idx = i - data_start  # 数据行索引（从 0 开始）
            if row_idx % 2 == 1:
                # 奇数行浅蓝背景
                style_commands.append(("BACKGROUND", (0, i), (-1, i), self.COLOR_TABLE_ALT_ROW))

        table.setStyle(TableStyle(style_commands))
        elements.append(table)
        elements.append(Spacer(1, 0.5 * cm))

    # ------------------------------------------------------------------ #
    #  Markdown 解析
    # ------------------------------------------------------------------ #

    def _looks_like_markdown(self, content: str) -> bool:
        """判断内容是否看起来像 Markdown 格式

        检测规则:
        - 包含标题标记 (# 开头)
        - 包含粗体标记 (**text**)
        - 包含斜体标记 (*text*)
        - 包含无序列表标记 (- 或 * 开头)
        - 包含有序列表标记 (1. 开头)
        - 包含代码块标记 (```)
        - 包含表格标记 (| 分隔)
        - 包含分割线标记 (---)

        Args:
            content: 文本内容

        Returns:
            True 如果内容包含 Markdown 特征
        """
        if not content or not content.strip():
            return False

        # 统计 Markdown 特征出现次数
        md_features = 0

        # 标题: # 开头（行首 1-6 个 # 后跟空格）
        if re.search(r"^#{1,6}\s+\S", content, re.MULTILINE):
            md_features += 2

        # 粗体: **text** 或 __text__
        if re.search(r"\*\*.+?\*\*", content) or re.search(r"__.+?__", content):
            md_features += 1

        # 斜体: *text* (排除粗体和列表标记)
        if re.search(r"(?<!\*)\*(?!\*).+?(?<!\*)\*(?!\*)", content):
            md_features += 1

        # 无序列表: - 或 * 开头后跟空格
        if re.search(r"^[\-\*]\s+\S", content, re.MULTILINE):
            md_features += 1

        # 有序列表: 数字. 开头
        if re.search(r"^\d+\.\s+\S", content, re.MULTILINE):
            md_features += 1

        # 代码块: ```
        if "```" in content:
            md_features += 2

        # 表格: | 分隔的行
        if re.search(r"^\|.+\|$", content, re.MULTILINE):
            md_features += 2

        # 分割线: --- 或 ***
        if re.search(r"^(-{3,}|\*{3,}|_{3,})\s*$", content, re.MULTILINE):
            md_features += 1

        # 至少 2 个特征才认为是 Markdown
        return md_features >= 2

    def _process_markdown_content(self, elements: list, content: str, styles: dict):
        """将 Markdown 内容解析为 PDF 元素

        支持的 Markdown 语法:
        - # / ## / ### 标题
        - **bold** 粗体
        - *italic* 斜体
        - `code` 行内代码
        - - item 无序列表
        - 1. item 有序列表
        - | table | 表格
        - ``` 代码块
        - --- 分割线

        Args:
            elements: PDF 元素列表（输出）
            content: Markdown 文本内容
            styles: 样式字典
        """
        lines = content.split("\n")
        i = 0

        while i < len(lines):
            line = lines[i]
            stripped = line.strip()

            # 空行跳过
            if not stripped:
                i += 1
                continue

            # 代码块: ``` 开始
            if stripped.startswith("```"):
                code_lines = []
                # 跳过 ``` 行本身（可能带有语言标识）
                i += 1
                while i < len(lines) and not lines[i].strip().startswith("```"):
                    code_lines.append(lines[i])
                    i += 1
                # 跳过结束的 ```
                i += 1
                # 生成代码块元素
                if code_lines:
                    code_text = self._escape_xml("\n".join(code_lines))
                    code_text = code_text.replace("<br/>", "\n")
                    # 使用换行符分隔代码行
                    code_parts = code_text.split("\n")
                    for cp in code_parts:
                        elements.append(Paragraph(cp if cp.strip() else "&nbsp;", styles["code_block"]))
                    elements.append(Spacer(1, 0.3 * cm))
                continue

            # 标题: # / ## / ### 等
            heading_match = re.match(r"^(#{1,6})\s+(.+)$", stripped)
            if heading_match:
                level = len(heading_match.group(1))
                text = heading_match.group(2).strip()
                # 限制标题级别到 1-3
                level = min(level, 3)
                style_key = f"heading{level}"
                style = styles.get(style_key, styles["heading1"])
                formatted = self._parse_inline_formatting(text)
                elements.append(Paragraph(formatted, style))
                elements.append(Spacer(1, 0.3 * cm))
                i += 1
                continue

            # 分割线: --- / *** / ___
            if re.match(r"^(-{3,}|\*{3,}|_{3,})\s*$", stripped):
                elements.append(Spacer(1, 0.3 * cm))
                elements.append(HRFlowable(
                    width="100%",
                    thickness=1,
                    color=self.COLOR_HR,
                    spaceAfter=0.3 * cm,
                    spaceBefore=0,
                ))
                i += 1
                continue

            # 表格: | 分隔的行
            if stripped.startswith("|") and stripped.endswith("|"):
                table_lines = []
                while i < len(lines) and lines[i].strip().startswith("|") and lines[i].strip().endswith("|"):
                    table_lines.append(lines[i].strip())
                    i += 1
                self._add_markdown_table(elements, table_lines, styles)
                continue

            # 无序列表: - 或 * 开头
            if re.match(r"^[\-\*]\s+\S", stripped):
                list_items = []
                while i < len(lines):
                    l = lines[i].strip()
                    if re.match(r"^[\-\*]\s+\S", l):
                        # 提取列表项文本（去掉 - 或 * 前缀）
                        item_text = re.sub(r"^[\-\*]\s+", "", l)
                        list_items.append(item_text)
                        i += 1
                    else:
                        break
                for item in list_items:
                    formatted = self._parse_inline_formatting(item)
                    elements.append(Paragraph(f"&bull; {formatted}", styles["list"]))
                elements.append(Spacer(1, 0.2 * cm))
                continue

            # 有序列表: 数字. 开头
            if re.match(r"^\d+\.\s+\S", stripped):
                list_items = []
                while i < len(lines):
                    l = lines[i].strip()
                    if re.match(r"^\d+\.\s+\S", l):
                        # 提取列表项文本（去掉数字. 前缀）
                        item_text = re.sub(r"^\d+\.\s+", "", l)
                        list_items.append(item_text)
                        i += 1
                    else:
                        break
                for idx, item in enumerate(list_items):
                    formatted = self._parse_inline_formatting(item)
                    elements.append(Paragraph(f"{idx + 1}. {formatted}", styles["ordered_list"]))
                elements.append(Spacer(1, 0.2 * cm))
                continue

            # 普通段落
            formatted = self._parse_inline_formatting(stripped)
            elements.append(Paragraph(formatted, styles["body"]))
            elements.append(Spacer(1, 0.2 * cm))
            i += 1

    def _add_markdown_table(self, elements: list, table_lines: list, styles: dict):
        """解析 Markdown 表格并添加为 PDF 表格元素

        Markdown 表格格式:
        | Header1 | Header2 |
        | ------- | ------- |
        | Cell1   | Cell2   |

        Args:
            elements: PDF 元素列表（输出）
            table_lines: Markdown 表格行列表
            styles: 样式字典
        """
        if not table_lines:
            return

        # 解析表格行
        parsed_rows = []
        separator_idx = -1

        for idx, line in enumerate(table_lines):
            # 检测分隔行 (| --- | --- |)
            if re.match(r"^\|[\s\-:]+\|$", line):
                separator_idx = idx
                continue

            # 解析单元格
            cells = [c.strip() for c in line.strip("|").split("|")]
            parsed_rows.append(cells)

        if not parsed_rows:
            return

        # 第一行作为表头（如果存在分隔行）
        headers = []
        data_rows = []
        if separator_idx >= 0:
            # 分隔行之前的行为表头
            headers = parsed_rows[0] if parsed_rows else []
            data_rows = parsed_rows[1:]
        else:
            # 没有分隔行，所有行作为数据
            data_rows = parsed_rows

        # 使用已有的表格方法
        self._add_table_block(elements, headers, data_rows, styles)

    # ------------------------------------------------------------------ #
    #  行内格式解析
    # ------------------------------------------------------------------ #

    def _parse_inline_formatting(self, text: str) -> str:
        """解析行内 Markdown 格式并转换为 reportlab XML 标记

        支持的格式:
        - **bold** -> <b>bold</b>
        - *italic* -> <i>italic</i>
        - `code` -> <font face="Courier" size="10">code</font>
        - ~~strikethrough~~ -> <strike>strikethrough</strike>

        处理顺序: 先处理代码（避免内部被解析），再处理粗体，最后处理斜体

        Args:
            text: 原始文本（可能包含 Markdown 行内格式）

        Returns:
            转换后的 reportlab XML 标记文本
        """
        if not text:
            return ""

        # 先转义 XML 特殊字符（保留 sub/super 标签）
        text = self._escape_xml(text)

        # 保护代码块内容（避免内部被其他规则处理）
        # 使用 \x00 作为占位符分隔符，避免与 Markdown 语法（__、**、~~等）冲突
        code_placeholders = []

        def protect_code(match):
            code_content = match.group(1)
            # 代码内容已经被 _escape_xml 转义过了
            placeholder = f"\x00CODEPH{len(code_placeholders)}\x00"
            code_placeholders.append(
                f'<font face="Courier" size="10" color="#C7254E">{code_content}</font>'
            )
            return placeholder

        # 行内代码: `code`
        text = re.sub(r"`([^`]+)`", protect_code, text)

        # 粗体: **text** 或 __text__
        text = re.sub(r"\*\*(.+?)\*\*", r"<b>\1</b>", text)
        text = re.sub(r"__(.+?)__", r"<b>\1</b>", text)

        # 斜体: *text* (排除已被粗体标记消耗的情况)
        text = re.sub(r"(?<!\*)\*(?!\*)(.+?)(?<!\*)\*(?!\*)", r"<i>\1</i>", text)
        text = re.sub(r"(?<!_)_(?!_)(.+?)(?<!_)_(?!_)", r"<i>\1</i>", text)

        # 删除线: ~~text~~
        text = re.sub(r"~~(.+?)~~", r"<strike>\1</strike>", text)

        # 恢复代码占位符
        for i, code_markup in enumerate(code_placeholders):
            text = text.replace(f"\x00CODEPH{i}\x00", code_markup)

        return text

    # ------------------------------------------------------------------ #
    #  XML 转义辅助
    # ------------------------------------------------------------------ #

    def _escape_xml(self, text: str) -> str:
        """转义 XML 特殊字符，同时保留 <sub>/<super> 标签

        遵循 Skill 规范：下标上标使用 <sub>/<super> XML 标签，不使用 Unicode 字符

        Args:
            text: 原始文本

        Returns:
            转义后的文本（保留 sub/super 标签）
        """
        # 先保护 sub/super 标签
        protected_tags = []

        def protect_tag(match):
            protected_tags.append(match.group(0))
            return f"__PROTECTED_{len(protected_tags) - 1}__"

        text = re.sub(r"<(sub|super)>(.*?)</\1>", protect_tag, text, flags=re.IGNORECASE | re.DOTALL)

        # 转义 XML 特殊字符
        text = html.escape(text)

        # 恢复 sub/super 标签
        for i, tag in enumerate(protected_tags):
            text = text.replace(f"__PROTECTED_{i}__", tag)

        # 处理换行符（reportlab Paragraph 使用 <br/> 换行）
        text = text.replace("\n", "<br/>")

        return text

    # ------------------------------------------------------------------ #
    #  水印相关方法
    # ------------------------------------------------------------------ #

    def _add_text_watermark(self, input_path: str, output_path: str,
                            text: str, font_size: int = 60,
                            color_hex: str = "CCCCCC", opacity: float = 0.3,
                            angle: int = 45):
        """添加文字水印

        使用 reportlab 生成水印页，再用 pypdf 叠加到原文

        Args:
            input_path: 源 PDF 路径
            output_path: 输出 PDF 路径
            text: 水印文字
            font_size: 字号
            color_hex: 颜色（十六进制）
            opacity: 透明度
            angle: 旋转角度
        """
        from pypdf import PdfReader, PdfWriter
        from handlers.font_utils import register_chinese_font

        font_name = register_chinese_font()

        # 读取源 PDF
        reader = PdfReader(input_path)
        page_width = float(reader.pages[0].mediabox.width)
        page_height = float(reader.pages[0].mediabox.height)

        # 用 reportlab 生成水印页
        watermark_buffer = io.BytesIO()
        from reportlab.pdfgen import canvas as pdfcanvas
        c = pdfcanvas.Canvas(watermark_buffer, pagesize=(page_width, page_height))

        # 解析颜色
        r = int(color_hex[0:2], 16) / 255.0
        g = int(color_hex[2:4], 16) / 255.0
        b = int(color_hex[4:6], 16) / 255.0

        c.setFillRGB(r, g, b, alpha=opacity)
        c.setFont(font_name, font_size)

        # 居中旋转绘制
        c.translate(page_width / 2, page_height / 2)
        c.rotate(angle)
        c.drawCentredString(0, 0, text)
        c.save()

        watermark_buffer.seek(0)
        watermark_reader = PdfReader(watermark_buffer)
        watermark_page = watermark_reader.pages[0]

        # 叠加水印到每一页
        writer = PdfWriter()
        for page in reader.pages:
            page.merge_page(watermark_page)
            writer.add_page(page)

        os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)
        with open(output_path, "wb") as f:
            writer.write(f)

    def _add_image_watermark(self, input_path: str, output_path: str,
                             image_path: str, opacity: float = 0.3,
                             x: Optional[float] = None, y: Optional[float] = None):
        """添加图片水印

        使用 reportlab 生成带图片的水印页，再用 pypdf 叠加到原文

        Args:
            input_path: 源 PDF 路径
            output_path: 输出 PDF 路径
            image_path: 水印图片路径
            opacity: 透明度
            x: 图片 x 坐标（默认居中）
            y: 图片 y 坐标（默认居中）
        """
        from pypdf import PdfReader, PdfWriter

        reader = PdfReader(input_path)
        page_width = float(reader.pages[0].mediabox.width)
        page_height = float(reader.pages[0].mediabox.height)

        # 用 reportlab 生成水印页
        watermark_buffer = io.BytesIO()
        from reportlab.pdfgen import canvas as pdfcanvas
        c = pdfcanvas.Canvas(watermark_buffer, pagesize=(page_width, page_height))

        # 计算图片位置
        if x is None:
            x = page_width / 2 - 100
        if y is None:
            y = page_height / 2 - 100

        c.setFillAlpha(opacity)
        c.drawImage(image_path, x, y, width=200, height=200, preserveAspectRatio=True, mask="auto")
        c.save()

        watermark_buffer.seek(0)
        watermark_reader = PdfReader(watermark_buffer)
        watermark_page = watermark_reader.pages[0]

        # 叠加水印到每一页
        writer = PdfWriter()
        for page in reader.pages:
            page.merge_page(watermark_page)
            writer.add_page(page)

        os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)
        with open(output_path, "wb") as f:
            writer.write(f)

    # ------------------------------------------------------------------ #
    #  格式转换辅助方法
    # ------------------------------------------------------------------ #

    def _convert_pages_to_html(self, pages_text: list[str]) -> str:
        """将 PDF 页面文本转换为 HTML"""
        sections = []
        for i, text in enumerate(pages_text):
            section_lines = [f'  <div class="page">']
            section_lines.append(f"    <h3>第 {i + 1} 页</h3>")
            for line in text.split("\n"):
                if line.strip():
                    section_lines.append(f"    <p>{html.escape(line)}</p>")
            section_lines.append("  </div>")
            sections.append("\n".join(section_lines))

        html_doc = f"""<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>PDF Content</title>
  <style>
    body {{ font-family: "Microsoft YaHei", "SimSun", sans-serif; max-width: 800px; margin: 0 auto; padding: 20px; }}
    .page {{ margin-bottom: 30px; padding: 15px; border: 1px solid #ddd; border-radius: 4px; }}
    h3 {{ color: #666; border-bottom: 1px solid #eee; padding-bottom: 5px; }}
    p {{ line-height: 1.8; color: #333; margin: 4px 0; }}
  </style>
</head>
<body>
{chr(10).join(sections)}
</body>
</html>"""
        return html_doc
