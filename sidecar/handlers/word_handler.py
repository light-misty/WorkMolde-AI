"""Word 文档处理器
基于 python-docx 实现 Word 文档的生成、读取、修改、转换
遵循 docx Skill 规范：页面尺寸、样式覆盖、专业表格、列表、页眉页脚、书签超链接、颜色编码
支持 Markdown 内容自动解析为专业 Word 元素
"""

import os
import json
import re
import html
import logging
from typing import Any, Optional, List, Tuple

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

    # 颜色编码映射表
    COLOR_MAP = {
        "input": RGBColor(0x00, 0x00, 0xFF),
        "formula": RGBColor(0x00, 0x00, 0x00),
        "cross_ref": RGBColor(0x00, 0x80, 0x00),
        "external": RGBColor(0xFF, 0x00, 0x00),
    }

    # 页面尺寸预设（DXA 单位）
    PAGE_SIZES = {
        "letter": {"width": 12240, "height": 15840},
        "a4": {"width": 11906, "height": 16838},
    }

    DXA_TO_EMU = 635

    # 专业配色方案
    THEME_COLORS = {
        "heading1": RGBColor(0x1F, 0x4E, 0x79),
        "heading2": RGBColor(0x2E, 0x75, 0xB6),
        "heading3": RGBColor(0x5B, 0x9B, 0xD5),
        "table_header_bg": "D6E4F0",
        "table_alt_row_bg": "EDF2F9",
        "table_border": "B4C6E7",
        "accent": RGBColor(0x2E, 0x75, 0xB6),
    }

    # 东亚字体和拉丁字体
    EAST_ASIAN_FONT = "微软雅黑"
    LATIN_FONT = "Arial"

    def generate(self, params: dict) -> dict:
        """生成 Word 文档

        params:
            path: 输出文件路径
            title: 文档标题
            content: 文档内容（Markdown 文本、结构化 JSON 字符串或纯文本）
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

        os.makedirs(os.path.dirname(path) or ".", exist_ok=True)

        doc = Document()

        # 设置页面尺寸
        if page_size:
            self._set_page_size(doc, page_size)

        # 设置专业页边距（2.54cm = 1 inch）
        section = doc.sections[0]
        section.top_margin = Cm(2.54)
        section.bottom_margin = Cm(2.54)
        section.left_margin = Cm(2.54)
        section.right_margin = Cm(2.54)

        # 应用专业样式覆盖
        self._apply_style_overrides(doc)

        # 设置文档属性
        if author:
            doc.core_properties.author = author
        if title:
            doc.core_properties.title = title

        # 添加标题（使用专业标题样式）
        if title:
            title_para = doc.add_paragraph()
            title_run = title_para.add_run(title)
            title_run.font.size = Pt(26)
            title_run.font.bold = True
            title_run.font.color.rgb = self.THEME_COLORS["heading1"]
            self._set_run_fonts(title_run, self.LATIN_FONT, self.EAST_ASIAN_FONT)
            title_para.alignment = WD_ALIGN_PARAGRAPH.CENTER
            title_para.paragraph_format.space_after = Pt(12)
            # 标题下方添加分隔线
            self._add_horizontal_rule(doc)

        # 添加目录
        if include_toc:
            doc.add_heading("目录", level=1)
            self._add_toc(doc)

        # 处理内容：优先尝试结构化 JSON，否则尝试 Markdown 解析，最后按纯文本处理
        if isinstance(content, str):
            parsed_content = self._try_parse_json_content(content)
            if parsed_content is not None:
                self._process_structured_content(doc, parsed_content, color_coding)
            elif self._looks_like_markdown(content):
                self._process_markdown_content(doc, content, color_coding)
            else:
                for paragraph_text in content.split("\n"):
                    if paragraph_text.strip():
                        p = doc.add_paragraph()
                        run = p.add_run(paragraph_text)
                        self._set_run_fonts(run, self.LATIN_FONT, self.EAST_ASIAN_FONT)
        elif isinstance(content, list):
            self._process_structured_content(doc, content, color_coding)
        elif isinstance(content, dict):
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

        # 添加页脚
        if footer_text or page_number:
            self._add_footer(doc, footer_text or "", page_number)

        doc.save(path)
        self.logger.info("generate: Word 文档已生成, path=%s", path)
        return {
            "path": path,
            "message": f"Word 文档已生成: {path}",
        }

    # ------------------------------------------------------------------ #
    #  Markdown 解析（核心新增功能）
    # ------------------------------------------------------------------ #

    def _looks_like_markdown(self, content: str) -> bool:
        """判断内容是否看起来像 Markdown 格式"""
        if not content or not content.strip():
            return False
        md_patterns = [
            r'^#{1,6}\s+',           # 标题
            r'^\s*[-*+]\s+',         # 无序列表
            r'^\s*\d+\.\s+',         # 有序列表
            r'\*\*[^*]+\*\*',        # 粗体
            r'\*[^*]+\*',            # 斜体
            r'^\|.+\|$',             # 表格
            r'^```',                 # 代码块
            r'^---+$',               # 分隔线
        ]
        lines = content.split('\n')
        md_line_count = 0
        for line in lines[:50]:
            for pattern in md_patterns:
                if re.search(pattern, line):
                    md_line_count += 1
                    break
        # 超过 20% 的行匹配 Markdown 模式则判定为 Markdown
        return md_line_count > 0 and md_line_count / min(len(lines), 50) > 0.15

    def _process_markdown_content(self, doc: Document, content: str, color_coding: bool = True):
        """将 Markdown 内容解析并转换为专业 Word 元素"""
        lines = content.split('\n')
        i = 0
        while i < len(lines):
            line = lines[i]

            # 代码块处理
            if line.strip().startswith('```'):
                code_lines = []
                i += 1
                while i < len(lines) and not lines[i].strip().startswith('```'):
                    code_lines.append(lines[i])
                    i += 1
                i += 1  # 跳过结束的 ```
                if code_lines:
                    self._add_code_block(doc, '\n'.join(code_lines))
                continue

            # 标题处理
            heading_match = re.match(r'^(#{1,6})\s+(.+)$', line)
            if heading_match:
                level = len(heading_match.group(1))
                text = heading_match.group(2).strip()
                self._add_styled_heading(doc, level, text)
                i += 1
                continue

            # 分隔线
            if re.match(r'^-{3,}$|^\*{3,}$|^_{3,}$', line.strip()):
                self._add_horizontal_rule(doc)
                i += 1
                continue

            # 表格处理
            if '|' in line and i + 1 < len(lines) and re.match(r'^[\s|:-]+$', lines[i + 1]):
                table_lines = []
                while i < len(lines) and '|' in lines[i]:
                    table_lines.append(lines[i])
                    i += 1
                self._add_markdown_table(doc, table_lines)
                continue

            # 无序列表
            ul_match = re.match(r'^\s*[-*+]\s+(.+)$', line)
            if ul_match:
                items = []
                while i < len(lines):
                    m = re.match(r'^\s*[-*+]\s+(.+)$', lines[i])
                    if not m:
                        break
                    items.append(m.group(1))
                    i += 1
                self._add_styled_list(doc, items, ordered=False)
                continue

            # 有序列表
            ol_match = re.match(r'^\s*(\d+)\.\s+(.+)$', line)
            if ol_match:
                items = []
                while i < len(lines):
                    m = re.match(r'^\s*\d+\.\s+(.+)$', lines[i])
                    if not m:
                        break
                    items.append(m.group(1))
                    i += 1
                self._add_styled_list(doc, items, ordered=True)
                continue

            # 空行
            if not line.strip():
                i += 1
                continue

            # 普通段落（支持行内格式）
            self._add_rich_paragraph(doc, line)
            i += 1

    def _add_styled_heading(self, doc: Document, level: int, text: str):
        """添加带专业样式的标题"""
        heading = doc.add_heading(text, level=min(level, 4))
        # 设置标题颜色
        color_key = f"heading{min(level, 3)}"
        color = self.THEME_COLORS.get(color_key, self.THEME_COLORS["heading3"])
        for run in heading.runs:
            run.font.color.rgb = color
            self._set_run_fonts(run, self.LATIN_FONT, self.EAST_ASIAN_FONT)
        # 标题下方增加间距
        heading.paragraph_format.space_before = Pt(18 if level <= 2 else 12)
        heading.paragraph_format.space_after = Pt(8)

    def _add_rich_paragraph(self, doc: Document, text: str):
        """添加支持行内格式的段落（粗体、斜体、行内代码）"""
        p = doc.add_paragraph()
        self._parse_inline_format(p, text)
        p.paragraph_format.space_after = Pt(6)

    def _parse_inline_format(self, paragraph, text: str):
        """解析行内格式并添加到段落"""
        # 匹配粗体、斜体、行内代码的混合模式
        pattern = r'(\*\*(.+?)\*\*|\*(.+?)\*|`(.+?)`)'
        last_end = 0

        for match in re.finditer(pattern, text):
            # 添加匹配前的普通文本
            if match.start() > last_end:
                plain_text = text[last_end:match.start()]
                if plain_text:
                    run = paragraph.add_run(plain_text)
                    self._set_run_fonts(run, self.LATIN_FONT, self.EAST_ASIAN_FONT)

            if match.group(2):  # 粗体 **text**
                run = paragraph.add_run(match.group(2))
                run.font.bold = True
                self._set_run_fonts(run, self.LATIN_FONT, self.EAST_ASIAN_FONT)
            elif match.group(3):  # 斜体 *text*
                run = paragraph.add_run(match.group(3))
                run.font.italic = True
                self._set_run_fonts(run, self.LATIN_FONT, self.EAST_ASIAN_FONT)
            elif match.group(4):  # 行内代码 `text`
                run = paragraph.add_run(match.group(4))
                run.font.name = "Consolas"
                rPr = run._r.get_or_add_rPr()
                rFonts = rPr.find(qn('w:rFonts'))
                if rFonts is None:
                    rFonts = OxmlElement('w:rFonts')
                    rPr.insert(0, rFonts)
                rFonts.set(qn('w:eastAsia'), 'Consolas')
                run.font.size = Pt(10)
                run.font.color.rgb = RGBColor(0xC7, 0x25, 0x4E)
                # 浅灰色背景
                shd = OxmlElement('w:shd')
                shd.set(qn('w:val'), 'clear')
                shd.set(qn('w:color'), 'auto')
                shd.set(qn('w:fill'), 'F0F0F0')
                rPr.append(shd)

            last_end = match.end()

        # 添加剩余的普通文本
        if last_end < len(text):
            remaining = text[last_end:]
            if remaining:
                run = paragraph.add_run(remaining)
                self._set_run_fonts(run, self.LATIN_FONT, self.EAST_ASIAN_FONT)

        # 如果没有匹配任何格式，确保文本被添加
        if not paragraph.runs and text:
            run = paragraph.add_run(text)
            self._set_run_fonts(run, self.LATIN_FONT, self.EAST_ASIAN_FONT)

    def _add_styled_list(self, doc: Document, items: list, ordered: bool = False):
        """添加带专业样式的列表"""
        for item in items:
            if ordered:
                p = doc.add_paragraph(style="List Number")
            else:
                p = doc.add_paragraph(style="List Bullet")
            # 支持列表项中的行内格式
            self._parse_inline_format(p, item)
            # 设置列表项字体
            for run in p.runs:
                self._set_run_fonts(run, self.LATIN_FONT, self.EAST_ASIAN_FONT)
            p.paragraph_format.space_after = Pt(3)

    def _add_code_block(self, doc: Document, code: str):
        """添加代码块（带背景色和等宽字体）"""
        for line in code.split('\n'):
            p = doc.add_paragraph()
            run = p.add_run(line)
            run.font.name = "Consolas"
            rPr = run._r.get_or_add_rPr()
            rFonts = rPr.find(qn('w:rFonts'))
            if rFonts is None:
                rFonts = OxmlElement('w:rFonts')
                rPr.insert(0, rFonts)
            rFonts.set(qn('w:eastAsia'), 'Consolas')
            run.font.size = Pt(9)
            run.font.color.rgb = RGBColor(0x33, 0x33, 0x33)
            # 浅灰色背景
            shd = OxmlElement('w:shd')
            shd.set(qn('w:val'), 'clear')
            shd.set(qn('w:color'), 'auto')
            shd.set(qn('w:fill'), 'F5F5F5')
            rPr.append(shd)
            # 左缩进
            p.paragraph_format.left_indent = Cm(1)
            p.paragraph_format.space_after = Pt(1)
            p.paragraph_format.space_before = Pt(1)

    def _add_markdown_table(self, doc: Document, table_lines: list):
        """从 Markdown 表格行创建专业表格"""
        if len(table_lines) < 2:
            return

        # 解析表头
        headers = [cell.strip() for cell in table_lines[0].split('|') if cell.strip()]
        # 跳过分隔行（第二行）
        # 解析数据行
        data_rows = []
        for line in table_lines[2:]:
            cells = [cell.strip() for cell in line.split('|') if cell.strip()]
            if cells:
                data_rows.append(cells)

        num_cols = len(headers)
        if num_cols == 0:
            return

        num_rows = 1 + len(data_rows)

        table = doc.add_table(rows=num_rows, cols=num_cols)
        table.style = "Table Grid"
        table.alignment = WD_TABLE_ALIGNMENT.CENTER

        # 填充表头
        for j, header_text in enumerate(headers):
            if j < num_cols:
                cell = table.rows[0].cells[j]
                cell.text = ""
                p = cell.paragraphs[0]
                run = p.add_run(header_text)
                run.font.bold = True
                run.font.color.rgb = RGBColor(0x1F, 0x4E, 0x79)
                run.font.size = Pt(11)
                self._set_run_fonts(run, self.LATIN_FONT, self.EAST_ASIAN_FONT)
                p.alignment = WD_ALIGN_PARAGRAPH.CENTER
                # 表头背景色
                self._set_cell_shading(cell, self.THEME_COLORS["table_header_bg"])

        # 填充数据行
        for i, row_data in enumerate(data_rows):
            for j, cell_text in enumerate(row_data):
                if j < num_cols:
                    cell = table.rows[i + 1].cells[j]
                    cell.text = ""
                    p = cell.paragraphs[0]
                    run = p.add_run(cell_text)
                    run.font.size = Pt(10)
                    self._set_run_fonts(run, self.LATIN_FONT, self.EAST_ASIAN_FONT)
                    # 交替行背景色
                    if i % 2 == 1:
                        self._set_cell_shading(cell, self.THEME_COLORS["table_alt_row_bg"])

        # 设置表格边框
        self._set_table_borders(table, self.THEME_COLORS["table_border"], 1)

        # 表格后添加间距
        spacer = doc.add_paragraph()
        spacer.paragraph_format.space_before = Pt(6)

    def _add_horizontal_rule(self, doc: Document):
        """添加水平分隔线"""
        p = doc.add_paragraph()
        pPr = p._p.get_or_add_pPr()
        pBdr = OxmlElement('w:pBdr')
        bottom = OxmlElement('w:bottom')
        bottom.set(qn('w:val'), 'single')
        bottom.set(qn('w:sz'), '6')
        bottom.set(qn('w:space'), '1')
        bottom.set(qn('w:color'), 'B4C6E7')
        pBdr.append(bottom)
        pPr.append(pBdr)
        p.paragraph_format.space_after = Pt(6)

    def _set_cell_shading(self, cell, color_hex: str):
        """设置单元格背景色"""
        tc = cell._tc
        tcPr = tc.get_or_add_tcPr()
        shd = OxmlElement('w:shd')
        shd.set(qn('w:val'), 'clear')
        shd.set(qn('w:color'), 'auto')
        shd.set(qn('w:fill'), color_hex)
        tcPr.append(shd)

    def _set_run_fonts(self, run, latin_font: str, east_asian_font: str):
        """设置 run 的拉丁字体和东亚字体"""
        run.font.name = latin_font
        rPr = run._r.get_or_add_rPr()
        rFonts = rPr.find(qn('w:rFonts'))
        if rFonts is None:
            rFonts = OxmlElement('w:rFonts')
            rPr.insert(0, rFonts)
        rFonts.set(qn('w:eastAsia'), east_asian_font)
        rFonts.set(qn('w:ascii'), latin_font)
        rFonts.set(qn('w:hAnsi'), latin_font)

    # ------------------------------------------------------------------ #
    #  样式覆盖（专业配色方案）
    # ------------------------------------------------------------------ #

    def _apply_style_overrides(self, doc: Document):
        """应用专业样式覆盖

        - 拉丁字体: Arial，东亚字体: 微软雅黑
        - 标题1: 深蓝色 22pt 粗体
        - 标题2: 中蓝色 16pt 粗体
        - 标题3: 浅蓝色 14pt 粗体
        - 正文: 12pt, 行间距 1.5
        """
        # 设置默认字体
        style_normal = doc.styles["Normal"]
        style_normal.font.name = self.LATIN_FONT
        style_normal.font.size = Pt(12)
        style_normal.paragraph_format.line_spacing = 1.5
        # 设置东亚字体
        self._set_style_east_asian_font(style_normal, self.EAST_ASIAN_FONT)

        # 标题1: 深蓝色 22pt 粗体
        style_h1 = doc.styles["Heading 1"]
        style_h1.font.name = self.LATIN_FONT
        style_h1.font.size = Pt(22)
        style_h1.font.bold = True
        style_h1.font.color.rgb = self.THEME_COLORS["heading1"]
        style_h1.paragraph_format.space_before = Pt(24)
        style_h1.paragraph_format.space_after = Pt(8)
        self._set_style_east_asian_font(style_h1, self.EAST_ASIAN_FONT)

        # 标题2: 中蓝色 16pt 粗体
        style_h2 = doc.styles["Heading 2"]
        style_h2.font.name = self.LATIN_FONT
        style_h2.font.size = Pt(16)
        style_h2.font.bold = True
        style_h2.font.color.rgb = self.THEME_COLORS["heading2"]
        style_h2.paragraph_format.space_before = Pt(18)
        style_h2.paragraph_format.space_after = Pt(6)
        self._set_style_east_asian_font(style_h2, self.EAST_ASIAN_FONT)

        # 标题3: 浅蓝色 14pt 粗体
        style_h3 = doc.styles["Heading 3"]
        style_h3.font.name = self.LATIN_FONT
        style_h3.font.size = Pt(14)
        style_h3.font.bold = True
        style_h3.font.color.rgb = self.THEME_COLORS["heading3"]
        style_h3.paragraph_format.space_before = Pt(12)
        style_h3.paragraph_format.space_after = Pt(4)
        self._set_style_east_asian_font(style_h3, self.EAST_ASIAN_FONT)

        # 标题4: 深灰色 12pt 粗体
        style_h4 = doc.styles["Heading 4"]
        style_h4.font.name = self.LATIN_FONT
        style_h4.font.size = Pt(12)
        style_h4.font.bold = True
        style_h4.font.color.rgb = RGBColor(0x40, 0x40, 0x40)
        self._set_style_east_asian_font(style_h4, self.EAST_ASIAN_FONT)

        self.logger.debug("_apply_style_overrides: 已应用专业样式覆盖")

    def _set_style_east_asian_font(self, style, east_asian_font: str):
        """设置样式的东亚字体"""
        rPr = style.element.get_or_add_rPr()
        rFonts = rPr.find(qn('w:rFonts'))
        if rFonts is None:
            rFonts = OxmlElement('w:rFonts')
            rPr.insert(0, rFonts)
        rFonts.set(qn('w:eastAsia'), east_asian_font)
        rFonts.set(qn('w:ascii'), self.LATIN_FONT)
        rFonts.set(qn('w:hAnsi'), self.LATIN_FONT)

    # ------------------------------------------------------------------ #
    #  结构化内容处理（保留原有功能）
    # ------------------------------------------------------------------ #

    def _try_parse_json_content(self, content: str):
        """尝试将字符串内容解析为结构化 JSON"""
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
        """处理结构化内容块列表"""
        for block in blocks:
            if not isinstance(block, dict):
                continue
            self._add_content_block(doc, block, color_coding=color_coding)

    def _add_content_block(self, doc: Document, block: dict, color_coding: bool = True):
        """添加结构化内容块"""
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
        self._add_styled_heading(doc, level, text)
        if color_coding and "colorType" in block:
            color = self.COLOR_MAP.get(block["colorType"])
            if color:
                heading = doc.paragraphs[-1]
                for run in heading.runs:
                    run.font.color.rgb = color

    def _add_paragraph_block(self, doc: Document, block: dict, color_coding: bool):
        """添加段落块"""
        text = block.get("text", "")
        style = block.get("style", None)
        alignment = block.get("alignment", None)
        bold = block.get("bold", False)
        italic = block.get("italic", False)

        p = doc.add_paragraph()
        self._parse_inline_format(p, text)
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

        if bold or italic:
            for run in p.runs:
                if bold:
                    run.font.bold = True
                if italic:
                    run.font.italic = True

        if color_coding and "colorType" in block:
            color = self.COLOR_MAP.get(block["colorType"])
            if color:
                for run in p.runs:
                    run.font.color.rgb = color

    def _add_table_block(self, doc: Document, block: dict, color_coding: bool):
        """添加专业表格块"""
        headers = block.get("headers", [])
        rows_data = block.get("rows", [])
        data = block.get("data", [])
        table_width_dxa = block.get("width", None)
        col_widths_dxa = block.get("colWidths", [])

        if isinstance(rows_data, list) and rows_data and not isinstance(rows_data[0], (int, float)):
            if headers:
                all_rows = [headers] + rows_data
            else:
                all_rows = rows_data
            num_rows = len(all_rows)
            num_cols = max(len(r) for r in all_rows) if all_rows else 1
        else:
            num_rows = rows_data if isinstance(rows_data, int) else (len(data) if data else 1)
            num_cols = block.get("cols", 1)
            all_rows = data if data else [[""] * num_cols for _ in range(num_rows)]

        table = doc.add_table(rows=num_rows, cols=num_cols)
        table.style = "Table Grid"
        table.alignment = WD_TABLE_ALIGNMENT.CENTER

        if table_width_dxa:
            table.width = Emu(table_width_dxa * self.DXA_TO_EMU)

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
                    cell.text = ""
                    p = cell.paragraphs[0]
                    run = p.add_run(str(cell_text))
                    self._set_run_fonts(run, self.LATIN_FONT, self.EAST_ASIAN_FONT)
                    if col_widths_dxa and j < len(col_widths_dxa):
                        cell.width = Emu(col_widths_dxa[j] * self.DXA_TO_EMU)

        # 专业边框
        self._set_table_borders(table, self.THEME_COLORS["table_border"], 1)

        # 表头行样式：蓝色背景 + 粗体居中
        if headers and num_rows > 0:
            for cell in table.rows[0].cells:
                self._set_cell_shading(cell, self.THEME_COLORS["table_header_bg"])
                for paragraph in cell.paragraphs:
                    paragraph.alignment = WD_ALIGN_PARAGRAPH.CENTER
                    for run in paragraph.runs:
                        run.font.bold = True
                        run.font.color.rgb = RGBColor(0x1F, 0x4E, 0x79)

        # 交替行背景色
        start_row = 1 if headers else 0
        for i in range(start_row, num_rows):
            if (i - start_row) % 2 == 1:
                for cell in table.rows[i].cells:
                    self._set_cell_shading(cell, self.THEME_COLORS["table_alt_row_bg"])

        if color_coding and "colorType" in block:
            color = self.COLOR_MAP.get(block["colorType"])
            if color:
                for row in table.rows:
                    for cell in row.cells:
                        for paragraph in cell.paragraphs:
                            for run in paragraph.runs:
                                run.font.color.rgb = color

    def _add_list_block(self, doc: Document, block: dict, color_coding: bool):
        """添加列表块"""
        items = block.get("items", [])
        ordered = block.get("ordered", False)
        self._add_styled_list(doc, items, ordered)

        if color_coding and "colorType" in block:
            color = self.COLOR_MAP.get(block["colorType"])
            if color:
                # 对最后添加的列表项应用颜色
                for _ in items:
                    para = doc.paragraphs[-1]
                    for run in para.runs:
                        run.font.color.rgb = color

    def _add_image_block(self, doc: Document, block: dict):
        """添加图片块"""
        image_path = block.get("path", "")
        width = block.get("width", None)
        height = block.get("height", None)
        alt_text = block.get("altText", {})

        if not image_path or not os.path.exists(image_path):
            self.logger.warning("_add_image_block: 图片路径不存在: %s", image_path)
            return

        if width and height:
            pic = doc.add_picture(image_path, width=Inches(width), height=Inches(height))
        elif width:
            pic = doc.add_picture(image_path, width=Inches(width))
        elif height:
            pic = doc.add_picture(image_path, height=Inches(height))
        else:
            pic = doc.add_picture(image_path)

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
    #  表格边框设置
    # ------------------------------------------------------------------ #

    def _set_table_borders(self, table, color_hex: str = "CCCCCC", size_pt: int = 1):
        """设置表格边框样式"""
        tbl = table._tbl
        tblPr = tbl.tblPr if tbl.tblPr is not None else OxmlElement("w:tblPr")

        borders = OxmlElement("w:tblBorders")
        for border_name in ("top", "left", "bottom", "right", "insideH", "insideV"):
            border = OxmlElement(f"w:{border_name}")
            border.set(qn("w:val"), "single")
            border.set(qn("w:sz"), str(size_pt * 8))
            border.set(qn("w:space"), "0")
            border.set(qn("w:color"), color_hex)
            borders.append(border)

        existing_borders = tblPr.find(qn("w:tblBorders"))
        if existing_borders is not None:
            tblPr.remove(existing_borders)

        tblPr.append(borders)

    # ------------------------------------------------------------------ #
    #  页面尺寸设置
    # ------------------------------------------------------------------ #

    def _set_page_size(self, doc: Document, size: str):
        """设置文档页面尺寸"""
        size_lower = size.lower() if size else ""
        if size_lower not in self.PAGE_SIZES:
            self.logger.warning("_set_page_size: 不支持的页面尺寸: %s", size)
            return

        page_size = self.PAGE_SIZES[size_lower]
        section = doc.sections[0]
        section.page_width = Emu(page_size["width"] * self.DXA_TO_EMU)
        section.page_height = Emu(page_size["height"] * self.DXA_TO_EMU)

    # ------------------------------------------------------------------ #
    #  页眉页脚
    # ------------------------------------------------------------------ #

    def _add_header(self, doc: Document, text: str):
        """添加页眉"""
        section = doc.sections[0]
        header = section.header
        header.is_linked_to_previous = False
        if header.paragraphs:
            paragraph = header.paragraphs[0]
        else:
            paragraph = header.add_paragraph()
        paragraph.text = ""
        run = paragraph.add_run(text)
        run.font.size = Pt(9)
        run.font.color.rgb = RGBColor(0x80, 0x80, 0x80)
        self._set_run_fonts(run, self.LATIN_FONT, self.EAST_ASIAN_FONT)
        paragraph.alignment = WD_ALIGN_PARAGRAPH.CENTER

    def _add_footer(self, doc: Document, text: str, page_number: bool = True):
        """添加页脚（含可选页码域代码）"""
        section = doc.sections[0]
        footer = section.footer
        footer.is_linked_to_previous = False
        if footer.paragraphs:
            paragraph = footer.paragraphs[0]
        else:
            paragraph = footer.add_paragraph()
        paragraph.alignment = WD_ALIGN_PARAGRAPH.CENTER

        if text:
            run = paragraph.add_run(text)
            run.font.size = Pt(9)
            run.font.color.rgb = RGBColor(0x80, 0x80, 0x80)
            self._set_run_fonts(run, self.LATIN_FONT, self.EAST_ASIAN_FONT)

        if page_number:
            if text:
                paragraph.add_run(" ")
            run_begin = paragraph.add_run()
            fldChar_begin = OxmlElement("w:fldChar")
            fldChar_begin.set(qn("w:fldCharType"), "begin")
            run_begin._r.append(fldChar_begin)

            run_instr = paragraph.add_run()
            instrText = OxmlElement("w:instrText")
            instrText.set(qn("xml:space"), "preserve")
            instrText.text = " PAGE "
            run_instr._r.append(instrText)

            run_sep = paragraph.add_run()
            fldChar_sep = OxmlElement("w:fldChar")
            fldChar_sep.set(qn("w:fldCharType"), "separate")
            run_sep._r.append(fldChar_sep)

            run_placeholder = paragraph.add_run("1")

            run_end = paragraph.add_run()
            fldChar_end = OxmlElement("w:fldChar")
            fldChar_end.set(qn("w:fldCharType"), "end")
            run_end._r.append(fldChar_end)

    # ------------------------------------------------------------------ #
    #  书签和超链接
    # ------------------------------------------------------------------ #

    def _add_bookmark(self, paragraph, bookmark_id: str, text: str, numeric_id: int = 0):
        """在段落中添加书签"""
        bookmark_start = OxmlElement("w:bookmarkStart")
        bookmark_start.set(qn("w:id"), str(numeric_id))
        bookmark_start.set(qn("w:name"), bookmark_id)

        bookmark_end = OxmlElement("w:bookmarkEnd")
        bookmark_end.set(qn("w:id"), str(numeric_id))

        run = paragraph.add_run(text)

        run._r.addprevious(bookmark_start)
        run._r.addnext(bookmark_end)

    def _add_hyperlink(self, paragraph, text: str, url: str):
        """添加外部超链接"""
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

        color_elem = OxmlElement("w:color")
        color_elem.set(qn("w:val"), "2E75B6")
        rPr.append(color_elem)

        u_elem = OxmlElement("w:u")
        u_elem.set(qn("w:val"), "single")
        rPr.append(u_elem)

        new_run.append(rPr)

        t_elem = OxmlElement("w:t")
        t_elem.text = text
        new_run.append(t_elem)

        hyperlink.append(new_run)
        paragraph._p.append(hyperlink)

    def _add_internal_link(self, paragraph, text: str, anchor: str):
        """添加内部书签链接"""
        hyperlink = OxmlElement("w:hyperlink")
        hyperlink.set(qn("w:anchor"), anchor)

        new_run = OxmlElement("w:r")
        rPr = OxmlElement("w:rPr")

        color_elem = OxmlElement("w:color")
        color_elem.set(qn("w:val"), "2E75B6")
        rPr.append(color_elem)

        u_elem = OxmlElement("w:u")
        u_elem.set(qn("w:val"), "single")
        rPr.append(u_elem)

        new_run.append(rPr)

        t_elem = OxmlElement("w:t")
        t_elem.text = text
        new_run.append(t_elem)

        hyperlink.append(new_run)
        paragraph._p.append(hyperlink)

    # ------------------------------------------------------------------ #
    #  目录
    # ------------------------------------------------------------------ #

    def _add_toc(self, doc: Document):
        """添加目录（TOC 域代码）"""
        paragraph = doc.add_paragraph()

        run_begin = paragraph.add_run()
        fldChar_begin = OxmlElement("w:fldChar")
        fldChar_begin.set(qn("w:fldCharType"), "begin")
        run_begin._r.append(fldChar_begin)

        run_instr = paragraph.add_run()
        instrText = OxmlElement("w:instrText")
        instrText.set(qn("xml:space"), "preserve")
        instrText.text = ' TOC \\o "1-3" \\h \\z \\u '
        run_instr._r.append(instrText)

        run_sep = paragraph.add_run()
        fldChar_sep = OxmlElement("w:fldChar")
        fldChar_sep.set(qn("w:fldCharType"), "separate")
        run_sep._r.append(fldChar_sep)

        run_placeholder = paragraph.add_run('（请右键点击此处，选择"更新域"以生成目录）')

        run_end = paragraph.add_run()
        fldChar_end = OxmlElement("w:fldChar")
        fldChar_end.set(qn("w:fldCharType"), "end")
        run_end._r.append(fldChar_end)

    # ------------------------------------------------------------------ #
    #  格式转换辅助方法
    # ------------------------------------------------------------------ #

    def read(self, params: dict) -> dict:
        """读取 Word 文档内容"""
        path = params.get("path", "")
        if not path:
            self.logger.error("read: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("read: 开始读取 Word 文档, path=%s", path)

        doc = Document(path)

        paragraphs = []
        for para in doc.paragraphs:
            para_info = {
                "text": para.text,
                "style": para.style.name if para.style else None,
            }
            paragraphs.append(para_info)

        tables = []
        for table in doc.tables:
            table_data = []
            for row in table.rows:
                row_data = [cell.text for cell in row.cells]
                table_data.append(row_data)
            tables.append(table_data)

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
        """修改 Word 文档"""
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
                    index = op.get("index", 0)
                    new_text = op.get("text", "")
                    if 0 <= index < len(doc.paragraphs):
                        para = doc.paragraphs[index]
                        para.clear()
                        run = para.add_run(new_text)
                        self._set_run_fonts(run, self.LATIN_FONT, self.EAST_ASIAN_FONT)
                        modified_count += 1
                else:
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
                p = doc.add_paragraph()
                self._parse_inline_format(p, text)
                if style:
                    p.style = style
                modified_count += 1

            elif op_type == "add_heading":
                text = op.get("text", "")
                level = op.get("level", 1)
                self._add_styled_heading(doc, level, text)
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
                                cell = table.rows[i].cells[j]
                                cell.text = ""
                                p = cell.paragraphs[0]
                                run = p.add_run(str(cell_text))
                                self._set_run_fonts(run, self.LATIN_FONT, self.EAST_ASIAN_FONT)
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

            elif op_type == "setPageSize":
                size = op.get("size", "")
                if size:
                    self._set_page_size(doc, size)
                    modified_count += 1

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
        """格式转换"""
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
            content = None
        else:
            self.logger.error("convert: 不支持的目标格式: %s", target_format)
            return {"error": f"不支持的目标格式: {target_format}"}

        if content is None:
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
        """分析 Word 文档"""
        path = params.get("path", "")
        if not path:
            self.logger.error("analyze: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("analyze: 开始分析 Word 文档, path=%s", path)

        doc = Document(path)

        total_chars = sum(len(p.text) for p in doc.paragraphs)
        total_words = sum(len(p.text.split()) for p in doc.paragraphs)
        heading_count = sum(
            1 for p in doc.paragraphs
            if p.style and ("Heading" in p.style.name or "Title" in p.style.name)
        )

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
