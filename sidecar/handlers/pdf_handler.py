"""PDF 文档处理器
基于 reportlab 实现 PDF 生成，基于 pdfkit 实现 HTML 转 PDF
"""

import os
import html
import logging
from typing import Any


class PdfHandler:
    """PDF (.pdf) 文档处理器"""

    logger = logging.getLogger(__name__)

    def generate(self, params: dict) -> dict:
        """生成 PDF 文档

        params:
            path: 输出文件路径
            title: 文档标题
            content: 文档内容
            author: 作者
        """
        path = params.get("path", "")
        title = params.get("title", "")
        content = params.get("content", "")
        author = params.get("author", "")

        if not path:
            self.logger.error("generate: 缺少输出文件路径")
            return {"error": "缺少输出文件路径"}

        self.logger.info("generate: 开始生成 PDF 文档, path=%s", path)

        os.makedirs(os.path.dirname(path) or ".", exist_ok=True)

        try:
            from reportlab.lib.pagesizes import A4
            from reportlab.lib.styles import getSampleStyleSheet, ParagraphStyle
            from reportlab.lib.units import cm
            from reportlab.platypus import SimpleDocTemplate, Paragraph, Spacer
        except ImportError:
            self.logger.error("generate: reportlab 未安装，无法生成 PDF")
            return {"error": "reportlab 未安装，无法生成 PDF"}

        from handlers.font_utils import register_chinese_font
        font_name = register_chinese_font()

        doc = SimpleDocTemplate(path, pagesize=A4)
        styles = getSampleStyleSheet()

        # 自定义标题样式
        title_style = ParagraphStyle(
            "CustomTitle",
            parent=styles["Title"],
            fontName=font_name,
            fontSize=24,
            spaceAfter=30,
        )

        # 自定义正文样式
        body_style = ParagraphStyle(
            "CustomBody",
            parent=styles["Normal"],
            fontName=font_name,
            fontSize=12,
            leading=20,
            spaceAfter=10,
        )

        elements = []

        # 添加标题（Paragraph 使用 XML 标记语法，需转义特殊字符）
        if title:
            elements.append(Paragraph(html.escape(title), title_style))
            elements.append(Spacer(1, 1 * cm))

        # 添加内容
        if isinstance(content, str):
            for line in content.split("\n"):
                if line.strip():
                    elements.append(Paragraph(html.escape(line), body_style))
        elif isinstance(content, list):
            for item in content:
                if isinstance(item, str):
                    elements.append(Paragraph(html.escape(item), body_style))
                elif isinstance(item, dict):
                    text = item.get("text", "")
                    style_type = item.get("style", "body")
                    if style_type == "heading":
                        elements.append(Paragraph(html.escape(text), title_style))
                    else:
                        elements.append(Paragraph(html.escape(text), body_style))

        # 设置作者
        if author:
            doc.author = author

        doc.build(elements)

        self.logger.info("generate: PDF 文档已生成, path=%s", path)
        return {
            "path": path,
            "message": f"PDF 文档已生成: {path}",
        }

    def read(self, params: dict) -> dict:
        """读取 PDF 文档（提取文本）"""
        path = params.get("path", "")
        if not path:
            self.logger.error("read: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("read: 开始读取 PDF 文档, path=%s", path)

        try:
            import fitz  # PyMuPDF
            doc = fitz.open(path)
            pages = []
            for page in doc:
                pages.append({
                    "page_number": page.number + 1,
                    "text": page.get_text(),
                })
            doc.close()
            self.logger.info("read: PDF 文档读取完成, path=%s, 页数=%d", path, len(pages))
            return {
                "pages": pages,
                "page_count": len(pages),
            }
        except ImportError:
            # 回退方案：使用 pdfminer，统一返回结构
            try:
                from pdfminer.high_level import extract_text
                text = extract_text(path)
                # 将文本按换页符分割为页面，保持与 PyMuPDF 一致的返回结构
                page_texts = text.split("\f")
                pages = []
                for i, page_text in enumerate(page_texts):
                    if page_text.strip():
                        pages.append({
                            "page_number": i + 1,
                            "text": page_text,
                        })
                self.logger.info("read: PDF 文档读取完成(pdfminer), path=%s, 页数=%d", path, len(pages))
                return {
                    "pages": pages,
                    "page_count": len(pages),
                }
            except ImportError:
                self.logger.error("read: 未安装 PDF 读取库（PyMuPDF 或 pdfminer.six）")
                return {
                    "pages": [],
                    "page_count": 0,
                    "error": "未安装 PDF 读取库（PyMuPDF 或 pdfminer.six）",
                }

    def modify(self, params: dict) -> dict:
        """修改 PDF 文档（PDF 不易直接修改，建议转换为其他格式后修改）"""
        self.logger.error("modify: PDF 格式不支持直接修改")
        return {"error": "PDF 格式不支持直接修改，建议转换为 Word 后修改"}

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

        # PDF 只支持文本提取类转换，不支持转为二进制格式
        supported_formats = ("txt", "md", "markdown", "html")
        if target_format not in supported_formats:
            self.logger.error("convert: 不支持的目标格式: %s，PDF 仅支持转为 txt/md/html", target_format)
            return {"error": f"不支持的目标格式: {target_format}，PDF 仅支持转为 txt/md/html"}

        self.logger.info("convert: 开始格式转换, path=%s, format=%s", path, target_format)

        # 提取 PDF 各页文本
        pages = self._extract_pages(path)
        if pages is None:
            return {"error": "未安装 PDF 读取库（PyMuPDF 或 pdfminer.six）"}

        # 根据目标格式生成内容
        if target_format == "txt":
            content = self._convert_to_txt(pages)
        elif target_format in ("md", "markdown"):
            content = self._convert_to_md(pages)
        elif target_format == "html":
            content = self._convert_to_html(pages)

        # 写入输出文件或直接返回内容
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

    def _extract_pages(self, path: str) -> list[dict] | None:
        """提取 PDF 各页文本，优先使用 PyMuPDF，回退到 pdfminer

        返回:
            [{"page_number": 1, "text": "..."}, ...] 或 None（库未安装时）
        """
        try:
            import fitz  # PyMuPDF
            doc = fitz.open(path)
            pages = []
            for page in doc:
                pages.append({
                    "page_number": page.number + 1,
                    "text": page.get_text(),
                })
            doc.close()
            return pages
        except ImportError:
            # 回退方案：使用 pdfminer
            try:
                from pdfminer.high_level import extract_text
                text = extract_text(path)
                # 将文本按换页符分割为页面
                page_texts = text.split("\f")
                pages = []
                for i, page_text in enumerate(page_texts):
                    if page_text.strip():
                        pages.append({
                            "page_number": i + 1,
                            "text": page_text,
                        })
                return pages
            except ImportError:
                self.logger.error("convert: 未安装 PDF 读取库（PyMuPDF 或 pdfminer.six）")
                return None

    def _convert_to_txt(self, pages: list[dict]) -> str:
        """将页面文本列表转换为纯文本格式"""
        parts = []
        for page in pages:
            parts.append(page["text"])
        return "\n\n".join(parts)

    def _convert_to_md(self, pages: list[dict]) -> str:
        """将页面文本列表转换为 Markdown 格式，每页用 ## 标题分隔"""
        lines = []
        for page in pages:
            page_num = page["page_number"]
            text = page["text"].strip()
            if not text:
                continue
            lines.append(f"## 第 {page_num} 页")
            lines.append("")
            lines.append(text)
            lines.append("")
        return "\n".join(lines)

    def _convert_to_html(self, pages: list[dict]) -> str:
        """将页面文本列表转换为 HTML 格式，每页用 section 标签包裹，段落用 p 标签"""
        sections = []
        for page in pages:
            page_num = page["page_number"]
            text = page["text"].strip()
            if not text:
                continue
            # 将文本按空行分割为段落
            paragraphs = text.split("\n\n")
            para_tags = []
            for para in paragraphs:
                para = para.strip()
                if para:
                    # 将段落内换行替换为 <br>
                    para_html = html.escape(para).replace("\n", "<br>")
                    para_tags.append(f"<p>{para_html}</p>")
            section_content = "\n    ".join(para_tags)
            sections.append(
                f'<section data-page="{page_num}">\n'
                f"    {section_content}\n"
                f"</section>"
            )
        body = "\n\n".join(sections)
        return (
            "<!DOCTYPE html>\n"
            "<html lang=\"zh-CN\">\n"
            "<head>\n"
            '  <meta charset="UTF-8">\n'
            "  <title>PDF 转换结果</title>\n"
            "</head>\n"
            "<body>\n"
            f"{body}\n"
            "</body>\n"
            "</html>"
        )

    def analyze(self, params: dict) -> dict:
        """分析 PDF 文档"""
        path = params.get("path", "")
        if not path:
            self.logger.error("analyze: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("analyze: 开始分析 PDF 文档, path=%s", path)

        try:
            import fitz
            doc = fitz.open(path)
            info = {
                "file_size": os.path.getsize(path),
                "page_count": len(doc),
                "metadata": doc.metadata,
            }
            doc.close()
            self.logger.info("analyze: PDF 文档分析完成, path=%s, 页数=%d", path, info["page_count"])
            return info
        except ImportError:
            self.logger.error("analyze: 未安装 PyMuPDF")
            return {
                "file_size": os.path.getsize(path),
                "page_count": 0,
                "error": "未安装 PyMuPDF",
            }
