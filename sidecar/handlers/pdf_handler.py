"""PDF 文档处理器
基于 reportlab + pypdf 实现 PDF 文档的读取、转换、分析
精简版：仅支持 read/convert/analyze 操作
"""

import os
import html
import logging


class PdfHandler:
    """PDF 文档处理器（精简版，仅支持 read/convert/analyze）"""

    logger = logging.getLogger(__name__)

    def read(self, params: dict) -> dict:
        """读取 PDF 文档内容

        params:
            path: 文件路径
            pages: 页码范围字符串，如 "1-5,8,10-12"（可选，默认读取所有）
            include_layout: 是否提取文本位置和样式（字号/字体/颜色），使用 PyMuPDF
            include_forms: 是否提取表单字段（AcroForm），使用 pypdf
            include_annotations: 是否提取注释（高亮/批注/签名等），使用 pypdf
            extract_tables: 是否提取表格结构，使用 pdfplumber
            include_images: 是否提取图片信息（数量/位置/尺寸），使用 PyMuPDF
        """
        path = params.get("path", "")
        pages_str = params.get("pages", None)
        if not path:
            self.logger.error("read: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("read: 开始读取 PDF 文档, path=%s", path)

        # 解析扩展参数
        include_layout = params.get("include_layout", False)
        include_forms = params.get("include_forms", False)
        include_annotations = params.get("include_annotations", False)
        extract_tables = params.get("extract_tables", False)
        include_images = params.get("include_images", False)

        try:
            import pdfplumber
        except ImportError:
            self.logger.error("read: pdfplumber 未安装，无法读取 PDF")
            return {"error": "pdfplumber 未安装"}

        # 解析页码范围字符串为页码列表（1-based）
        total_pages = 0
        with pdfplumber.open(path) as pdf:
            total_pages = len(pdf.pages)

        page_numbers = self._parse_page_ranges(pages_str, total_pages)

        text_content = []
        tables_by_page = []
        layout_by_page = []
        images_by_page = []

        # ------------------------------------------------------------------ #
        #  基本文本提取 + 表格提取（pdfplumber）
        # ------------------------------------------------------------------ #
        with pdfplumber.open(path) as pdf:
            for page_num in page_numbers:
                idx = page_num - 1  # 转为 0-based 索引
                page = pdf.pages[idx]
                page_text = page.extract_text() or ""
                page_info = {
                    "page": page_num,
                    "text": page_text,
                }
                text_content.append(page_info)

                # 表格提取
                if extract_tables:
                    try:
                        tables = page.extract_tables() or []
                        tables_by_page.append({
                            "page": page_num,
                            "tables": tables,
                        })
                    except Exception as e:
                        self.logger.warning("read: 第 %d 页表格提取失败: %s", page_num, e)
                        tables_by_page.append({"page": page_num, "tables": [], "error": str(e)})

        # ------------------------------------------------------------------ #
        #  文本布局和样式提取 + 图片信息（PyMuPDF / fitz）
        # ------------------------------------------------------------------ #
        if include_layout or include_images:
            try:
                import fitz  # PyMuPDF
                doc = fitz.open(path)
                for page_num in page_numbers:
                    idx = page_num - 1
                    page = doc[idx]

                    # 文本布局和样式
                    if include_layout:
                        try:
                            text_dict = page.get_text("dict")
                            layout_info = self._extract_layout_from_fitz(text_dict)
                            layout_by_page.append({
                                "page": page_num,
                                "blocks": layout_info,
                            })
                        except Exception as e:
                            self.logger.warning("read: 第 %d 页布局提取失败: %s", page_num, e)
                            layout_by_page.append({"page": page_num, "blocks": [], "error": str(e)})

                    # 图片信息
                    if include_images:
                        try:
                            image_list = page.get_images(full=True)
                            images_info = []
                            for img_idx, img in enumerate(image_list):
                                # img 是元组 (xref, smask, width, height, bpc, colorspace, ...)
                                images_info.append({
                                    "index": img_idx,
                                    "xref": img[0],
                                    "width": img[2] if len(img) > 2 else None,
                                    "height": img[3] if len(img) > 3 else None,
                                })
                            images_by_page.append({
                                "page": page_num,
                                "image_count": len(images_info),
                                "images": images_info,
                            })
                        except Exception as e:
                            self.logger.warning("read: 第 %d 页图片提取失败: %s", page_num, e)
                            images_by_page.append({"page": page_num, "image_count": 0, "images": [], "error": str(e)})

                doc.close()
            except ImportError:
                self.logger.warning("read: PyMuPDF 未安装，布局和图片提取不可用")
            except Exception as e:
                self.logger.warning("read: PyMuPDF 提取失败: %s", e)

        # ------------------------------------------------------------------ #
        #  表单字段提取（pypdf）
        # ------------------------------------------------------------------ #
        forms_info = []
        if include_forms:
            try:
                from pypdf import PdfReader
                reader = PdfReader(path)
                fields = reader.get_fields()
                if fields:
                    for field_name, field in fields.items():
                        forms_info.append({
                            "name": field_name,
                            "value": self._safe_get_field_value(field, "value"),
                            "field_type": self._safe_get_field_value(field, "field_type"),
                        })
            except Exception as e:
                self.logger.warning("read: 表单字段提取失败: %s", e)

        # ------------------------------------------------------------------ #
        #  注释提取（pypdf）
        # ------------------------------------------------------------------ #
        annotations_by_page = []
        if include_annotations:
            try:
                from pypdf import PdfReader
                reader = PdfReader(path)
                for page_num in page_numbers:
                    idx = page_num - 1
                    if idx >= len(reader.pages):
                        continue
                    page = reader.pages[idx]
                    annots = []
                    if "/Annots" in page:
                        try:
                            for annot_ref in page["/Annots"]:
                                annot = annot_ref.get_object()
                                annots.append({
                                    "subtype": str(annot.get("/Subtype", "")),
                                    "contents": str(annot.get("/Contents", "")),
                                })
                        except Exception as e:
                            self.logger.warning("read: 第 %d 页注释解析失败: %s", page_num, e)
                    annotations_by_page.append({
                        "page": page_num,
                        "annotations": annots,
                    })
            except Exception as e:
                self.logger.warning("read: 注释提取失败: %s", e)

        # ------------------------------------------------------------------ #
        #  构建返回结果（仅在对应参数为 true 时包含扩展字段，保持向后兼容）
        # ------------------------------------------------------------------ #
        self.logger.info("read: PDF 文档读取完成, path=%s, 总页数=%d", path, len(text_content))
        result = {
            "pages": text_content,
            "total_pages": len(text_content),
        }
        if extract_tables:
            result["tables"] = tables_by_page
        if include_layout:
            result["layout"] = layout_by_page
        if include_images:
            result["images"] = images_by_page
        if include_forms:
            result["forms"] = forms_info
        if include_annotations:
            result["annotations"] = annotations_by_page
        return result

    @staticmethod
    def _parse_page_ranges(pages_str, total_pages: int) -> list:
        """解析页码范围字符串为页码列表（1-based）

        支持格式：
            "1-5,8,10-12" → [1, 2, 3, 4, 5, 8, 10, 11, 12]
            "3" → [3]
            None 或空字符串 → 所有页
        """
        if not pages_str:
            return list(range(1, total_pages + 1))

        result = []
        for part in str(pages_str).split(","):
            part = part.strip()
            if not part:
                continue
            if "-" in part:
                # 范围：如 "1-5"
                try:
                    start, end = part.split("-", 1)
                    start = int(start.strip())
                    end = int(end.strip())
                    for p in range(start, end + 1):
                        if 1 <= p <= total_pages:
                            result.append(p)
                except ValueError:
                    continue
            else:
                # 单个页码
                try:
                    p = int(part)
                    if 1 <= p <= total_pages:
                        result.append(p)
                except ValueError:
                    continue
        # 去重并排序
        return sorted(set(result))

    @staticmethod
    def _extract_layout_from_fitz(text_dict: dict) -> list:
        """从 PyMuPDF get_text("dict") 结果提取文本布局信息

        返回 blocks 列表，每个 block 含 lines，每行含 spans（带字号/字体/颜色）
        """
        blocks = []
        for block in text_dict.get("blocks", []):
            # 仅处理文本块（type=0），跳过图片块（type=1）
            if block.get("type", 0) != 0:
                continue
            block_info = {
                "bbox": block.get("bbox", []),
                "lines": [],
            }
            for line in block.get("lines", []):
                line_info = {
                    "bbox": line.get("bbox", []),
                    "spans": [],
                }
                for span in line.get("spans", []):
                    span_info = {
                        "text": span.get("text", ""),
                        "font": span.get("font", ""),
                        "size": span.get("size", 0),
                        "color": span.get("color", 0),
                        "bbox": span.get("bbox", []),
                    }
                    line_info["spans"].append(span_info)
                block_info["lines"].append(line_info)
            blocks.append(block_info)
        return blocks

    @staticmethod
    def _safe_get_field_value(field, attr: str):
        """安全获取 pypdf Field 对象的属性值，失败返回 None"""
        try:
            value = getattr(field, attr, None)
            # 转换为可序列化的类型
            if isinstance(value, (str, int, float, bool)):
                return value
            return str(value) if value is not None else None
        except Exception:
            return None

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
