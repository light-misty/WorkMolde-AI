"""PPT 文档处理器
基于 python-pptx 实现 PPT 文档的生成、读取、修改
遵循 pptx Skill 规范：5种颜色方案、字体规范、间距规范、多种布局、避免常见错误
"""

import os
import html
import logging
from typing import Any, Optional

from pptx import Presentation
from pptx.util import Inches, Pt, Emu
from pptx.enum.text import PP_ALIGN
from pptx.dml.color import RGBColor


class PptHandler:
    """PowerPoint (.pptx) 文档处理器"""

    logger = logging.getLogger(__name__)

    # 颜色方案定义，遵循 Skill 规范
    # primary=主色, secondary=辅色, accent=强调色
    COLOR_SCHEMES = {
        "midnight": {
            "primary": "1E2761",
            "secondary": "CADCFC",
            "accent": "FFFFFF",
            "name": "Midnight Executive",
        },
        "forest": {
            "primary": "2C5F2D",
            "secondary": "97BC62",
            "accent": "F5F5F5",
            "name": "Forest & Moss",
        },
        "coral": {
            "primary": "F96167",
            "secondary": "F9E795",
            "accent": "2F3C7E",
            "name": "Coral Energy",
        },
        "ocean": {
            "primary": "065A82",
            "secondary": "1C7293",
            "accent": "21295C",
            "name": "Ocean Gradient",
        },
        "charcoal": {
            "primary": "36454F",
            "secondary": "F2F2F2",
            "accent": "212121",
            "name": "Charcoal Minimal",
        },
    }

    # 字体规范，遵循 Skill 规范
    FONT_SIZES = {
        "slide_title": Pt(44),    # 幻灯片标题 36-44pt 粗体
        "section_title": Pt(24),  # 节标题 20-24pt 粗体
        "body": Pt(16),           # 正文 14-16pt
        "note": Pt(12),           # 注释 10-12pt 淡色
    }

    # 间距规范，遵循 Skill 规范
    # 最小边距 0.5 inch，内容块间距 0.3-0.5 inch
    DEFAULT_MARGINS = {
        "top": 0.5,
        "right": 0.5,
        "bottom": 0.5,
        "left": 0.5,
    }

    # 内容块间距
    CONTENT_SPACING = Inches(0.4)

    def generate(self, params: dict) -> dict:
        """生成 PPT 文档

        遵循 Skill 规范：
        - 5种颜色方案
        - 字体规范：标题36-44pt粗体、节标题20-24pt粗体、正文14-16pt、注释10-12pt
        - 间距规范：最小边距0.5inch、内容块间距0.3-0.5inch
        - 多种布局：标题页（深色背景+强调色文字）、内容页（浅色背景+深色文字）
        - 避免常见错误：不重复布局、不居中正文、不默认蓝色、不创建纯文字幻灯片

        params:
            path: 输出文件路径
            slides: 幻灯片列表
                [{"title": "...", "content": "...", "layout": "title_slide"|"content_slide"|"section_slide",
                  "bullets": [...], "notes": "..."}]
            content: 文档内容（当 slides 为空时，从 content 构建）
            title: 文档标题（当 slides 为空时，作为标题幻灯片的标题）
            colorScheme: 颜色方案名称 "midnight"|"forest"|"coral"|"ocean"|"charcoal"
            fonts: 字体配置 {"title": "Arial Black", "body": "Arial"}
            margins: 边距配置 {"top": 0.5, "right": 0.5, "bottom": 0.5, "left": 0.5}（单位: inch）
        """
        path = params.get("path", "")
        slides = params.get("slides", [])
        content = params.get("content", "")
        title = params.get("title", "")
        color_scheme = params.get("colorScheme", "")
        fonts = params.get("fonts", {})
        margins = params.get("margins", {})

        if not path:
            self.logger.error("generate: 缺少输出文件路径")
            return {"error": "缺少输出文件路径"}

        # 当 slides 为空但 content 非空时，从 content 构建默认幻灯片
        if not slides and content:
            self.logger.info("generate: slides 为空，从 content 参数构建默认幻灯片")
            paragraphs = [p.strip() for p in content.split("\n") if p.strip()]
            chunk_size = 5
            for i in range(0, len(paragraphs), chunk_size):
                chunk = paragraphs[i:i + chunk_size]
                slide_title = title if i == 0 and title else f"第 {i // chunk_size + 1} 页"
                slides.append({
                    "title": slide_title,
                    "content": "\n".join(chunk),
                    "layout": "title_slide" if i == 0 else "content_slide",
                })

        self.logger.info("generate: 开始生成 PPT 文档, path=%s, 幻灯片数=%d", path, len(slides))

        os.makedirs(os.path.dirname(path) or ".", exist_ok=True)

        prs = Presentation()

        # 解析边距参数，默认最小边距 0.5 inch
        margin_top = Inches(margins.get("top", self.DEFAULT_MARGINS["top"]))
        margin_right = Inches(margins.get("right", self.DEFAULT_MARGINS["right"]))
        margin_bottom = Inches(margins.get("bottom", self.DEFAULT_MARGINS["bottom"]))
        margin_left = Inches(margins.get("left", self.DEFAULT_MARGINS["left"]))

        # 解析字体参数
        title_font = fonts.get("title", "")
        body_font = fonts.get("body", "")

        # 解析颜色方案
        scheme_colors = self.COLOR_SCHEMES.get(color_scheme, None)

        for slide_idx, slide_info in enumerate(slides):
            slide_title = slide_info.get("title", "")
            slide_content = slide_info.get("content", "")
            layout_name = slide_info.get("layout", "")
            bullets = slide_info.get("bullets", [])
            notes = slide_info.get("notes", "")

            # 自动判断布局类型
            is_title_slide = (slide_idx == 0) if not layout_name else (layout_name == "title_slide")
            is_section_slide = layout_name == "section_slide"

            # 选择布局
            layout_idx = 0
            if layout_name:
                for i, layout in enumerate(prs.slide_layouts):
                    if layout_name.lower() in layout.name.lower():
                        layout_idx = i
                        break

            slide_layout = prs.slide_layouts[layout_idx]
            slide = prs.slides.add_slide(slide_layout)

            # 应用颜色方案到幻灯片
            if scheme_colors:
                self._apply_color_scheme(slide, color_scheme, is_title_slide)

            # 计算文本框位置和大小（考虑边距）
            slide_width = prs.slide_width
            slide_height = prs.slide_height
            text_left = margin_left
            text_width = slide_width - margin_left - margin_right

            # 设置标题
            if slide.shapes.title:
                slide.shapes.title.text = slide_title
                # 标题字体大小和颜色
                title_color = None
                title_size = self.FONT_SIZES["slide_title"]
                if scheme_colors:
                    if is_title_slide or is_section_slide:
                        # 标题页/节标题页：强调色文字
                        title_color = scheme_colors["accent"]
                    else:
                        # 内容页：使用主色（深色）
                        title_color = scheme_colors["primary"]
                if is_section_slide:
                    title_size = self.FONT_SIZES["section_title"]
                self._set_shape_font(
                    slide.shapes.title,
                    font_name=title_font or None,
                    font_size=title_size,
                    color=title_color,
                    bold=True,
                )
            else:
                # 手动添加标题文本框
                if slide_title:
                    title_color = None
                    title_size = self.FONT_SIZES["slide_title"]
                    if scheme_colors:
                        if is_title_slide or is_section_slide:
                            title_color = scheme_colors["accent"]
                        else:
                            title_color = scheme_colors["primary"]
                    if is_section_slide:
                        title_size = self.FONT_SIZES["section_title"]
                    self._add_text_box(
                        slide,
                        text=slide_title,
                        left=text_left,
                        top=margin_top,
                        width=text_width,
                        height=Inches(1.5),
                        font_name=title_font or None,
                        font_size=title_size,
                        color=title_color,
                        bold=True,
                        alignment=PP_ALIGN.LEFT,
                    )

            # 设置内容
            body_color = None
            if scheme_colors:
                if is_title_slide:
                    # 标题页：强调色文字
                    body_color = scheme_colors["accent"]
                else:
                    # 内容页：使用主色（深色文字）
                    body_color = scheme_colors["primary"]

            # 优先使用 bullets 列表（避免纯文字幻灯片）
            if bullets:
                content_top = margin_top + Inches(1.8)
                content_height = slide_height - content_top - margin_bottom
                bullet_text = "\n".join(bullets)
                self._add_text_box(
                    slide,
                    text=bullet_text,
                    left=text_left,
                    top=content_top,
                    width=text_width,
                    height=content_height,
                    font_name=body_font or None,
                    font_size=self.FONT_SIZES["body"],
                    color=body_color,
                    alignment=PP_ALIGN.LEFT,
                )
            elif slide_content:
                if len(slide.placeholders) > 1:
                    for placeholder in slide.placeholders:
                        if placeholder.placeholder_format.idx == 1:
                            placeholder.text = slide_content
                            self._set_shape_font(
                                placeholder,
                                font_name=body_font or None,
                                font_size=self.FONT_SIZES["body"],
                                color=body_color,
                            )
                            break
                else:
                    content_top = margin_top + Inches(1.8)
                    content_height = slide_height - content_top - margin_bottom
                    self._add_text_box(
                        slide,
                        text=slide_content,
                        left=text_left,
                        top=content_top,
                        width=text_width,
                        height=content_height,
                        font_name=body_font or None,
                        font_size=self.FONT_SIZES["body"],
                        color=body_color,
                        alignment=PP_ALIGN.LEFT,
                    )

            # 添加注释（10-12pt 淡色）
            if notes:
                note_top = slide_height - margin_bottom - Inches(0.6)
                note_color = None
                if scheme_colors:
                    # 注释使用辅色（淡色）
                    note_color = scheme_colors["secondary"]
                self._add_text_box(
                    slide,
                    text=notes,
                    left=text_left,
                    top=note_top,
                    width=text_width,
                    height=Inches(0.5),
                    font_name=body_font or None,
                    font_size=self.FONT_SIZES["note"],
                    color=note_color,
                    alignment=PP_ALIGN.LEFT,
                )

        prs.save(path)
        self.logger.info("generate: PPT 文档已生成, path=%s, 幻灯片数=%d", path, len(slides))
        return {
            "path": path,
            "slide_count": len(slides),
            "message": f"PPT 文档已生成: {path}",
        }

    def read(self, params: dict) -> dict:
        """读取 PPT 文档"""
        path = params.get("path", "")
        if not path:
            self.logger.error("read: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("read: 开始读取 PPT 文档, path=%s", path)

        prs = Presentation(path)
        slides = []
        for slide in prs.slides:
            slide_info = {
                "shapes": [],
            }
            for shape in slide.shapes:
                shape_info = {
                    "name": shape.name,
                    "type": str(shape.shape_type),
                }
                if shape.has_text_frame:
                    texts = []
                    for para in shape.text_frame.paragraphs:
                        texts.append(para.text)
                    shape_info["text"] = "\n".join(texts)
                slide_info["shapes"].append(shape_info)
            slides.append(slide_info)

        self.logger.info("read: PPT 文档读取完成, path=%s, 幻灯片数=%d", path, len(slides))
        return {
            "slides": slides,
            "slide_count": len(slides),
        }

    def modify(self, params: dict) -> dict:
        """修改 PPT 文档

        params:
            path: 文件路径
            operations: 操作列表，支持以下操作类型:
                - add_slide: 添加幻灯片
                - replace_text: 替换文本
                - applyColorScheme: 应用颜色方案 {type, scheme}
                - setFont: 设置字体 {type, element, font, size}
                - setMargins: 设置边距 {type, top, right, bottom, left}
                - setSlideBackground: 设置幻灯片背景色 {type, slideIndex, color}
        """
        path = params.get("path", "")
        operations = params.get("operations", [])
        if not path:
            self.logger.error("modify: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("modify: 开始修改 PPT 文档, path=%s, 操作数=%d", path, len(operations))

        prs = Presentation(path)
        modified_count = 0

        for op in operations:
            op_type = op.get("type", "")

            if op_type == "add_slide":
                title = op.get("title", "")
                content = op.get("content", "")
                layout_idx = op.get("layout_index", 1)
                if layout_idx < len(prs.slide_layouts):
                    slide = prs.slides.add_slide(prs.slide_layouts[layout_idx])
                    if slide.shapes.title:
                        slide.shapes.title.text = title
                    modified_count += 1

            elif op_type == "replace_text":
                old_text = op.get("old", "")
                new_text = op.get("new", "")
                for slide in prs.slides:
                    for shape in slide.shapes:
                        if shape.has_text_frame:
                            for para in shape.text_frame.paragraphs:
                                for run in para.runs:
                                    if old_text in run.text:
                                        run.text = run.text.replace(old_text, new_text)
                                        modified_count += 1

            elif op_type == "applyColorScheme":
                scheme_name = op.get("scheme", "")
                if scheme_name not in self.COLOR_SCHEMES:
                    self.logger.warning("modify: 未知颜色方案: %s", scheme_name)
                    continue
                for slide_idx, slide in enumerate(prs.slides):
                    is_title = (slide_idx == 0)
                    self._apply_color_scheme(slide, scheme_name, is_title)
                    modified_count += 1

            elif op_type == "setFont":
                element = op.get("element", "body")
                font_name = op.get("font", "")
                font_size = op.get("size", 0)
                color_hex = op.get("color", "")

                for slide in prs.slides:
                    for shape in slide.shapes:
                        if not shape.has_text_frame:
                            continue
                        is_title_shape = (
                            shape == slide.shapes.title
                            or (hasattr(shape, "placeholder_format")
                                and hasattr(shape.placeholder_format, "type")
                                and shape.placeholder_format.type in (1, 3))
                        )
                        if element == "title" and not is_title_shape:
                            continue
                        if element == "body" and is_title_shape:
                            continue

                        size_pt = Pt(font_size) if font_size else None
                        color_rgb = color_hex if color_hex else None
                        self._set_shape_font(
                            shape,
                            font_name=font_name or None,
                            font_size=size_pt,
                            color=color_rgb,
                        )
                        modified_count += 1

            elif op_type == "setMargins":
                margin_top = op.get("top", 1.0)
                margin_right = op.get("right", 1.0)
                margin_bottom = op.get("bottom", 1.0)
                margin_left = op.get("left", 1.0)

                slide_width = prs.slide_width
                new_left = Inches(margin_left)
                new_width = slide_width - Inches(margin_left) - Inches(margin_right)

                for slide in prs.slides:
                    for shape in slide.shapes:
                        if not shape.has_text_frame:
                            continue
                        is_title_shape = (
                            shape == slide.shapes.title
                            or (hasattr(shape, "placeholder_format")
                                and hasattr(shape.placeholder_format, "type")
                                and shape.placeholder_format.type in (1, 3))
                        )
                        shape.left = new_left
                        shape.width = new_width
                        if is_title_shape:
                            shape.top = Inches(margin_top)
                        else:
                            shape.top = max(shape.top, Inches(margin_top) + Inches(1.5))
                        modified_count += 1

            elif op_type == "setSlideBackground":
                slide_index = op.get("slideIndex", -1)
                bg_color = op.get("color", "")
                if slide_index < 0 or slide_index >= len(prs.slides):
                    self.logger.warning("modify: 幻灯片索引越界: %d", slide_index)
                    continue
                if not bg_color:
                    self.logger.warning("modify: 未指定背景颜色")
                    continue
                slide = prs.slides[slide_index]
                self._set_slide_background(slide, bg_color)
                modified_count += 1

        prs.save(path)
        self.logger.info("modify: PPT 文档修改完成, path=%s, 修改数=%d", path, modified_count)
        return {
            "path": path,
            "modified_count": modified_count,
            "message": f"已执行 {modified_count} 项修改",
        }

    def convert(self, params: dict) -> dict:
        """格式转换

        params:
            path: 源文件路径
            output_path: 输出文件路径（可选）
            format: 目标格式（pdf, md, txt, html）
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

        prs = Presentation(path)
        slides_data = self._extract_slides_text(prs)

        if target_format == "pdf":
            if not output_path:
                base = os.path.splitext(path)[0]
                output_path = base + ".pdf"
            self._convert_to_pdf(slides_data, output_path)
            self.logger.info("convert: PPT 转 PDF 完成, output_path=%s", output_path)
            return {
                "path": output_path,
                "format": target_format,
                "message": f"已转换为 {target_format} 格式",
            }

        elif target_format in ("md", "markdown"):
            content = self._convert_to_markdown(slides_data)

        elif target_format == "txt":
            content = self._convert_to_txt(slides_data)

        elif target_format == "html":
            content = self._convert_to_html(slides_data)

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
        """分析 PPT 文档"""
        path = params.get("path", "")
        if not path:
            self.logger.error("analyze: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("analyze: 开始分析 PPT 文档, path=%s", path)

        prs = Presentation(path)
        self.logger.info("analyze: PPT 文档分析完成, path=%s, 幻灯片数=%d", path, len(prs.slides))
        return {
            "file_size": os.path.getsize(path),
            "slide_count": len(prs.slides),
            "width": prs.slide_width,
            "height": prs.slide_height,
        }

    # ------------------------------------------------------------------ #
    #  颜色方案应用（Skill 规范）
    # ------------------------------------------------------------------ #

    def _set_slide_background(self, slide, color_hex: str):
        """设置幻灯片背景色

        Args:
            slide: pptx.slide 对象
            color_hex: 十六进制颜色值，如 "1E2761"
        """
        background = slide.background
        fill = background.fill
        fill.solid()
        fill.fore_color.rgb = RGBColor.from_string(color_hex)

    def _apply_color_scheme(self, slide, scheme_name: str, is_title_slide: bool = False):
        """应用颜色方案到幻灯片

        遵循 Skill 规范：
        - 标题页：深色背景（主色）+ 强调色文字
        - 内容页：浅色背景（辅色）+ 深色文字（主色或强调色）
        - 一种颜色占主导（60-70% 视觉权重）

        Args:
            slide: pptx.slide 对象
            scheme_name: 颜色方案名称
            is_title_slide: 是否为标题页
        """
        scheme = self.COLOR_SCHEMES.get(scheme_name)
        if not scheme:
            self.logger.warning("_apply_color_scheme: 未知颜色方案: %s", scheme_name)
            return

        if is_title_slide:
            # 标题页: 主色背景（深色）
            self._set_slide_background(slide, scheme["primary"])
            # 标题使用强调色
            if slide.shapes.title:
                self._set_shape_font(
                    slide.shapes.title,
                    color=scheme["accent"],
                    font_size=self.FONT_SIZES["slide_title"],
                    bold=True,
                )
        else:
            # 内容页: 辅色背景（浅色）
            self._set_slide_background(slide, scheme["secondary"])
            # 标题使用主色（深色）
            if slide.shapes.title:
                self._set_shape_font(
                    slide.shapes.title,
                    color=scheme["primary"],
                    font_size=self.FONT_SIZES["section_title"],
                    bold=True,
                )
            # 正文使用主色（深色）
            for shape in slide.shapes:
                if shape.has_text_frame and shape != slide.shapes.title:
                    self._set_shape_font(
                        shape,
                        color=scheme["primary"],
                        font_size=self.FONT_SIZES["body"],
                    )

    # ------------------------------------------------------------------ #
    #  字体和文本框辅助方法
    # ------------------------------------------------------------------ #

    def _set_shape_font(self, shape, font_name: Optional[str] = None,
                        font_size=None, color: Optional[str] = None,
                        bold: bool = False):
        """设置形状文本的字体属性

        Args:
            shape: pptx.shape 对象
            font_name: 字体名称
            font_size: 字号
            color: 十六进制颜色值
            bold: 是否粗体
        """
        if not shape.has_text_frame:
            return
        for para in shape.text_frame.paragraphs:
            if not para.runs:
                if para.text:
                    run = para.add_run()
                    run.text = para.text
                    para.text = ""
                else:
                    continue
            for run in para.runs:
                if font_name:
                    run.font.name = font_name
                if font_size:
                    run.font.size = font_size
                if color:
                    run.font.color.rgb = RGBColor.from_string(color)
                if bold:
                    run.font.bold = True

    def _add_text_box(self, slide, text: str,
                      left, top, width, height,
                      font_name: Optional[str] = None,
                      font_size=None,
                      color: Optional[str] = None,
                      bold: bool = False,
                      alignment=PP_ALIGN.LEFT):
        """添加文本框到幻灯片

        Args:
            slide: pptx.slide 对象
            text: 文本内容
            left: 左边距
            top: 上边距
            width: 宽度
            height: 高度
            font_name: 字体名称
            font_size: 字号
            color: 十六进制颜色值
            bold: 是否粗体
            alignment: 对齐方式
        """
        txBox = slide.shapes.add_textbox(left, top, width, height)
        tf = txBox.text_frame
        tf.word_wrap = True

        # 按换行符拆分段落
        lines = text.split("\n")
        for i, line in enumerate(lines):
            if i == 0:
                para = tf.paragraphs[0]
            else:
                para = tf.add_paragraph()
            para.alignment = alignment
            # 内容块间距
            para.space_after = self.CONTENT_SPACING
            run = para.add_run()
            run.text = line
            if font_name:
                run.font.name = font_name
            if font_size:
                run.font.size = font_size
            if color:
                run.font.color.rgb = RGBColor.from_string(color)
            if bold:
                run.font.bold = True

        return txBox

    # ------------------------------------------------------------------ #
    #  转换相关私有方法
    # ------------------------------------------------------------------ #

    def _extract_slides_text(self, prs: Presentation) -> list[dict]:
        """从 PPT 中提取每张幻灯片的标题和内容文本"""
        slides_data = []
        for slide in prs.slides:
            title = ""
            content_parts = []

            for shape in slide.shapes:
                if not shape.has_text_frame:
                    continue
                texts = []
                for para in shape.text_frame.paragraphs:
                    if para.text.strip():
                        texts.append(para.text.strip())

                if not texts:
                    continue

                if shape.shape_type == 14:
                    ph_type = shape.placeholder_format.type
                    if ph_type in (1, 3):
                        title = "\n".join(texts)
                        continue

                if not title and shape == slide.shapes[0]:
                    title = "\n".join(texts)
                else:
                    content_parts.extend(texts)

            slides_data.append({
                "title": title,
                "content": "\n".join(content_parts),
            })
        return slides_data

    def _convert_to_pdf(self, slides_data: list[dict], output_path: str) -> str:
        """将幻灯片内容转换为 PDF"""
        try:
            from reportlab.lib.pagesizes import A4
            from reportlab.lib.styles import getSampleStyleSheet, ParagraphStyle
            from reportlab.lib.units import cm
            from reportlab.platypus import SimpleDocTemplate, Paragraph, Spacer, PageBreak
        except ImportError:
            self.logger.error("convert: reportlab 未安装，无法转换为 PDF")
            raise RuntimeError("reportlab 未安装，无法转换为 PDF")

        os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)

        from handlers.font_utils import register_chinese_font
        font_name = register_chinese_font()

        doc = SimpleDocTemplate(output_path, pagesize=A4)
        styles = getSampleStyleSheet()

        slide_title_style = ParagraphStyle(
            "SlideTitle",
            parent=styles["Title"],
            fontName=font_name,
            fontSize=20,
            spaceAfter=15,
        )

        slide_body_style = ParagraphStyle(
            "SlideBody",
            parent=styles["Normal"],
            fontName=font_name,
            fontSize=12,
            leading=20,
            spaceAfter=8,
        )

        elements = []
        for i, slide in enumerate(slides_data):
            if i > 0:
                elements.append(PageBreak())

            if slide["title"]:
                elements.append(Paragraph(html.escape(slide["title"]), slide_title_style))
                elements.append(Spacer(1, 0.5 * cm))

            if slide["content"]:
                for line in slide["content"].split("\n"):
                    if line.strip():
                        elements.append(Paragraph(html.escape(line), slide_body_style))

            if not slide["title"] and not slide["content"]:
                elements.append(Paragraph(f"(幻灯片 {i + 1} 无文本内容)", slide_body_style))

        doc.build(elements)
        return output_path

    def _convert_to_markdown(self, slides_data: list[dict]) -> str:
        """将幻灯片内容转换为 Markdown"""
        lines = []
        for i, slide in enumerate(slides_data):
            title = slide["title"] or f"幻灯片 {i + 1}"
            lines.append(f"## {title}")
            lines.append("")

            if slide["content"]:
                for line in slide["content"].split("\n"):
                    if line.strip():
                        lines.append(line)
                lines.append("")

        return "\n".join(lines)

    def _convert_to_txt(self, slides_data: list[dict]) -> str:
        """将幻灯片内容转换为纯文本"""
        parts = []
        for i, slide in enumerate(slides_data):
            if i > 0:
                parts.append("")
                parts.append("=" * 60)
                parts.append("")

            title = slide["title"] or f"幻灯片 {i + 1}"
            parts.append(title)
            parts.append("-" * 40)

            if slide["content"]:
                parts.append(slide["content"])

        return "\n".join(parts)

    def _convert_to_html(self, slides_data: list[dict]) -> str:
        """将幻灯片内容转换为 HTML"""
        sections = []
        for i, slide in enumerate(slides_data):
            title = slide["title"] or f"幻灯片 {i + 1}"

            section_lines = ["<section>"]
            section_lines.append(f"  <h2>{html.escape(title)}</h2>")

            if slide["content"]:
                section_lines.append("  <div>")
                for line in slide["content"].split("\n"):
                    if line.strip():
                        section_lines.append(f"    <p>{html.escape(line)}</p>")
                section_lines.append("  </div>")

            section_lines.append("</section>")
            sections.append("\n".join(section_lines))

        html_doc = f"""<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>PPT Content</title>
  <style>
    body {{ font-family: "Microsoft YaHei", "SimSun", sans-serif; max-width: 960px; margin: 0 auto; padding: 20px; }}
    section {{ margin-bottom: 40px; padding: 20px; border: 1px solid #ddd; border-radius: 8px; }}
    h2 {{ color: #333; border-bottom: 2px solid #4a90d9; padding-bottom: 8px; }}
    p {{ line-height: 1.8; color: #555; }}
  </style>
</head>
<body>
{chr(10).join(sections)}
</body>
</html>"""
        return html_doc
