"""PPT 文档处理器
基于 python-pptx 实现 PPT 文档的生成、读取、修改
"""

import os
import html
import logging
from typing import Any

from pptx import Presentation
from pptx.util import Inches, Pt, Emu
from pptx.enum.text import PP_ALIGN


class PptHandler:
    """PowerPoint (.pptx) 文档处理器"""

    logger = logging.getLogger(__name__)

    def generate(self, params: dict) -> dict:
        """生成 PPT 文档

        params:
            path: 输出文件路径
            slides: 幻灯片列表
                [{"title": "...", "content": "...", "layout": "title_slide"}]
            content: 文档内容（当 slides 为空时，从 content 构建）
            title: 文档标题（当 slides 为空时，作为标题幻灯片的标题）
        """
        path = params.get("path", "")
        slides = params.get("slides", [])
        content = params.get("content", "")
        title = params.get("title", "")
        if not path:
            self.logger.error("generate: 缺少输出文件路径")
            return {"error": "缺少输出文件路径"}

        # 当 slides 为空但 content 非空时，从 content 构建默认幻灯片
        if not slides and content:
            self.logger.info("generate: slides 为空，从 content 参数构建默认幻灯片")
            # 将 content 按段落拆分为多张幻灯片
            paragraphs = [p.strip() for p in content.split("\n") if p.strip()]
            # 将段落分组，每组最多 5 个段落作为一张幻灯片
            chunk_size = 5
            for i in range(0, len(paragraphs), chunk_size):
                chunk = paragraphs[i:i + chunk_size]
                slide_title = title if i == 0 and title else f"第 {i // chunk_size + 1} 页"
                slides.append({
                    "title": slide_title,
                    "content": "\n".join(chunk),
                    "layout": "title_slide",
                })

        self.logger.info("generate: 开始生成 PPT 文档, path=%s, 幻灯片数=%d", path, len(slides))

        os.makedirs(os.path.dirname(path) or ".", exist_ok=True)

        prs = Presentation()

        for slide_info in slides:
            title = slide_info.get("title", "")
            content = slide_info.get("content", "")
            layout_name = slide_info.get("layout", "title_slide")

            # 选择布局
            layout_idx = 0
            for i, layout in enumerate(prs.slide_layouts):
                if layout_name.lower() in layout.name.lower():
                    layout_idx = i
                    break

            slide_layout = prs.slide_layouts[layout_idx]
            slide = prs.slides.add_slide(slide_layout)

            # 设置标题
            if slide.shapes.title:
                slide.shapes.title.text = title

            # 设置内容
            if content and len(slide.placeholders) > 1:
                for placeholder in slide.placeholders:
                    if placeholder.placeholder_format.idx == 1:
                        placeholder.text = content
                        break

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
        """修改 PPT 文档"""
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

        # 提取每张幻灯片的文本内容
        prs = Presentation(path)
        slides_data = self._extract_slides_text(prs)

        # 根据目标格式进行转换
        if target_format == "pdf":
            # PDF 必须写入文件，若未指定 output_path 则自动生成
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

        # 写入输出文件或返回内容
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

    def _extract_slides_text(self, prs: Presentation) -> list[dict]:
        """从 PPT 中提取每张幻灯片的标题和内容文本

        返回:
            [{"title": "...", "content": "..."}, ...]
        """
        slides_data = []
        for slide in prs.slides:
            title = ""
            content_parts = []

            for shape in slide.shapes:
                if not shape.has_text_frame:
                    continue
                # 提取文本框中所有段落的文本
                texts = []
                for para in shape.text_frame.paragraphs:
                    if para.text.strip():
                        texts.append(para.text.strip())

                if not texts:
                    continue

                # 如果是标题占位符，作为幻灯片标题
                if shape.shape_type == 14:  # MSO_SHAPE_TYPE.PLACEHOLDER
                    ph_type = shape.placeholder_format.type
                    # 标题占位符类型: 1=标题, 2=正文, 3=中心标题, 4=副标题
                    if ph_type in (1, 3):
                        title = "\n".join(texts)
                        continue

                # 如果第一个有文本的 shape 没有被识别为标题占位符，
                # 且尚未设置标题，则将其作为标题
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
        """将幻灯片内容转换为 PDF

        使用 reportlab 将每张幻灯片的文本内容渲染到 PDF 页面
        """
        try:
            from reportlab.lib.pagesizes import A4
            from reportlab.lib.styles import getSampleStyleSheet, ParagraphStyle
            from reportlab.lib.units import cm
            from reportlab.platypus import SimpleDocTemplate, Paragraph, Spacer, PageBreak
        except ImportError:
            self.logger.error("convert: reportlab 未安装，无法转换为 PDF")
            raise RuntimeError("reportlab 未安装，无法转换为 PDF")

        os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)

        # 注册中文字体
        from handlers.font_utils import register_chinese_font
        font_name = register_chinese_font()

        doc = SimpleDocTemplate(output_path, pagesize=A4)
        styles = getSampleStyleSheet()

        # 幻灯片标题样式
        slide_title_style = ParagraphStyle(
            "SlideTitle",
            parent=styles["Title"],
            fontName=font_name,
            fontSize=20,
            spaceAfter=15,
        )

        # 幻灯片正文样式
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
            # 每张幻灯片之前添加分页（第一张除外）
            if i > 0:
                elements.append(PageBreak())

            # 添加幻灯片标题
            if slide["title"]:
                elements.append(Paragraph(html.escape(slide["title"]), slide_title_style))
                elements.append(Spacer(1, 0.5 * cm))

            # 添加幻灯片正文内容
            if slide["content"]:
                for line in slide["content"].split("\n"):
                    if line.strip():
                        elements.append(Paragraph(html.escape(line), slide_body_style))

            # 如果标题和内容都为空，添加占位文本
            if not slide["title"] and not slide["content"]:
                elements.append(Paragraph(f"(幻灯片 {i + 1} 无文本内容)", slide_body_style))

        doc.build(elements)
        return output_path

    def _convert_to_markdown(self, slides_data: list[dict]) -> str:
        """将幻灯片内容转换为 Markdown"""
        lines = []
        for i, slide in enumerate(slides_data):
            # 每张幻灯片用二级标题
            title = slide["title"] or f"幻灯片 {i + 1}"
            lines.append(f"## {title}")
            lines.append("")

            # 添加正文内容
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
            # 幻灯片之间用分隔线隔开
            if i > 0:
                parts.append("")
                parts.append("=" * 60)
                parts.append("")

            # 添加标题
            title = slide["title"] or f"幻灯片 {i + 1}"
            parts.append(title)
            parts.append("-" * 40)

            # 添加正文内容
            if slide["content"]:
                parts.append(slide["content"])

        return "\n".join(parts)

    def _convert_to_html(self, slides_data: list[dict]) -> str:
        """将幻灯片内容转换为 HTML"""
        sections = []
        for i, slide in enumerate(slides_data):
            title = slide["title"] or f"幻灯片 {i + 1}"

            # 构建 section 内容
            section_lines = [f"<section>"]
            section_lines.append(f"  <h2>{html.escape(title)}</h2>")

            if slide["content"]:
                section_lines.append("  <div>")
                for line in slide["content"].split("\n"):
                    if line.strip():
                        section_lines.append(f"    <p>{html.escape(line)}</p>")
                section_lines.append("  </div>")

            section_lines.append("</section>")
            sections.append("\n".join(section_lines))

        # 组装完整 HTML 文档
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
