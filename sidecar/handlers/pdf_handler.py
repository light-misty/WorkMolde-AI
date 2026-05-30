"""PDF 文档处理器
基于 reportlab + pypdf 实现 PDF 文档的生成、读取、修改、转换
遵循 pdf Skill 规范：Platypus 框架、下标上标 XML 标签、中文字体注册、合并/拆分/旋转/水印/加密
"""

import os
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
)
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont


class PdfHandler:
    """PDF 文档处理器"""

    logger = logging.getLogger(__name__)

    # 页面尺寸映射
    PAGE_SIZES = {
        "a4": A4,
        "letter": letter,
    }

    def generate(self, params: dict) -> dict:
        """生成 PDF 文档

        遵循 Skill 规范：
        - 使用 reportlab Platypus 框架（SimpleDocTemplate + Paragraph + Spacer + PageBreak）
        - 下标上标使用 <sub>/<super> XML 标签，不使用 Unicode 字符
        - 页面尺寸设置（letter / A4）
        - 中文字体注册（使用 font_utils.register_chinese_font()）

        params:
            path: 输出文件路径
            content: 文档内容（纯文本或结构化 JSON）
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
            else:
                # 纯文本内容，按段落分割
                for line in content.split("\n"):
                    if line.strip():
                        elements.append(Paragraph(self._escape_xml(line), custom_styles["body"]))
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
        )

        # 一级标题
        custom["heading1"] = ParagraphStyle(
            "CustomHeading1",
            parent=styles["Heading1"],
            fontName=font_name,
            fontSize=18,
            spaceBefore=20,
            spaceAfter=10,
        )

        # 二级标题
        custom["heading2"] = ParagraphStyle(
            "CustomHeading2",
            parent=styles["Heading2"],
            fontName=font_name,
            fontSize=14,
            spaceBefore=15,
            spaceAfter=8,
        )

        # 正文样式
        custom["body"] = ParagraphStyle(
            "CustomBody",
            parent=styles["Normal"],
            fontName=font_name,
            fontSize=12,
            leading=20,
            spaceAfter=8,
            alignment=TA_JUSTIFY,
        )

        # 列表样式
        custom["list"] = ParagraphStyle(
            "CustomList",
            parent=styles["Normal"],
            fontName=font_name,
            fontSize=12,
            leading=20,
            spaceAfter=4,
            leftIndent=20,
            bulletIndent=10,
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
            style_key = f"heading{level}" if level <= 2 else "heading2"
            style = styles.get(style_key, styles["heading1"])
            elements.append(Paragraph(self._escape_xml(text), style))
            elements.append(Spacer(1, 0.3 * cm))

        elif block_type == "paragraph":
            text = block.get("text", "")
            elements.append(Paragraph(self._escape_xml(text), styles["body"]))
            elements.append(Spacer(1, 0.2 * cm))

        elif block_type == "table":
            headers = block.get("headers", [])
            rows = block.get("rows", [])
            self._add_table_block(elements, headers, rows, styles)

        elif block_type == "list":
            items = block.get("items", [])
            for item in items:
                bullet_text = f"&bull; {self._escape_xml(str(item))}"
                elements.append(Paragraph(bullet_text, styles["list"]))
            elements.append(Spacer(1, 0.2 * cm))

        elif block_type == "spacer":
            height = block.get("height", 0.5)
            elements.append(Spacer(1, height * cm))

        elif block_type == "pagebreak":
            elements.append(PageBreak())

    def _add_table_block(self, elements: list, headers: list, rows: list, styles: dict):
        """添加表格块"""
        table_data = []
        if headers:
            table_data.append(headers)
        for row in rows:
            table_data.append([str(cell) for cell in row])

        if not table_data:
            return

        # 获取字体名称
        font_name = styles["body"].fontName

        table = Table(table_data)
        style_commands = [
            ("GRID", (0, 0), (-1, -1), 0.5, colors.grey),
            ("FONTNAME", (0, 0), (-1, -1), font_name),
            ("FONTSIZE", (0, 0), (-1, -1), 10),
            ("ALIGN", (0, 0), (-1, -1), "CENTER"),
            ("VALIGN", (0, 0), (-1, -1), "MIDDLE"),
        ]

        # 表头行样式
        if headers:
            style_commands.extend([
                ("BACKGROUND", (0, 0), (-1, 0), colors.Color(0.9, 0.9, 0.9)),
                ("FONTNAME", (0, 0), (-1, 0), font_name),
                ("FONTSIZE", (0, 0), (-1, 0), 11),
                ("BOLD", (0, 0), (-1, 0), True),
            ])

        table.setStyle(TableStyle(style_commands))
        elements.append(table)
        elements.append(Spacer(1, 0.5 * cm))

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
        import re
        # 匹配 <sub>...</sub> 和 <super>...</super>
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
