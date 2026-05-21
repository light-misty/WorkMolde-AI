"""Word 文档处理器
基于 python-docx 实现 Word 文档的生成、读取、修改、转换
"""

import os
import html
import logging
from typing import Any

from docx import Document
from docx.shared import Inches, Pt, Cm, RGBColor
from docx.enum.text import WD_ALIGN_PARAGRAPH
from docx.enum.table import WD_TABLE_ALIGNMENT


class WordHandler:
    """Word (.docx) 文档处理器"""

    logger = logging.getLogger(__name__)

    def generate(self, params: dict) -> dict:
        """生成 Word 文档

        params:
            path: 输出文件路径
            title: 文档标题
            content: 文档内容（结构化或纯文本）
            author: 作者
            template: 模板路径（可选）
        """
        path = params.get("path", "")
        title = params.get("title", "")
        content = params.get("content", "")
        author = params.get("author", "")

        if not path:
            self.logger.error("generate: 缺少输出文件路径")
            return {"error": "缺少输出文件路径"}

        self.logger.info("generate: 开始生成 Word 文档, path=%s", path)

        # 确保输出目录存在
        os.makedirs(os.path.dirname(path) or ".", exist_ok=True)

        doc = Document()

        # 设置文档属性
        if author:
            doc.core_properties.author = author
        if title:
            doc.core_properties.title = title

        # 添加标题
        if title:
            heading = doc.add_heading(title, level=0)

        # 处理内容
        if isinstance(content, str):
            # 纯文本内容，按段落分割
            for paragraph_text in content.split("\n"):
                if paragraph_text.strip():
                    doc.add_paragraph(paragraph_text)
        elif isinstance(content, list):
            # 结构化内容
            for item in content:
                self._add_content_block(doc, item)

        doc.save(path)
        self.logger.info("generate: Word 文档已生成, path=%s", path)
        return {
            "path": path,
            "message": f"Word 文档已生成: {path}",
        }

    def read(self, params: dict) -> dict:
        """读取 Word 文档内容

        params:
            path: 文件路径
            include_formatting: 是否包含格式信息（可选，默认 false）
        """
        path = params.get("path", "")
        if not path:
            self.logger.error("read: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("read: 开始读取 Word 文档, path=%s", path)

        doc = Document(path)

        # 提取段落文本
        paragraphs = []
        for para in doc.paragraphs:
            para_info = {
                "text": para.text,
                "style": para.style.name if para.style else None,
            }
            paragraphs.append(para_info)

        # 提取表格
        tables = []
        for table in doc.tables:
            table_data = []
            for row in table.rows:
                row_data = [cell.text for cell in row.cells]
                table_data.append(row_data)
            tables.append(table_data)

        # 文档属性
        props = {
            "title": doc.core_properties.title or "",
            "author": doc.core_properties.author or "",
            "created": str(doc.core_properties.created) if doc.core_properties.created else "",
            "modified": str(doc.core_properties.modified) if doc.core_properties.modified else "",
        }

        self.logger.info("read: Word 文档读取完成, path=%s, 段落数=%d, 表格数=%d", path, len(paragraphs), len(tables))
        return {
            "paragraphs": paragraphs,
            "tables": tables,
            "properties": props,
            "paragraph_count": len(paragraphs),
            "table_count": len(tables),
        }

    def modify(self, params: dict) -> dict:
        """修改 Word 文档

        params:
            path: 文件路径
            operations: 修改操作列表
                [{"type": "replace", "old": "...", "new": "..."},  # 全文搜索替换
                 {"type": "replace", "index": 1, "text": "..."},   # 按段落索引替换整段
                 {"type": "add_paragraph", "text": "...", "position": 0},
                 {"type": "add_table", "rows": 3, "cols": 2, "data": [[...]]}]
        """
        path = params.get("path", "")
        operations = params.get("operations", [])
        if not path:
            self.logger.error("modify: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("modify: 开始修改 Word 文档, path=%s, 操作数=%d", path, len(operations))

        doc = Document(path)
        modified_count = 0

        for op in operations:
            op_type = op.get("type", "")

            if op_type == "replace":
                # 检查是按索引替换还是全文搜索替换
                if "index" in op:
                    # 按段落索引替换整段内容
                    index = op.get("index", 0)
                    new_text = op.get("text", "")
                    if 0 <= index < len(doc.paragraphs):
                        para = doc.paragraphs[index]
                        # 清空原有内容并设置新文本
                        para.clear()
                        para.add_run(new_text)
                        modified_count += 1
                        self.logger.debug("modify: 按索引替换段落 %d, 新文本: %s", index, new_text[:50])
                    else:
                        self.logger.warning("modify: 段落索引 %d 超出范围 (0-%d)", index, len(doc.paragraphs) - 1)
                else:
                    # 全文搜索替换
                    old_text = op.get("old", "")
                    new_text = op.get("new", "")
                    for para in doc.paragraphs:
                        if old_text in para.text:
                            for run in para.runs:
                                if old_text in run.text:
                                    run.text = run.text.replace(old_text, new_text)
                                    modified_count += 1

            elif op_type == "add_paragraph":
                # 添加段落
                text = op.get("text", "")
                style = op.get("style", None)
                p = doc.add_paragraph(text)
                if style:
                    p.style = style
                modified_count += 1

            elif op_type == "add_heading":
                # 添加标题
                text = op.get("text", "")
                level = op.get("level", 1)
                doc.add_heading(text, level=level)
                modified_count += 1

            elif op_type == "add_table":
                # 添加表格
                rows = op.get("rows", 1)
                cols = op.get("cols", 1)
                data = op.get("data", [])
                table = doc.add_table(rows=rows, cols=cols)
                table.style = "Table Grid"
                for i, row_data in enumerate(data):
                    if i < rows:
                        for j, cell_text in enumerate(row_data):
                            if j < cols:
                                table.rows[i].cells[j].text = str(cell_text)
                modified_count += 1

        doc.save(path)
        self.logger.info("modify: Word 文档修改完成, path=%s, 修改数=%d", path, modified_count)
        return {
            "path": path,
            "modified_count": modified_count,
            "message": f"已执行 {modified_count} 项修改",
        }

    def convert(self, params: dict) -> dict:
        """格式转换

        params:
            path: 源文件路径
            output_path: 输出文件路径
            format: 目标格式（md, txt）
        """
        path = params.get("path", "")
        output_path = params.get("output_path", "")
        target_format = params.get("format", "md")
        if not path:
            self.logger.error("convert: 缺少源文件路径")
            return {"error": "缺少源文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("convert: 开始格式转换, path=%s, format=%s", path, target_format)

        doc = Document(path)

        if target_format in ("md", "markdown"):
            # 转换为 Markdown
            lines = []
            for para in doc.paragraphs:
                style = para.style.name if para.style else ""
                text = para.text
                if not text.strip():
                    continue
                if "Heading 1" in style:
                    lines.append(f"# {text}")
                elif "Heading 2" in style:
                    lines.append(f"## {text}")
                elif "Heading 3" in style:
                    lines.append(f"### {text}")
                elif "Heading 4" in style:
                    lines.append(f"#### {text}")
                elif "List" in style:
                    lines.append(f"- {text}")
                else:
                    lines.append(text)

            # 处理表格
            for table in doc.tables:
                lines.append("")
                for i, row in enumerate(table.rows):
                    row_text = "| " + " | ".join(cell.text for cell in row.cells) + " |"
                    lines.append(row_text)
                    if i == 0:
                        lines.append("| " + " | ".join("---" for _ in row.cells) + " |")
                lines.append("")

            content = "\n\n".join(lines)

        elif target_format == "txt":
            # 转换为纯文本
            content = "\n".join(para.text for para in doc.paragraphs)

        elif target_format == "pdf":
            # Word -> PDF: 使用 reportlab 将文档内容渲染为 PDF
            from reportlab.lib.pagesizes import A4
            from reportlab.lib.styles import getSampleStyleSheet, ParagraphStyle
            from reportlab.lib.units import cm
            from reportlab.platypus import SimpleDocTemplate, Paragraph, Spacer, Table, TableStyle

            # 注册中文字体
            from handlers.font_utils import register_chinese_font
            font_name = register_chinese_font()

            # PDF 必须写入文件
            if not output_path:
                output_path = os.path.splitext(path)[0] + ".pdf"

            os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)

            doc_pdf = SimpleDocTemplate(output_path, pagesize=A4)
            styles = getSampleStyleSheet()
            title_style = ParagraphStyle("CustomTitle", parent=styles["Title"], fontName=font_name, fontSize=24, spaceAfter=30)
            heading_style = ParagraphStyle("CustomHeading", parent=styles["Heading2"], fontName=font_name, fontSize=16, spaceAfter=12)
            body_style = ParagraphStyle("CustomBody", parent=styles["Normal"], fontName=font_name, fontSize=12, leading=20, spaceAfter=10)

            elements = []
            for para in doc.paragraphs:
                style = para.style.name if para.style else ""
                text = para.text
                if not text.strip():
                    continue
                if "Heading 1" in style:
                    elements.append(Paragraph(html.escape(text), heading_style))
                elif "Heading 2" in style:
                    elements.append(Paragraph(html.escape(text), heading_style))
                elif "Heading 3" in style:
                    elements.append(Paragraph(html.escape(text), body_style))
                elif "Title" in style:
                    elements.append(Paragraph(html.escape(text), title_style))
                else:
                    elements.append(Paragraph(html.escape(text), body_style))
                elements.append(Spacer(1, 0.3 * cm))

            # 处理表格
            for table in doc.tables:
                table_data = []
                for row in table.rows:
                    row_data = [cell.text for cell in row.cells]
                    table_data.append(row_data)
                if table_data:
                    t = Table(table_data)
                    t.setStyle(TableStyle([
                        ('GRID', (0, 0), (-1, -1), 0.5, '#999999'),
                        ('FONTNAME', (0, 0), (-1, -1), font_name),
                        ('FONTSIZE', (0, 0), (-1, -1), 10),
                    ]))
                    elements.append(t)
                    elements.append(Spacer(1, 0.5 * cm))

            doc_pdf.build(elements)
            content = None  # PDF 已写入文件

        else:
            self.logger.error("convert: 不支持的目标格式: %s", target_format)
            return {"error": f"不支持的目标格式: {target_format}"}

        # 写入输出文件
        if content is None:
            # content 为 None 表示已在转换分支内直接写入文件（如 PDF）
            self.logger.info("convert: 格式转换完成, output_path=%s, format=%s", output_path, target_format)
            return {
                "path": output_path,
                "format": target_format,
                "message": f"已转换为 {target_format} 格式",
            }
        elif output_path:
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
        """分析 Word 文档

        params:
            path: 文件路径
        """
        path = params.get("path", "")
        if not path:
            self.logger.error("analyze: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("analyze: 开始分析 Word 文档, path=%s", path)

        doc = Document(path)

        # 统计信息
        total_chars = sum(len(p.text) for p in doc.paragraphs)
        total_words = sum(len(p.text.split()) for p in doc.paragraphs)
        heading_count = sum(
            1 for p in doc.paragraphs
            if p.style and ("Heading" in p.style.name or "Title" in p.style.name)
        )

        # 提取标题结构
        headings = []
        for para in doc.paragraphs:
            if para.style and ("Heading" in para.style.name or "Title" in para.style.name):
                level = 1
                try:
                    level = int(para.style.name.replace("Heading ", ""))
                except ValueError:
                    if "Title" in para.style.name:
                        level = 0
                headings.append({"level": level, "text": para.text})

        self.logger.info("analyze: Word 文档分析完成, path=%s, 段落数=%d, 标题数=%d", path, len(doc.paragraphs), heading_count)
        return {
            "file_size": os.path.getsize(path),
            "paragraph_count": len(doc.paragraphs),
            "table_count": len(doc.tables),
            "total_chars": total_chars,
            "total_words": total_words,
            "heading_count": heading_count,
            "headings": headings,
            "properties": {
                "title": doc.core_properties.title or "",
                "author": doc.core_properties.author or "",
                "created": str(doc.core_properties.created) if doc.core_properties.created else "",
                "modified": str(doc.core_properties.modified) if doc.core_properties.modified else "",
            },
        }

    def _add_content_block(self, doc: Document, block: dict):
        """添加结构化内容块"""
        block_type = block.get("type", "paragraph")

        if block_type == "heading":
            level = block.get("level", 1)
            text = block.get("text", "")
            doc.add_heading(text, level=level)

        elif block_type == "paragraph":
            text = block.get("text", "")
            style = block.get("style", None)
            alignment = block.get("alignment", None)
            p = doc.add_paragraph(text)
            if style:
                p.style = style
            if alignment:
                align_map = {
                    "left": WD_ALIGN_PARAGRAPH.LEFT,
                    "center": WD_ALIGN_PARAGRAPH.CENTER,
                    "right": WD_ALIGN_PARAGRAPH.RIGHT,
                    "justify": WD_ALIGN_PARAGRAPH.JUSTIFY,
                }
                if alignment in align_map:
                    p.alignment = align_map[alignment]

        elif block_type == "table":
            headers = block.get("headers", [])
            rows_data = block.get("rows", [])
            data = block.get("data", [])

            # 兼容两种格式：
            # 格式1: {"headers": [...], "rows": [[...], [...]]} - rows 是数据列表
            # 格式2: {"rows": 3, "cols": 2, "data": [[...]]} - rows/cols 是行列数
            if isinstance(rows_data, list) and rows_data:
                # 格式1: rows 是数据列表，自动计算行列数
                if headers:
                    all_rows = [headers] + rows_data
                else:
                    all_rows = rows_data
                num_rows = len(all_rows)
                num_cols = max(len(r) for r in all_rows) if all_rows else 1
                table = doc.add_table(rows=num_rows, cols=num_cols)
                table.style = "Table Grid"
                for i, row_data in enumerate(all_rows):
                    for j, cell_text in enumerate(row_data):
                        if j < num_cols:
                            table.rows[i].cells[j].text = str(cell_text)
            else:
                # 格式2: rows/cols 是整数
                rows = rows_data if isinstance(rows_data, int) else 1
                cols = block.get("cols", 1)
                table = doc.add_table(rows=rows, cols=cols)
                table.style = "Table Grid"
                for i, row_data in enumerate(data):
                    if i < rows:
                        for j, cell_text in enumerate(row_data):
                            if j < cols:
                                table.rows[i].cells[j].text = str(cell_text)

        elif block_type == "list":
            items = block.get("items", [])
            for item in items:
                doc.add_paragraph(str(item), style="List Bullet")

        elif block_type == "image":
            image_path = block.get("path", "")
            width = block.get("width", None)
            if image_path and os.path.exists(image_path):
                if width:
                    doc.add_picture(image_path, width=Inches(width))
                else:
                    doc.add_picture(image_path)
