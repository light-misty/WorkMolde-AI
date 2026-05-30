"""Word 文档处理器
基于 python-docx 实现 Word 文档的生成、读取、修改、转换
遵循 docx Skill 规范：页面尺寸、样式覆盖、专业表格、列表、页眉页脚、书签超链接、颜色编码
"""

import os
import json
import html
import logging
from typing import Any

from docx import Document
from docx.shared import Inches, Pt, Cm, RGBColor, Emu
from docx.enum.text import WD_ALIGN_PARAGRAPH
from docx.enum.table import WD_TABLE_ALIGNMENT
from docx.enum.style import WD_STYLE_TYPE
from docx.oxml.ns import qn
from docx.oxml import OxmlElement


class WordHandler:
    """Word (.docx) 文档处理器"""

    logger = logging.getLogger(__name__)

    # 颜色编码映射表：根据 colorType 字段应用对应颜色
    # 遵循 Skill 规范：蓝色(0,0,255)=输入值、黑色(0,0,0)=公式、绿色(0,128,0)=跨表引用、红色(255,0,0)=外部链接
    COLOR_MAP = {
        "input": RGBColor(0x00, 0x00, 0xFF),
        "formula": RGBColor(0x00, 0x00, 0x00),
        "cross_ref": RGBColor(0x00, 0x80, 0x00),
        "external": RGBColor(0xFF, 0x00, 0x00),
    }

    # 页面尺寸预设（DXA 单位，1 inch = 1440 DXA）
    PAGE_SIZES = {
        "letter": {"width": 12240, "height": 15840},
        "a4": {"width": 11906, "height": 16838},
    }

    # DXA 到 EMU 的转换系数：1 DXA = 635 EMU
    DXA_TO_EMU = 635

    def generate(self, params: dict) -> dict:
        """生成 Word 文档

        params:
            path: 输出文件路径
            title: 文档标题
            content: 文档内容（结构化 JSON 字符串或纯文本）
                    结构化格式: {"blocks": [{type, ...}]}
                    block 类型: heading/paragraph/table/list/image
            author: 作者
            template: 模板路径（可选）
            pageSize: 页面尺寸 "letter" | "a4"（可选）
            header: 页眉文本（可选）
            footer: 页脚文本（可选）
            pageNumber: 是否显示页码（默认 true）
            includeToc: 是否包含目录（默认 false）
            colorCoding: 是否启用颜色编码（默认 true）
            bookmarks: 书签列表 [{id, text}]（可选）
            hyperlinks: 超链接列表 [{text, url, anchor}]（可选）
        """
        path = params.get("path", "")
        title = params.get("title", "")
        content = params.get("content", "")
        author = params.get("author", "")
        page_size = params.get("pageSize", None)
        header_text = params.get("header", None)
        footer_text = params.get("footer", None)
        page_number = params.get("pageNumber", True)
        include_toc = params.get("includeToc", False)
        color_coding = params.get("colorCoding", True)
        bookmarks = params.get("bookmarks", []) or []
        hyperlinks = params.get("hyperlinks", []) or []

        if not path:
            self.logger.error("generate: 缺少输出文件路径")
            return {"error": "缺少输出文件路径"}

        self.logger.info("generate: 开始生成 Word 文档, path=%s", path)

        # 确保输出目录存在
        os.makedirs(os.path.dirname(path) or ".", exist_ok=True)

        doc = Document()

        # 设置页面尺寸
        if page_size:
            self._set_page_size(doc, page_size)

        # 应用样式覆盖：默认字体 Arial 12pt，标题层级规范
        self._apply_style_overrides(doc)

        # 设置文档属性
        if author:
            doc.core_properties.author = author
        if title:
            doc.core_properties.title = title

        # 添加标题
        if title:
            doc.add_heading(title, level=0)

        # 添加目录（在标题之后、正文之前）
        if include_toc:
            doc.add_heading("目录", level=1)
            self._add_toc(doc)

        # 处理内容
        if isinstance(content, str):
            # 尝试解析为结构化 JSON
            parsed_content = self._try_parse_json_content(content)
            if parsed_content is not None:
                self._process_structured_content(doc, parsed_content, color_coding)
            else:
                # 纯文本内容，按段落分割
                for paragraph_text in content.split("\n"):
                    if paragraph_text.strip():
                        doc.add_paragraph(paragraph_text)
        elif isinstance(content, list):
            # 结构化内容列表（blocks 数组）
            self._process_structured_content(doc, content, color_coding)
        elif isinstance(content, dict):
            # 结构化内容字典 {"blocks": [...]}
            blocks = content.get("blocks", [])
            self._process_structured_content(doc, blocks, color_coding)

        # 添加书签
        bookmark_id_counter = 0
        for bm in bookmarks:
            bm_id = bm.get("id", f"bookmark_{bookmark_id_counter}")
            bm_text = bm.get("text", "")
            p = doc.add_paragraph()
            self._add_bookmark(p, bm_id, bm_text, bookmark_id_counter)
            bookmark_id_counter += 1

        # 添加超链接
        for hl in hyperlinks:
            hl_text = hl.get("text", "")
            hl_url = hl.get("url", None)
            hl_anchor = hl.get("anchor", None)
            p = doc.add_paragraph()
            if hl_url:
                self._add_hyperlink(p, hl_text, hl_url)
            elif hl_anchor:
                self._add_internal_link(p, hl_text, hl_anchor)

        # 添加页眉
        if header_text:
            self._add_header(doc, header_text)

        # 添加页脚（含可选页码）
        if footer_text or page_number:
            self._add_footer(doc, footer_text or "", page_number)

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
                 {"type": "add_table", "rows": 3, "cols": 2, "data": [[...]]},
                 {"type": "addHeader", "text": "页眉文本"},
                 {"type": "addFooter", "text": "页脚文本", "pageNumber": true},
                 {"type": "addBookmark", "id": "chapter1", "text": "第一章"},
                 {"type": "addHyperlink", "text": "点击跳转", "url": "https://..."},
                 {"type": "addHyperlink", "text": "跳转", "anchor": "chapter1"},
                 {"type": "setPageSize", "size": "a4"},
                 {"type": "addToc"}]
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
                if "index" in op:
                    # 按段落索引替换整段内容
                    index = op.get("index", 0)
                    new_text = op.get("text", "")
                    if 0 <= index < len(doc.paragraphs):
                        para = doc.paragraphs[index]
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
                text = op.get("text", "")
                style = op.get("style", None)
                p = doc.add_paragraph(text)
                if style:
                    p.style = style
                modified_count += 1

            elif op_type == "add_heading":
                text = op.get("text", "")
                level = op.get("level", 1)
                doc.add_heading(text, level=level)
                modified_count += 1

            elif op_type == "add_table":
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

            elif op_type == "addHeader":
                header_text = op.get("text", "")
                self._add_header(doc, header_text)
                modified_count += 1

            elif op_type == "addFooter":
                footer_text = op.get("text", "")
                page_number = op.get("pageNumber", True)
                self._add_footer(doc, footer_text, page_number)
                modified_count += 1

            elif op_type == "addBookmark":
                bm_id = op.get("id", "")
                bm_text = op.get("text", "")
                if bm_id:
                    p = doc.add_paragraph()
                    numeric_id = abs(hash(bm_id)) % 10000
                    self._add_bookmark(p, bm_id, bm_text, numeric_id)
                    modified_count += 1
                else:
                    self.logger.warning("modify: addBookmark 缺少 id 字段")

            elif op_type == "addHyperlink":
                hl_text = op.get("text", "")
                hl_url = op.get("url", None)
                hl_anchor = op.get("anchor", None)
                p = doc.add_paragraph()
                if hl_url:
                    self._add_hyperlink(p, hl_text, hl_url)
                    modified_count += 1
                elif hl_anchor:
                    self._add_internal_link(p, hl_text, hl_anchor)
                    modified_count += 1
                else:
                    self.logger.warning("modify: addHyperlink 缺少 url 或 anchor 字段")

            elif op_type == "setPageSize":
                size = op.get("size", "")
                if size:
                    self._set_page_size(doc, size)
                    modified_count += 1
                else:
                    self.logger.warning("modify: setPageSize 缺少 size 字段")

            elif op_type == "addToc":
                self._add_toc(doc)
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
            format: 目标格式（md, txt, pdf）
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
            content = self._convert_to_markdown(doc)

        elif target_format == "txt":
            content = "\n".join(para.text for para in doc.paragraphs)

        elif target_format == "pdf":
            content = self._convert_to_pdf(doc, output_path or os.path.splitext(path)[0] + ".pdf")
            # PDF 已写入文件，content 置空
            content = None

        else:
            self.logger.error("convert: 不支持的目标格式: %s", target_format)
            return {"error": f"不支持的目标格式: {target_format}"}

        # 写入输出文件或返回内容
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

    # ------------------------------------------------------------------ #
    #  样式覆盖（Skill 规范）
    # ------------------------------------------------------------------ #

    def _apply_style_overrides(self, doc: Document):
        """应用样式覆盖，遵循 Skill 规范

        - 默认字体: Arial 12pt
        - 标题1: 16pt 粗体, 间距前后 240 DXA
        - 标题2: 14pt 粗体, 间距前后 180 DXA
        - 正文: 12pt, 行间距 1.5
        """
        # 设置默认字体为 Arial 12pt
        style_normal = doc.styles["Normal"]
        style_normal.font.name = "Arial"
        style_normal.font.size = Pt(12)
        # 设置行间距 1.5 倍
        style_normal.paragraph_format.line_spacing = 1.5

        # 标题1: 16pt 粗体, 间距前后 240 DXA
        style_h1 = doc.styles["Heading 1"]
        style_h1.font.name = "Arial"
        style_h1.font.size = Pt(16)
        style_h1.font.bold = True
        style_h1.paragraph_format.space_before = Emu(240 * self.DXA_TO_EMU)
        style_h1.paragraph_format.space_after = Emu(240 * self.DXA_TO_EMU)

        # 标题2: 14pt 粗体, 间距前后 180 DXA
        style_h2 = doc.styles["Heading 2"]
        style_h2.font.name = "Arial"
        style_h2.font.size = Pt(14)
        style_h2.font.bold = True
        style_h2.paragraph_format.space_before = Emu(180 * self.DXA_TO_EMU)
        style_h2.paragraph_format.space_after = Emu(180 * self.DXA_TO_EMU)

        self.logger.debug("_apply_style_overrides: 已应用样式覆盖")

    # ------------------------------------------------------------------ #
    #  结构化内容处理
    # ------------------------------------------------------------------ #

    def _try_parse_json_content(self, content: str):
        """尝试将字符串内容解析为结构化 JSON

        支持格式:
        - {"blocks": [{type, ...}]}
        - [{type, ...}]（直接数组）

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

    def _process_structured_content(self, doc: Document, blocks: list, color_coding: bool = True):
        """处理结构化内容块列表

        Args:
            doc: Document 对象
            blocks: 内容块列表
            color_coding: 是否启用颜色编码
        """
        for block in blocks:
            if not isinstance(block, dict):
                continue
            self._add_content_block(doc, block, color_coding=color_coding)

    def _add_content_block(self, doc: Document, block: dict, color_coding: bool = True):
        """添加结构化内容块

        支持的 block 类型:
        - heading: 标题 {type:"heading", level:1, text:"...", colorType:"input"}
        - paragraph: 段落 {type:"paragraph", text:"...", style, alignment, colorType, bold, italic}
        - table: 专业表格 {type:"table", headers:[], rows:[], width, colWidths, colorType}
        - list: 列表 {type:"list", items:[], ordered, colorType}
        - image: 图片 {type:"image", path:"...", width, height, format, altText}
        """
        block_type = block.get("type", "paragraph")

        if block_type == "heading":
            self._add_heading_block(doc, block, color_coding)

        elif block_type == "paragraph":
            self._add_paragraph_block(doc, block, color_coding)

        elif block_type == "table":
            self._add_table_block(doc, block, color_coding)

        elif block_type == "list":
            self._add_list_block(doc, block, color_coding)

        elif block_type == "image":
            self._add_image_block(doc, block)

    def _add_heading_block(self, doc: Document, block: dict, color_coding: bool):
        """添加标题块"""
        level = block.get("level", 1)
        text = block.get("text", "")
        heading = doc.add_heading(text, level=level)
        # 颜色编码：根据 colorType 应用颜色
        if color_coding and "colorType" in block:
            color = self.COLOR_MAP.get(block["colorType"])
            if color:
                for run in heading.runs:
                    run.font.color.rgb = color

    def _add_paragraph_block(self, doc: Document, block: dict, color_coding: bool):
        """添加段落块"""
        text = block.get("text", "")
        style = block.get("style", None)
        alignment = block.get("alignment", None)
        bold = block.get("bold", False)
        italic = block.get("italic", False)

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

        # 应用粗体/斜体
        if bold or italic:
            for run in p.runs:
                if bold:
                    run.font.bold = True
                if italic:
                    run.font.italic = True

        # 颜色编码：根据 colorType 应用颜色
        if color_coding and "colorType" in block:
            color = self.COLOR_MAP.get(block["colorType"])
            if color:
                for run in p.runs:
                    run.font.color.rgb = color

    def _add_table_block(self, doc: Document, block: dict, color_coding: bool):
        """添加专业表格块，遵循 Skill 规范

        规范要求:
        - 设置表格宽度（DXA 单位）
        - 同时设置列宽和每个单元格的宽度
        - 边框: 单线 1pt 灰色 (#CCCCCC)
        - 使用 ShadingType.CLEAR 而非 SOLID 防止黑色背景
        """
        headers = block.get("headers", [])
        rows_data = block.get("rows", [])
        data = block.get("data", [])
        # 表格总宽度（DXA 单位），默认使用页面可用宽度
        table_width_dxa = block.get("width", None)
        # 各列宽度（DXA 单位列表）
        col_widths_dxa = block.get("colWidths", [])

        # 兼容两种格式
        if isinstance(rows_data, list) and rows_data and not isinstance(rows_data[0], (int, float)):
            # 格式1: rows 是数据列表
            if headers:
                all_rows = [headers] + rows_data
            else:
                all_rows = rows_data
            num_rows = len(all_rows)
            num_cols = max(len(r) for r in all_rows) if all_rows else 1
        else:
            # 格式2: rows/cols 是整数
            num_rows = rows_data if isinstance(rows_data, int) else (len(data) if data else 1)
            num_cols = block.get("cols", 1)
            all_rows = data if data else [[""] * num_cols for _ in range(num_rows)]

        table = doc.add_table(rows=num_rows, cols=num_cols)
        table.style = "Table Grid"
        table.alignment = WD_TABLE_ALIGNMENT.CENTER

        # 设置表格总宽度
        if table_width_dxa:
            table.width = Emu(table_width_dxa * self.DXA_TO_EMU)

        # 设置列宽和单元格宽度
        if col_widths_dxa and len(col_widths_dxa) >= num_cols:
            for i, col_w in enumerate(col_widths_dxa):
                if i < num_cols:
                    col_width_emu = Emu(col_w * self.DXA_TO_EMU)
                    table.columns[i].width = col_width_emu

        # 填充表格数据
        for i, row_data in enumerate(all_rows):
            for j, cell_text in enumerate(row_data):
                if j < num_cols:
                    cell = table.rows[i].cells[j]
                    cell.text = str(cell_text)
                    # 设置单元格宽度（与列宽一致）
                    if col_widths_dxa and j < len(col_widths_dxa):
                        cell.width = Emu(col_widths_dxa[j] * self.DXA_TO_EMU)

        # 应用专业边框：1pt 灰色 #CCCCCC
        self._set_table_borders(table, "CCCCCC", 1)

        # 表头行样式：粗体居中
        if headers and num_rows > 0:
            for cell in table.rows[0].cells:
                for paragraph in cell.paragraphs:
                    paragraph.alignment = WD_ALIGN_PARAGRAPH.CENTER
                    for run in paragraph.runs:
                        run.font.bold = True

        # 颜色编码
        if color_coding and "colorType" in block:
            color = self.COLOR_MAP.get(block["colorType"])
            if color:
                for row in table.rows:
                    for cell in row.cells:
                        for paragraph in cell.paragraphs:
                            for run in paragraph.runs:
                                run.font.color.rgb = color

    def _add_list_block(self, doc: Document, block: dict, color_coding: bool):
        """添加列表块，遵循 Skill 规范

        规范要求:
        - 使用 WD_STYLE_PARAGRAPH.LIST_BULLET 而非 Unicode 字符
        - 缩进: 左 720 DXA, 悬挂 360 DXA
        """
        items = block.get("items", [])
        ordered = block.get("ordered", False)

        for item in items:
            if ordered:
                p = doc.add_paragraph(str(item), style="List Number")
            else:
                p = doc.add_paragraph(str(item), style="List Bullet")

            # 设置缩进：左 720 DXA，悬挂 360 DXA
            p.paragraph_format.left_indent = Emu(720 * self.DXA_TO_EMU)
            p.paragraph_format.first_line_indent = Emu(-360 * self.DXA_TO_EMU)

            # 颜色编码
            if color_coding and "colorType" in block:
                color = self.COLOR_MAP.get(block["colorType"])
                if color:
                    for run in p.runs:
                        run.font.color.rgb = color

    def _add_image_block(self, doc: Document, block: dict):
        """添加图片块，遵循 Skill 规范

        规范要求:
        - 必须指定图片格式（png/jpg/jpeg/gif/bmp/svg）
        - 需指定 width/height
        - 提供 altText 三字段: title, description, name
        """
        image_path = block.get("path", "")
        width = block.get("width", None)
        height = block.get("height", None)
        # altText 三字段
        alt_text = block.get("altText", {})

        if not image_path or not os.path.exists(image_path):
            self.logger.warning("_add_image_block: 图片路径不存在: %s", image_path)
            return

        # 插入图片
        if width and height:
            pic = doc.add_picture(image_path, width=Inches(width), height=Inches(height))
        elif width:
            pic = doc.add_picture(image_path, width=Inches(width))
        elif height:
            pic = doc.add_picture(image_path, height=Inches(height))
        else:
            pic = doc.add_picture(image_path)

        # 设置 altText（如果提供了）
        if alt_text:
            inline = pic.inline
            if inline is not None:
                docPr = inline.find(qn('wp:docPr'))
                if docPr is not None:
                    if "title" in alt_text:
                        docPr.set("title", alt_text["title"])
                    if "description" in alt_text:
                        docPr.set("descr", alt_text["description"])
                    if "name" in alt_text:
                        docPr.set("name", alt_text["name"])

    # ------------------------------------------------------------------ #
    #  表格边框设置（Skill 规范）
    # ------------------------------------------------------------------ #

    def _set_table_borders(self, table, color_hex: str = "CCCCCC", size_pt: int = 1):
        """设置表格边框样式

        遵循 Skill 规范: 单线 1pt 灰色边框

        Args:
            table: 表格对象
            color_hex: 边框颜色十六进制值（不带 #）
            size_pt: 边框粗细（单位: 1/8 pt，1 表示 1/8 pt）
        """
        tbl = table._tbl
        tblPr = tbl.tblPr if tbl.tblPr is not None else OxmlElement("w:tblPr")

        borders = OxmlElement("w:tblBorders")
        for border_name in ("top", "left", "bottom", "right", "insideH", "insideV"):
            border = OxmlElement(f"w:{border_name}")
            border.set(qn("w:val"), "single")
            border.set(qn("w:sz"), str(size_pt * 8))  # sz 单位为 1/8 pt
            border.set(qn("w:space"), "0")
            border.set(qn("w:color"), color_hex)
            borders.append(border)

        # 移除已有边框设置
        existing_borders = tblPr.find(qn("w:tblBorders"))
        if existing_borders is not None:
            tblPr.remove(existing_borders)

        tblPr.append(borders)

    # ------------------------------------------------------------------ #
    #  页面尺寸设置
    # ------------------------------------------------------------------ #

    def _set_page_size(self, doc: Document, size: str):
        """设置文档页面尺寸

        Args:
            doc: Document 对象
            size: 页面尺寸标识 "letter" | "a4"
        """
        size_lower = size.lower() if size else ""
        if size_lower not in self.PAGE_SIZES:
            self.logger.warning("_set_page_size: 不支持的页面尺寸: %s, 支持的值: %s", size, list(self.PAGE_SIZES.keys()))
            return

        page_size = self.PAGE_SIZES[size_lower]
        section = doc.sections[0]
        section.page_width = Emu(page_size["width"] * self.DXA_TO_EMU)
        section.page_height = Emu(page_size["height"] * self.DXA_TO_EMU)
        self.logger.debug("_set_page_size: 设置页面尺寸为 %s (%d x %d DXA)", size_lower, page_size["width"], page_size["height"])

    # ------------------------------------------------------------------ #
    #  页眉页脚
    # ------------------------------------------------------------------ #

    def _add_header(self, doc: Document, text: str):
        """添加页眉

        Args:
            doc: Document 对象
            text: 页眉文本
        """
        section = doc.sections[0]
        header = section.header
        header.is_linked_to_previous = False
        if header.paragraphs:
            paragraph = header.paragraphs[0]
        else:
            paragraph = header.add_paragraph()
        paragraph.text = text
        paragraph.alignment = WD_ALIGN_PARAGRAPH.CENTER
        self.logger.debug("_add_header: 已添加页眉: %s", text[:50])

    def _add_footer(self, doc: Document, text: str, page_number: bool = True):
        """添加页脚（含可选页码域代码）

        Args:
            doc: Document 对象
            text: 页脚文本
            page_number: 是否显示页码（默认 True）
        """
        section = doc.sections[0]
        footer = section.footer
        footer.is_linked_to_previous = False
        if footer.paragraphs:
            paragraph = footer.paragraphs[0]
        else:
            paragraph = footer.add_paragraph()
        paragraph.alignment = WD_ALIGN_PARAGRAPH.CENTER

        # 添加页脚文本
        if text:
            paragraph.add_run(text)

        # 添加页码域代码
        if page_number:
            if text:
                paragraph.add_run(" ")
            # fldChar begin
            run_begin = paragraph.add_run()
            fldChar_begin = OxmlElement("w:fldChar")
            fldChar_begin.set(qn("w:fldCharType"), "begin")
            run_begin._r.append(fldChar_begin)

            # instrText: PAGE 域代码
            run_instr = paragraph.add_run()
            instrText = OxmlElement("w:instrText")
            instrText.set(qn("xml:space"), "preserve")
            instrText.text = " PAGE "
            run_instr._r.append(instrText)

            # fldChar separate
            run_sep = paragraph.add_run()
            fldChar_sep = OxmlElement("w:fldChar")
            fldChar_sep.set(qn("w:fldCharType"), "separate")
            run_sep._r.append(fldChar_sep)

            # 占位文本
            run_placeholder = paragraph.add_run("1")

            # fldChar end
            run_end = paragraph.add_run()
            fldChar_end = OxmlElement("w:fldChar")
            fldChar_end.set(qn("w:fldCharType"), "end")
            run_end._r.append(fldChar_end)

        self.logger.debug("_add_footer: 已添加页脚, text=%s, pageNumber=%s", text[:50] if text else "", page_number)

    # ------------------------------------------------------------------ #
    #  书签和超链接
    # ------------------------------------------------------------------ #

    def _add_bookmark(self, paragraph, bookmark_id: str, text: str, numeric_id: int = 0):
        """在段落中添加书签

        Args:
            paragraph: 段落对象
            bookmark_id: 书签名称（字符串标识）
            text: 书签包围的文本
            numeric_id: 书签数字 ID（XML 中 w:id 需要整数）
        """
        bookmark_start = OxmlElement("w:bookmarkStart")
        bookmark_start.set(qn("w:id"), str(numeric_id))
        bookmark_start.set(qn("w:name"), bookmark_id)

        bookmark_end = OxmlElement("w:bookmarkEnd")
        bookmark_end.set(qn("w:id"), str(numeric_id))

        run = paragraph.add_run(text)

        run._r.addprevious(bookmark_start)
        run._r.addnext(bookmark_end)

        self.logger.debug("_add_bookmark: 已添加书签 id=%s, text=%s", bookmark_id, text[:50])

    def _add_hyperlink(self, paragraph, text: str, url: str):
        """添加外部超链接

        python-docx 没有原生超链接 API，需要通过 OxmlElement 操作 XML 实现。

        Args:
            paragraph: 段落对象
            text: 超链接显示文本
            url: 超链接目标 URL
        """
        part = paragraph.part
        r_id = part.relate_to(
            url,
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink",
            is_external=True,
        )

        hyperlink = OxmlElement("w:hyperlink")
        hyperlink.set(qn("r:id"), r_id)

        new_run = OxmlElement("w:r")
        rPr = OxmlElement("w:rPr")

        # 蓝色字体
        color_elem = OxmlElement("w:color")
        color_elem.set(qn("w:val"), "0563C1")
        rPr.append(color_elem)

        # 下划线
        u_elem = OxmlElement("w:u")
        u_elem.set(qn("w:val"), "single")
        rPr.append(u_elem)

        new_run.append(rPr)

        t_elem = OxmlElement("w:t")
        t_elem.text = text
        new_run.append(t_elem)

        hyperlink.append(new_run)
        paragraph._p.append(hyperlink)

        self.logger.debug("_add_hyperlink: 已添加超链接 text=%s, url=%s", text[:50], url[:80])

    def _add_internal_link(self, paragraph, text: str, anchor: str):
        """添加内部书签链接

        Args:
            paragraph: 段落对象
            text: 链接显示文本
            anchor: 目标书签名称（对应 bookmarkStart 的 w:name）
        """
        hyperlink = OxmlElement("w:hyperlink")
        hyperlink.set(qn("w:anchor"), anchor)

        new_run = OxmlElement("w:r")
        rPr = OxmlElement("w:rPr")

        # 蓝色字体
        color_elem = OxmlElement("w:color")
        color_elem.set(qn("w:val"), "0563C1")
        rPr.append(color_elem)

        # 下划线
        u_elem = OxmlElement("w:u")
        u_elem.set(qn("w:val"), "single")
        rPr.append(u_elem)

        new_run.append(rPr)

        t_elem = OxmlElement("w:t")
        t_elem.text = text
        new_run.append(t_elem)

        hyperlink.append(new_run)
        paragraph._p.append(hyperlink)

        self.logger.debug("_add_internal_link: 已添加内部链接 text=%s, anchor=%s", text[:50], anchor)

    # ------------------------------------------------------------------ #
    #  目录
    # ------------------------------------------------------------------ #

    def _add_toc(self, doc: Document):
        """添加目录（TOC 域代码）

        注意：目录内容在 Word 中打开后需要手动更新域（右键 -> 更新域）才会显示。
        """
        paragraph = doc.add_paragraph()

        # fldChar begin
        run_begin = paragraph.add_run()
        fldChar_begin = OxmlElement("w:fldChar")
        fldChar_begin.set(qn("w:fldCharType"), "begin")
        run_begin._r.append(fldChar_begin)

        # instrText: TOC 域指令
        run_instr = paragraph.add_run()
        instrText = OxmlElement("w:instrText")
        instrText.set(qn("xml:space"), "preserve")
        instrText.text = ' TOC \\o "1-3" \\h \\z \\u '
        run_instr._r.append(instrText)

        # fldChar separate
        run_sep = paragraph.add_run()
        fldChar_sep = OxmlElement("w:fldChar")
        fldChar_sep.set(qn("w:fldCharType"), "separate")
        run_sep._r.append(fldChar_sep)

        # 占位提示文本
        run_placeholder = paragraph.add_run('（请右键点击此处，选择"更新域"以生成目录）')

        # fldChar end
        run_end = paragraph.add_run()
        fldChar_end = OxmlElement("w:fldChar")
        fldChar_end.set(qn("w:fldCharType"), "end")
        run_end._r.append(fldChar_end)

        self.logger.debug("_add_toc: 已添加目录域代码")

    # ------------------------------------------------------------------ #
    #  格式转换辅助方法
    # ------------------------------------------------------------------ #

    def _convert_to_markdown(self, doc: Document) -> str:
        """将 Word 文档内容转换为 Markdown"""
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

        return "\n\n".join(lines)

    def _convert_to_pdf(self, doc: Document, output_path: str) -> None:
        """将 Word 文档内容转换为 PDF（使用 reportlab 渲染）"""
        from reportlab.lib.pagesizes import A4
        from reportlab.lib.styles import getSampleStyleSheet, ParagraphStyle
        from reportlab.lib.units import cm
        from reportlab.platypus import SimpleDocTemplate, Paragraph, Spacer, Table, TableStyle

        # 注册中文字体
        from handlers.font_utils import register_chinese_font
        font_name = register_chinese_font()

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
                    ("GRID", (0, 0), (-1, -1), 0.5, "#999999"),
                    ("FONTNAME", (0, 0), (-1, -1), font_name),
                    ("FONTSIZE", (0, 0), (-1, -1), 10),
                ]))
                elements.append(t)
                elements.append(Spacer(1, 0.5 * cm))

        doc_pdf.build(elements)
