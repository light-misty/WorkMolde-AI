"""PDF 文档处理器
基于 PyMuPDF(fitz) + pypdf + pdfplumber 实现 PDF 文档的读取、转换、分析、修改
完整版：支持 read/convert/analyze/modify 操作

modify 操作通过 operation 参数分发到具体子操作，覆盖：
- 页面操作：rotate_pages / delete_pages / extract_pages / reorder_pages
- 合并拆分：merge / split
- 水印：add_text_watermark / add_image_watermark
- 页眉页脚：add_header_footer
- 加密解密：encrypt / decrypt
- 元数据：set_metadata
- 书签目录：add_bookmarks / set_toc
- 注释：add_annotation
- 表单：fill_form
- 压缩：compress
"""

import os
import html
import logging


class PdfHandler:
    """PDF 文档处理器（完整版，支持 read/convert/analyze/modify）"""

    logger = logging.getLogger(__name__)

    def read(self, params: dict) -> dict:
        """读取 PDF 文档内容（完整版，支持提取 PDF 中的所有视觉元素）

        params:
            path: 文件路径
            pages: 页码范围字符串，如 "1-5,8,10-12"（可选，默认读取所有）
            include_layout: 是否提取文本位置和样式（字号/字体/颜色），使用 PyMuPDF
            include_forms: 是否提取表单字段（AcroForm），使用 pypdf
            include_annotations: 是否提取注释（高亮/批注/签名等），使用 pypdf
            extract_tables: 是否提取表格结构，使用 pdfplumber
            include_images: 是否提取图片信息（数量/位置/尺寸），使用 PyMuPDF
            include_links: 是否提取超链接（URI/内部跳转），使用 PyMuPDF page.get_links()
            include_toc: 是否提取书签/大纲（目录），使用 PyMuPDF doc.get_toc()
            include_fonts: 是否提取字体清单，使用 PyMuPDF page.get_fonts()
            include_drawings: 是否提取绘图元素（横线/边框/矩形/曲线等矢量图形），使用 PyMuPDF page.get_drawings()
            include_image_data: 是否提取图片二进制数据（base64 编码），使用 PyMuPDF doc.extract_image()
            include_metadata_full: 是否提取完整元数据（含日期/keywords/PDF版本/加密状态等）
            include_page_geometry: 是否提取页面尺寸/方向/旋转角度
            include_signatures: 是否提取数字签名信息
            include_visual: 便捷开关，启用时同时提取 layout + drawings + page_geometry（视觉级布局）
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
        # 新增开关
        include_links = params.get("include_links", False)
        include_toc = params.get("include_toc", False)
        include_fonts = params.get("include_fonts", False)
        include_drawings = params.get("include_drawings", False)
        include_image_data = params.get("include_image_data", False)
        include_metadata_full = params.get("include_metadata_full", False)
        include_page_geometry = params.get("include_page_geometry", False)
        include_signatures = params.get("include_signatures", False)
        # 便捷开关：include_visual 同时启用 layout + drawings + page_geometry
        include_visual = params.get("include_visual", False)
        if include_visual:
            include_layout = True
            include_drawings = True
            include_page_geometry = True

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
        links_by_page = []
        fonts_by_page = []
        drawings_by_page = []
        page_geometry_by_page = []
        annotations_by_page = []
        image_data_list = []

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
        #  PyMuPDF 统一提取（布局/图片/链接/字体/绘图/页面几何/图片二进制）
        #  使用一次 doc.open 避免重复打开，提升性能
        # ------------------------------------------------------------------ #
        need_fitz = (include_layout or include_images or include_links
                     or include_fonts or include_drawings
                     or include_page_geometry or include_image_data)

        if need_fitz:
            try:
                import fitz  # PyMuPDF
                doc = fitz.open(path)

                # 书签/大纲提取（文档级，非页面级）
                toc_info = []
                if include_toc:
                    try:
                        toc_info = doc.get_toc(simple=False)
                    except Exception as e:
                        self.logger.warning("read: 书签/大纲提取失败: %s", e)

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

                    # 图片信息（含可选的二进制数据）
                    if include_images or include_image_data:
                        try:
                            image_list = page.get_images(full=True)
                            images_info = []
                            for img_idx, img in enumerate(image_list):
                                xref = img[0]
                                img_info = {
                                    "index": img_idx,
                                    "xref": xref,
                                    "width": img[2] if len(img) > 2 else None,
                                    "height": img[3] if len(img) > 3 else None,
                                    "colorspace": img[5] if len(img) > 5 else None,
                                    "bpc": img[4] if len(img) > 4 else None,
                                }
                                # 提取图片二进制数据（base64）
                                if include_image_data:
                                    try:
                                        img_data = doc.extract_image(xref)
                                        if img_data and img_data.get("image"):
                                            import base64
                                            img_info["image_base64"] = base64.b64encode(
                                                img_data["image"]).decode("ascii")
                                            img_info["ext"] = img_data.get("ext", "")
                                            img_info["size_bytes"] = len(img_data["image"])
                                            image_data_list.append({
                                                "page": page_num,
                                                "xref": xref,
                                                "ext": img_info["ext"],
                                                "size_bytes": img_info["size_bytes"],
                                            })
                                    except Exception as e:
                                        self.logger.warning("read: 第 %d 页图片 %d 二进制提取失败: %s",
                                                            page_num, img_idx, e)
                                        img_info["image_error"] = str(e)
                                images_info.append(img_info)
                            images_by_page.append({
                                "page": page_num,
                                "image_count": len(images_info),
                                "images": images_info,
                            })
                        except Exception as e:
                            self.logger.warning("read: 第 %d 页图片提取失败: %s", page_num, e)
                            images_by_page.append({"page": page_num, "image_count": 0,
                                                   "images": [], "error": str(e)})

                    # 超链接提取
                    if include_links:
                        try:
                            links = page.get_links()
                            links_info = []
                            for link in links:
                                link_info = {
                                    "kind": link.get("kind", 0),
                                    "from": list(link.get("from", [])),
                                }
                                # kind=0 文本链接(GOTO)，kind=1 URI 链接，kind=2 LAUNCH
                                if "uri" in link:
                                    link_info["uri"] = link["uri"]
                                if "page" in link:
                                    link_info["target_page"] = link["page"] + 1  # 转 1-based
                                if "to" in link:
                                    link_info["to_point"] = list(link["to"])
                                if "nameddest" in link:
                                    link_info["named_dest"] = link["nameddest"]
                                if "file" in link:
                                    link_info["file"] = link["file"]
                                links_info.append(link_info)
                            links_by_page.append({
                                "page": page_num,
                                "links": links_info,
                            })
                        except Exception as e:
                            self.logger.warning("read: 第 %d 页链接提取失败: %s", page_num, e)
                            links_by_page.append({"page": page_num, "links": [], "error": str(e)})

                    # 字体清单提取
                    if include_fonts:
                        try:
                            fonts = page.get_fonts(full=True)
                            fonts_info = []
                            for f in fonts:
                                # f 是元组 (xref, ext, type, basefont, name, encoding)
                                fonts_info.append({
                                    "xref": f[0] if len(f) > 0 else None,
                                    "ext": f[1] if len(f) > 1 else None,
                                    "type": f[2] if len(f) > 2 else None,
                                    "basefont": f[3] if len(f) > 3 else None,
                                    "name": f[4] if len(f) > 4 else None,
                                    "encoding": f[5] if len(f) > 5 else None,
                                })
                            fonts_by_page.append({
                                "page": page_num,
                                "fonts": fonts_info,
                            })
                        except Exception as e:
                            self.logger.warning("read: 第 %d 页字体提取失败: %s", page_num, e)
                            fonts_by_page.append({"page": page_num, "fonts": [], "error": str(e)})

                    # 绘图元素提取（横线、边框、矩形、曲线等矢量图形）
                    # 这是"视觉级布局"的核心：让智能体看到 PDF 中的所有视觉元素
                    if include_drawings:
                        try:
                            drawings = page.get_drawings()
                            drawings_info = []
                            for draw in drawings:
                                draw_info = {
                                    "rect": list(draw.get("rect", [])),
                                    "fill": draw.get("fill"),
                                    "color": draw.get("color"),
                                    "width": draw.get("width", 0),
                                    "stroke_opacity": draw.get("stroke_opacity", 1),
                                    "fill_opacity": draw.get("fill_opacity", 1),
                                    "items": [],
                                }
                                # 提取路径项（line/rect/curve/quadding 等）
                                for item in draw.get("items", []):
                                    item_info = {
                                        "op": item[0] if len(item) > 0 else None,
                                    }
                                    if item[0] == "l":  # 线段
                                        item_info["p1"] = list(item[1]) if item[1] else None
                                        item_info["p2"] = list(item[2]) if item[2] else None
                                    elif item[0] == "re":  # 矩形
                                        item_info["rect"] = list(item[1]) if item[1] else None
                                    elif item[0] in ("c", "cu"):  # 曲线
                                        for i, p in enumerate(item[1:], 1):
                                            if p:
                                                item_info[f"p{i}"] = list(p)
                                    draw_info["items"].append(item_info)
                                drawings_info.append(draw_info)
                            drawings_by_page.append({
                                "page": page_num,
                                "drawings": drawings_info,
                            })
                        except Exception as e:
                            self.logger.warning("read: 第 %d 页绘图提取失败: %s", page_num, e)
                            drawings_by_page.append({"page": page_num, "drawings": [],
                                                     "error": str(e)})

                    # 页面几何信息（尺寸/方向/旋转角度）
                    if include_page_geometry:
                        try:
                            page_rect = page.rect
                            page_geometry_by_page.append({
                                "page": page_num,
                                "width": page_rect.width,
                                "height": page_rect.height,
                                "rotation": page.rotation,
                                "mediabox": list(page.mediabox),
                                "cropbox": list(page.cropbox),
                                "aspect_ratio": round(page_rect.width / page_rect.height, 3)
                                                if page_rect.height else None,
                                "orientation": "landscape" if page_rect.width > page_rect.height
                                               else "portrait",
                            })
                        except Exception as e:
                            self.logger.warning("read: 第 %d 页几何信息提取失败: %s", page_num, e)
                            page_geometry_by_page.append({"page": page_num, "error": str(e)})

                # 完整元数据提取（文档级）
                metadata_full = {}
                if include_metadata_full:
                    try:
                        meta = doc.metadata or {}
                        metadata_full = {
                            "title": meta.get("title", ""),
                            "author": meta.get("author", ""),
                            "subject": meta.get("subject", ""),
                            "keywords": meta.get("keywords", ""),
                            "creator": meta.get("creator", ""),
                            "producer": meta.get("producer", ""),
                            "creation_date": meta.get("creationDate", ""),
                            "mod_date": meta.get("modDate", ""),
                            "format": meta.get("format", ""),
                            "encryption": meta.get("encryption", None),
                            "encrypted": doc.is_encrypted,
                            "page_count": len(doc),
                        }
                    except Exception as e:
                        self.logger.warning("read: 完整元数据提取失败: %s", e)

                # 数字签名信息提取（文档级）
                signatures_info = []
                if include_signatures:
                    try:
                        signatures_info = self._extract_signatures(doc, page_numbers)
                    except Exception as e:
                        self.logger.warning("read: 数字签名提取失败: %s", e)

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
        #  注释提取（pypdf，增强版含位置/作者/时间）
        # ------------------------------------------------------------------ #
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
                                annot_info = {
                                    "subtype": str(annot.get("/Subtype", "")),
                                    "contents": str(annot.get("/Contents", "")),
                                }
                                # 增强提取：位置/作者/时间/颜色
                                if "/Rect" in annot:
                                    rect = annot["/Rect"]
                                    annot_info["rect"] = [float(rect[i]) for i in range(4)]
                                if "/T" in annot:
                                    annot_info["author"] = str(annot["/T"])
                                if "/M" in annot:
                                    annot_info["mod_date"] = str(annot["/M"])
                                if "/C" in annot:
                                    colors = annot["/C"]
                                    annot_info["color"] = [float(c) for c in colors]
                                if "/Name" in annot:
                                    annot_info["name"] = str(annot["/Name"])
                                annots.append(annot_info)
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
        if include_links:
            result["links"] = links_by_page
        if include_toc:
            result["toc"] = toc_info
        if include_fonts:
            result["fonts"] = fonts_by_page
        if include_drawings:
            result["drawings"] = drawings_by_page
        if include_page_geometry:
            result["page_geometry"] = page_geometry_by_page
        if include_metadata_full:
            result["metadata_full"] = metadata_full
        if include_signatures:
            result["signatures"] = signatures_info
        if include_image_data:
            result["image_data_summary"] = image_data_list
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

    @staticmethod
    def _extract_signatures(doc, page_numbers):
        """提取 PDF 数字签名信息

        使用 PyMuPDF 遍历每页的 widget，识别签名字段
        （fitz.PDF_WIDGET_TYPE_SIGNATURE 常量值为 7）

        Args:
            doc: fitz.Document 实例
            page_numbers: 页码列表（1-based）

        Returns:
            list: 签名信息列表，每个元素含 page/field_name/field_value/rect 等
        """
        import fitz

        signatures = []
        # fitz.PDF_WIDGET_TYPE_SIGNATURE 常量值为 7（直接用数值，避免版本差异）
        PDF_WIDGET_TYPE_SIGNATURE = getattr(fitz, "PDF_WIDGET_TYPE_SIGNATURE", 7)

        for page_num in page_numbers:
            idx = page_num - 1
            if idx >= len(doc):
                continue
            page = doc[idx]

            # 通过 widget 遍历表单字段，筛选签名类型
            try:
                for widget in page.widgets():
                    if widget.field_type == PDF_WIDGET_TYPE_SIGNATURE:
                        sig_info = {
                            "page": page_num,
                            "field_name": widget.field_name or "",
                            "field_value": widget.field_value or "",
                            "rect": list(widget.rect),
                            "field_type": "signature",
                            "is_signed": bool(widget.field_value),
                        }
                        signatures.append(sig_info)
            except Exception as e:
                PdfHandler.logger.warning(
                    "_extract_signatures: 第 %d 页 widget 签名提取失败: %s",
                    page_num, e)

        # 文档级签名标志（-1=无签名字段，0=有未签名字段，1+=有签名）
        try:
            sig_flags = doc.get_sigflags()
            for sig in signatures:
                sig["doc_signature_flags"] = sig_flags
        except Exception:
            pass

        return signatures

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
    #  修改操作（modify）
    # ------------------------------------------------------------------ #

    def modify(self, params: dict) -> dict:
        """修改 PDF 文档（通过 operation 参数分发到具体子操作）

        params:
            path: 源 PDF 文件路径
            output_path: 输出文件路径（可选，默认覆盖源文件）
            operation: 操作类型，枚举值见 _MODIFY_OPERATIONS
            ... 其他操作特定参数（见各 _op_xxx 方法）
        """
        path = params.get("path", "")
        if not path:
            self.logger.error("modify: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        operation = params.get("operation", "")
        if not operation:
            return {"error": "缺少 operation 参数"}

        # 默认输出路径为源文件（覆盖）
        output_path = params.get("output_path", "") or path

        # 操作分发表
        operations = {
            # 页面操作
            "rotate_pages": self._op_rotate_pages,
            "delete_pages": self._op_delete_pages,
            "extract_pages": self._op_extract_pages,
            "reorder_pages": self._op_reorder_pages,
            # 合并拆分
            "merge": self._op_merge,
            "split": self._op_split,
            # 水印
            "add_text_watermark": self._op_add_text_watermark,
            "add_image_watermark": self._op_add_image_watermark,
            # 页眉页脚
            "add_header_footer": self._op_add_header_footer,
            # 加密解密
            "encrypt": self._op_encrypt,
            "decrypt": self._op_decrypt,
            # 元数据
            "set_metadata": self._op_set_metadata,
            # 书签目录
            "add_bookmarks": self._op_add_bookmarks,
            "set_toc": self._op_set_toc,
            # 注释
            "add_annotation": self._op_add_annotation,
            # 表单
            "fill_form": self._op_fill_form,
            # 压缩
            "compress": self._op_compress,
        }

        handler = operations.get(operation)
        if not handler:
            return {"error": f"不支持的修改操作: {operation}"}

        self.logger.info("modify: 开始执行 %s, path=%s", operation, path)
        try:
            result = handler(path, output_path, params)
            self.logger.info("modify: %s 完成, output=%s", operation, output_path)
            return result
        except Exception as e:
            self.logger.error("modify: %s 失败: %s", operation, e)
            return {"error": f"{type(e).__name__}: {e}"}

    # ------------------------------------------------------------------ #
    #  通用辅助方法
    # ------------------------------------------------------------------ #

    @staticmethod
    def _parse_pages_list(pages_param, total_pages: int) -> list:
        """将页码参数解析为 0-based 索引列表

        支持以下输入格式：
        - None 或 "all"：所有页
        - list[int]：页码列表（1-based）
        - str：页码范围字符串，如 "1-3,5,7-9"
        """
        if pages_param is None or pages_param == "all" or pages_param == "":
            return list(range(total_pages))

        if isinstance(pages_param, list):
            # 已是页码列表（1-based），转为 0-based
            return [p - 1 for p in pages_param if 1 <= p <= total_pages]

        # 字符串格式：使用现有的 _parse_page_ranges
        page_numbers = PdfHandler._parse_page_ranges(str(pages_param), total_pages)
        return [p - 1 for p in page_numbers]

    @staticmethod
    def _save_fitz_doc(doc, output_path: str, encryption=None, owner_pw=None,
                       user_pw=None, permissions=None):
        """保存 PyMuPDF 文档，自动创建父目录

        Args:
            doc: fitz.Document 对象
            output_path: 输出文件路径
            encryption: 加密方式（可选）
            owner_pw: 所有者密码（可选）
            user_pw: 用户密码（可选）
            permissions: 权限标志位（可选，仅 encryption 时生效）
        """
        os.makedirs(os.path.dirname(os.path.abspath(output_path)) or ".", exist_ok=True)
        save_kwargs = {}
        if encryption is not None:
            save_kwargs["encryption"] = encryption
            if owner_pw:
                save_kwargs["owner_pw"] = owner_pw
            if user_pw:
                save_kwargs["user_pw"] = user_pw
            if permissions is not None:
                save_kwargs["permissions"] = permissions
        doc.save(output_path, **save_kwargs)

    # ------------------------------------------------------------------ #
    #  页面操作
    # ------------------------------------------------------------------ #

    def _op_rotate_pages(self, path: str, output_path: str, params: dict) -> dict:
        """旋转页面

        params:
            pages: 页码列表/范围（1-based），如 "1-3,5" 或 [1,2,3] 或 "all"
            rotation: 旋转角度（90/180/270）
        """
        import fitz

        rotation = int(params.get("rotation", 90))
        if rotation not in (90, 180, 270):
            return {"error": f"rotation 必须是 90/180/270, 当前: {rotation}"}

        doc = fitz.open(path)
        total_pages = len(doc)
        page_indices = self._parse_pages_list(params.get("pages"), total_pages)

        for idx in page_indices:
            page = doc[idx]
            # set_rotation 会累加旋转角度（设置绝对值）
            page.set_rotation(rotation)

        self._save_fitz_doc(doc, output_path)
        doc.close()

        return {
            "operation": "rotate_pages",
            "path": output_path,
            "rotated_pages": [i + 1 for i in page_indices],
            "rotation": rotation,
            "message": f"已旋转 {len(page_indices)} 个页面 {rotation} 度",
        }

    def _op_delete_pages(self, path: str, output_path: str, params: dict) -> dict:
        """删除页面

        params:
            pages: 页码列表/范围（1-based），如 "1-3,5" 或 [1,2,3]
        """
        import fitz

        doc = fitz.open(path)
        total_pages = len(doc)
        page_indices = self._parse_pages_list(params.get("pages"), total_pages)

        if not page_indices:
            doc.close()
            return {"error": "没有指定要删除的页面"}

        if len(page_indices) >= total_pages:
            doc.close()
            return {"error": "不能删除所有页面"}

        # fitz 的 delete_pages 接受 0-based 索引列表
        doc.delete_pages(page_indices)
        self._save_fitz_doc(doc, output_path)
        remaining_pages = len(doc)
        doc.close()

        return {
            "operation": "delete_pages",
            "path": output_path,
            "deleted_pages": [i + 1 for i in page_indices],
            "remaining_pages": remaining_pages,
            "message": f"已删除 {len(page_indices)} 个页面，剩余 {remaining_pages} 页",
        }

    def _op_extract_pages(self, path: str, output_path: str, params: dict) -> dict:
        """提取页面为新 PDF

        params:
            pages: 页码列表/范围（1-based），如 "1-3,5" 或 [1,2,3]
        """
        import fitz

        doc = fitz.open(path)
        total_pages = len(doc)
        page_indices = self._parse_pages_list(params.get("pages"), total_pages)

        if not page_indices:
            doc.close()
            return {"error": "没有指定要提取的页面"}

        # 逐页插入到新文档（支持非连续页面范围）
        new_doc = fitz.open()
        for idx in page_indices:
            new_doc.insert_pdf(doc, from_page=idx, to_page=idx)

        self._save_fitz_doc(new_doc, output_path)
        extracted_count = len(new_doc)
        new_doc.close()
        doc.close()

        return {
            "operation": "extract_pages",
            "path": output_path,
            "extracted_pages": [i + 1 for i in page_indices],
            "extracted_count": extracted_count,
            "message": f"已提取 {extracted_count} 个页面到新 PDF",
        }

    def _op_reorder_pages(self, path: str, output_path: str, params: dict) -> dict:
        """重排页面顺序

        params:
            new_order: 新的页面顺序列表（1-based），如 [3, 1, 2, 4]
        """
        import fitz

        doc = fitz.open(path)
        total_pages = len(doc)

        new_order = params.get("new_order", [])
        if not new_order:
            doc.close()
            return {"error": "缺少 new_order 参数"}

        # 转为 0-based 索引并校验
        new_indices = []
        for p in new_order:
            p = int(p)
            if not 1 <= p <= total_pages:
                doc.close()
                return {"error": f"页码 {p} 超出范围（1-{total_pages}）"}
            new_indices.append(p - 1)

        # 校验是否包含所有页面（每个页面恰好出现一次）
        if sorted(new_indices) != list(range(total_pages)):
            doc.close()
            return {"error": "new_order 必须包含所有页面且每个页面只出现一次"}

        # 使用 new PDF + 按新顺序插入页面
        new_doc = fitz.open()
        for idx in new_indices:
            new_doc.insert_pdf(doc, from_page=idx, to_page=idx)

        self._save_fitz_doc(new_doc, output_path)
        new_doc.close()
        doc.close()

        return {
            "operation": "reorder_pages",
            "path": output_path,
            "new_order": new_order,
            "message": f"已重排页面顺序为 {new_order}",
        }

    # ------------------------------------------------------------------ #
    #  合并与拆分
    # ------------------------------------------------------------------ #

    def _op_merge(self, path: str, output_path: str, params: dict) -> dict:
        """合并多个 PDF

        params:
            input_paths: 要合并到源 PDF 之后的 PDF 文件路径列表
        """
        import fitz

        input_paths = params.get("input_paths", [])
        if not input_paths:
            return {"error": "缺少 input_paths 参数"}

        # 校验所有输入文件存在
        for p in input_paths:
            if not os.path.exists(p):
                return {"error": f"输入文件不存在: {p}"}

        doc = fitz.open(path)
        for p in input_paths:
            other = fitz.open(p)
            doc.insert_pdf(other)
            other.close()

        self._save_fitz_doc(doc, output_path)
        total_pages = len(doc)
        doc.close()

        return {
            "operation": "merge",
            "path": output_path,
            "merged_files": input_paths,
            "total_pages": total_pages,
            "message": f"已合并 {len(input_paths)} 个 PDF，共 {total_pages} 页",
        }

    def _op_split(self, path: str, output_path: str, params: dict) -> dict:
        """拆分 PDF

        params:
            mode: 拆分模式
                - "ranges": 按指定范围拆分，需要 ranges 参数
                - "every_page": 每页拆分为单独 PDF
                - "every_n_pages": 每 N 页拆分，需要 n 参数
            ranges: 拆分范围列表（mode="ranges" 时必填），如 [{"start": 1, "end": 3}, {"start": 4, "end": 6}]
            n: 每 N 页拆分（mode="every_n_pages" 时必填）
            output_dir: 输出目录（可选，默认与源文件同目录）
        """
        import fitz

        doc = fitz.open(path)
        total_pages = len(doc)
        mode = params.get("mode", "ranges")
        output_dir = params.get("output_dir", "") or os.path.dirname(os.path.abspath(path))
        os.makedirs(output_dir, exist_ok=True)

        base_name = os.path.splitext(os.path.basename(path))[0]
        split_files = []

        if mode == "every_page":
            # 每页拆分为单独 PDF
            for i in range(total_pages):
                new_doc = fitz.open()
                new_doc.insert_pdf(doc, from_page=i, to_page=i)
                out_path = os.path.join(output_dir, f"{base_name}_part_{i + 1}.pdf")
                self._save_fitz_doc(new_doc, out_path)
                new_doc.close()
                split_files.append(out_path)

        elif mode == "every_n_pages":
            n = int(params.get("n", 1))
            if n < 1:
                doc.close()
                return {"error": "n 必须大于 0"}
            for start in range(0, total_pages, n):
                end = min(start + n - 1, total_pages - 1)
                new_doc = fitz.open()
                new_doc.insert_pdf(doc, from_page=start, to_page=end)
                part_num = start // n + 1
                out_path = os.path.join(output_dir, f"{base_name}_part_{part_num}.pdf")
                self._save_fitz_doc(new_doc, out_path)
                new_doc.close()
                split_files.append(out_path)

        elif mode == "ranges":
            ranges = params.get("ranges", [])
            if not ranges:
                doc.close()
                return {"error": "mode='ranges' 时需要 ranges 参数"}
            for idx, r in enumerate(ranges):
                start = int(r.get("start", 1)) - 1
                end = int(r.get("end", total_pages)) - 1
                if start < 0 or end >= total_pages or start > end:
                    doc.close()
                    return {"error": f"范围 {r} 无效"}
                new_doc = fitz.open()
                new_doc.insert_pdf(doc, from_page=start, to_page=end)
                out_path = os.path.join(output_dir, f"{base_name}_part_{idx + 1}.pdf")
                self._save_fitz_doc(new_doc, out_path)
                new_doc.close()
                split_files.append(out_path)

        else:
            doc.close()
            return {"error": f"不支持的拆分模式: {mode}"}

        doc.close()

        return {
            "operation": "split",
            "split_files": split_files,
            "split_count": len(split_files),
            "message": f"已拆分为 {len(split_files)} 个 PDF 文件",
        }

    # ------------------------------------------------------------------ #
    #  水印
    # ------------------------------------------------------------------ #

    def _op_add_text_watermark(self, path: str, output_path: str, params: dict) -> dict:
        """添加文字水印

        params:
            text: 水印文字
            pages: 页码列表/范围（1-based），如 "1-3,5" 或 "all"
            font_size: 字号（默认 50）
            color: 颜色（RGB 元组或十六进制字符串，默认 红色 (1, 0, 0)）
            opacity: 不透明度（0-1，默认 0.3）
            rotation: 旋转角度（默认 45，支持任意角度）
            position: 位置（"center"/"top-left"/"bottom-right" 或 [x, y] 坐标）
        """
        import fitz

        text = params.get("text", "")
        if not text:
            return {"error": "缺少 text 参数"}

        font_size = float(params.get("font_size", 50))
        opacity = float(params.get("opacity", 0.3))
        rotation = float(params.get("rotation", 45))

        # 解析颜色
        color = params.get("color", (1, 0, 0))
        if isinstance(color, str):
            # 十六进制颜色 "#FF0000" -> (1.0, 0.0, 0.0)
            color = color.lstrip("#")
            r = int(color[0:2], 16) / 255
            g = int(color[2:4], 16) / 255
            b = int(color[4:6], 16) / 255
            color = (r, g, b)

        # 检测是否包含中文，自动选择 CJK 字体
        # PyMuPDF 内置 CJK 字体：china-s（简体）/china-t（繁体）/japan/korea
        has_chinese = any('\u4e00' <= ch <= '\u9fff' for ch in text)
        fontname = "china-s" if has_chinese else "helv"

        doc = fitz.open(path)
        total_pages = len(doc)
        page_indices = self._parse_pages_list(params.get("pages"), total_pages)

        for idx in page_indices:
            page = doc[idx]
            page_rect = page.rect

            # 计算水印位置
            position = params.get("position", "center")
            if isinstance(position, list) and len(position) == 2:
                center_x, center_y = position[0], position[1]
            elif position == "top-left":
                center_x, center_y = page_rect.width * 0.25, page_rect.height * 0.25
            elif position == "top-right":
                center_x, center_y = page_rect.width * 0.75, page_rect.height * 0.25
            elif position == "bottom-left":
                center_x, center_y = page_rect.width * 0.25, page_rect.height * 0.75
            elif position == "bottom-right":
                center_x, center_y = page_rect.width * 0.75, page_rect.height * 0.75
            else:  # center
                center_x, center_y = page_rect.width / 2, page_rect.height / 2

            # 使用 insert_text + morph 实现任意角度旋转 + 透明度
            # morph 参数格式：(旋转中心点, 变换矩阵)
            # insert_point 是文字基线起点（左下角）
            text_width = len(text) * font_size * 0.6
            insert_point = fitz.Point(center_x - text_width / 2, center_y + font_size * 0.35)
            morph = (fitz.Point(center_x, center_y), fitz.Matrix(rotation))

            page.insert_text(
                insert_point, text,
                fontsize=font_size,
                fontname=fontname,
                color=color,
                morph=morph,
                fill_opacity=opacity,
                overlay=True,
            )

        self._save_fitz_doc(doc, output_path)
        doc.close()

        return {
            "operation": "add_text_watermark",
            "path": output_path,
            "text": text,
            "watermarked_pages": [i + 1 for i in page_indices],
            "message": f"已在 {len(page_indices)} 个页面添加文字水印",
        }

    def _op_add_image_watermark(self, path: str, output_path: str, params: dict) -> dict:
        """添加图片水印

        params:
            image_path: 水印图片路径
            pages: 页码列表/范围（1-based）
            opacity: 不透明度（0-1，默认 0.3）
            scale: 缩放比例（默认 0.5）
            position: 位置（"center"/"top-left"/"bottom-right" 或 [x, y] 坐标）
        """
        import fitz

        image_path = params.get("image_path", "")
        if not image_path:
            return {"error": "缺少 image_path 参数"}
        if not os.path.exists(image_path):
            return {"error": f"水印图片不存在: {image_path}"}

        scale = float(params.get("scale", 0.5))

        doc = fitz.open(path)
        total_pages = len(doc)
        page_indices = self._parse_pages_list(params.get("pages"), total_pages)

        # 预先获取图片原始尺寸（避免在循环内重复打开）
        img_doc = fitz.open(image_path)
        img_rect = img_doc[0].rect if len(img_doc) > 0 else fitz.Rect(0, 0, 200, 200)
        img_doc.close()
        img_width = img_rect.width * scale
        img_height = img_rect.height * scale

        for idx in page_indices:
            page = doc[idx]
            page_rect = page.rect

            # 计算图片位置
            position = params.get("position", "center")
            if isinstance(position, list) and len(position) == 2:
                x, y = position[0], position[1]
            elif position == "top-left":
                x, y = 0, 0
            elif position == "top-right":
                x, y = page_rect.width - img_width, 0
            elif position == "bottom-left":
                x, y = 0, page_rect.height - img_height
            elif position == "bottom-right":
                x, y = page_rect.width - img_width, page_rect.height - img_height
            else:  # center
                x = (page_rect.width - img_width) / 2
                y = (page_rect.height - img_height) / 2

            rect = fitz.Rect(x, y, x + img_width, y + img_height)
            page.insert_image(rect, filename=image_path, overlay=True)

        self._save_fitz_doc(doc, output_path)
        doc.close()

        return {
            "operation": "add_image_watermark",
            "path": output_path,
            "image_path": image_path,
            "watermarked_pages": [i + 1 for i in page_indices],
            "message": f"已在 {len(page_indices)} 个页面添加图片水印",
        }

    # ------------------------------------------------------------------ #
    #  页眉页脚
    # ------------------------------------------------------------------ #

    def _op_add_header_footer(self, path: str, output_path: str, params: dict) -> dict:
        """添加页眉页脚

        params:
            header_text: 页眉文字（可选）
            footer_text: 页脚文字（可选）
            pages: 页码列表/范围（1-based）
            font_size: 字号（默认 10）
            margin: 边距（默认 30 points）
            show_page_number: 是否在页脚显示页码（默认 true）
            header_align: 页眉对齐（"left"/"center"/"right"，默认 "center"）
            footer_align: 页脚对齐（默认 "center"）
        """
        import fitz

        header_text = params.get("header_text", "")
        footer_text = params.get("footer_text", "")
        if not header_text and not footer_text:
            return {"error": "至少需要提供 header_text 或 footer_text 之一"}

        font_size = float(params.get("font_size", 10))
        margin = float(params.get("margin", 30))
        show_page_number = params.get("show_page_number", True)
        header_align = params.get("header_align", "center")
        footer_align = params.get("footer_align", "center")

        doc = fitz.open(path)
        total_pages = len(doc)
        page_indices = self._parse_pages_list(params.get("pages"), total_pages)

        align_map = {
            "left": fitz.TEXT_ALIGN_LEFT,
            "center": fitz.TEXT_ALIGN_CENTER,
            "right": fitz.TEXT_ALIGN_RIGHT,
        }
        h_align = align_map.get(header_align, fitz.TEXT_ALIGN_CENTER)
        f_align = align_map.get(footer_align, fitz.TEXT_ALIGN_CENTER)

        for idx in page_indices:
            page = doc[idx]
            page_rect = page.rect

            # 插入页眉
            if header_text:
                header_rect = fitz.Rect(
                    margin,
                    margin / 2,
                    page_rect.width - margin,
                    margin / 2 + font_size * 1.5,
                )
                page.insert_textbox(
                    header_rect, header_text,
                    fontsize=font_size, align=h_align, color=(0.2, 0.2, 0.2),
                )

            # 插入页脚
            footer_content = footer_text
            if show_page_number:
                page_str = f"第 {idx + 1} 页 / 共 {total_pages} 页"
                footer_content = f"{footer_content}  {page_str}" if footer_content else page_str

            if footer_content:
                footer_rect = fitz.Rect(
                    margin,
                    page_rect.height - margin - font_size * 1.5,
                    page_rect.width - margin,
                    page_rect.height - margin,
                )
                page.insert_textbox(
                    footer_rect, footer_content,
                    fontsize=font_size, align=f_align, color=(0.2, 0.2, 0.2),
                )

        self._save_fitz_doc(doc, output_path)
        doc.close()

        return {
            "operation": "add_header_footer",
            "path": output_path,
            "header_text": header_text,
            "footer_text": footer_text,
            "processed_pages": [i + 1 for i in page_indices],
            "message": f"已在 {len(page_indices)} 个页面添加页眉页脚",
        }

    # ------------------------------------------------------------------ #
    #  加密与解密
    # ------------------------------------------------------------------ #

    def _op_encrypt(self, path: str, output_path: str, params: dict) -> dict:
        """加密 PDF

        params:
            user_password: 用户密码（打开 PDF 需要）
            owner_password: 所有者密码（修改权限需要，可选，默认同 user_password）
            permissions: 权限字典（可选），如 {"print": false, "copy": false, "modify": false}
        """
        import fitz

        user_pw = params.get("user_password", "")
        owner_pw = params.get("owner_password", "") or user_pw
        if not user_pw and not owner_pw:
            return {"error": "至少需要提供 user_password 或 owner_password"}

        # 解析权限
        perms = params.get("permissions", {})
        # fitz 权限常量：PDF_PERM_PRINT / PDF_PERM_COPY / PDF_PERM_MODIFY 等
        perm_flags = 0xFFFFFFFF  # 默认全部允许
        if perms:
            perm_flags = 0
            if perms.get("print", True):
                perm_flags |= fitz.PDF_PERM_PRINT
            if perms.get("copy", True):
                perm_flags |= fitz.PDF_PERM_COPY
            if perms.get("modify", True):
                perm_flags |= fitz.PDF_PERM_MODIFY
            if perms.get("annotate", True):
                perm_flags |= fitz.PDF_PERM_ANNOTATE
            if perms.get("fill_forms", True):
                perm_flags |= fitz.PDF_PERM_FORM
            if perms.get("extract", True):
                perm_flags |= fitz.PDF_PERM_ACCESSIBILITY  # 提取文本/图形（对应可访问性权限）
            if perms.get("assemble", True):
                perm_flags |= fitz.PDF_PERM_ASSEMBLE
            if perms.get("print_hq", True):
                perm_flags |= fitz.PDF_PERM_PRINT_HQ

        doc = fitz.open(path)
        # 使用 AES-256 加密，传递权限标志位
        self._save_fitz_doc(
            doc, output_path,
            encryption=fitz.PDF_ENCRYPT_AES_256,
            owner_pw=owner_pw,
            user_pw=user_pw,
            permissions=perm_flags if perms else None,
        )
        doc.close()

        return {
            "operation": "encrypt",
            "path": output_path,
            "encryption": "AES-256",
            "message": "PDF 已加密（AES-256）",
        }

    def _op_decrypt(self, path: str, output_path: str, params: dict) -> dict:
        """解密 PDF

        params:
            password: PDF 密码（用户密码或所有者密码）
        """
        import fitz

        password = params.get("password", "")
        if not password:
            return {"error": "缺少 password 参数"}

        doc = fitz.open(path)
        if not doc.is_encrypted:
            doc.close()
            return {"error": "PDF 未加密"}

        if not doc.authenticate(password):
            doc.close()
            return {"error": "密码错误"}

        # 保存时不带加密参数，并清除密码
        # 注意：需要先认证才能保存解密版本
        self._save_fitz_doc(doc, output_path)
        doc.close()

        return {
            "operation": "decrypt",
            "path": output_path,
            "message": "PDF 已解密",
        }

    # ------------------------------------------------------------------ #
    #  元数据
    # ------------------------------------------------------------------ #

    def _op_set_metadata(self, path: str, output_path: str, params: dict) -> dict:
        """设置 PDF 元数据

        params:
            metadata: 元数据字典，可包含 title/author/subject/keywords/creator/producer
        """
        import fitz

        metadata = params.get("metadata", {})
        if not metadata:
            return {"error": "缺少 metadata 参数"}

        doc = fitz.open(path)
        # 获取现有元数据并合并
        existing = doc.metadata or {}
        new_meta = {
            "title": metadata.get("title", existing.get("title", "")),
            "author": metadata.get("author", existing.get("author", "")),
            "subject": metadata.get("subject", existing.get("subject", "")),
            "keywords": metadata.get("keywords", existing.get("keywords", "")),
            "creator": metadata.get("creator", existing.get("creator", "")),
            "producer": metadata.get("producer", "WorkMolde AI"),
        }
        doc.set_metadata(new_meta)
        self._save_fitz_doc(doc, output_path)
        doc.close()

        return {
            "operation": "set_metadata",
            "path": output_path,
            "metadata": new_meta,
            "message": "元数据已更新",
        }

    # ------------------------------------------------------------------ #
    #  书签与目录
    # ------------------------------------------------------------------ #

    def _op_add_bookmarks(self, path: str, output_path: str, params: dict) -> dict:
        """添加书签（在现有书签之后追加）

        params:
            bookmarks: 书签列表，每项为 {"title": "标题", "page": 1, "level": 1}
        """
        import fitz

        bookmarks = params.get("bookmarks", [])
        if not bookmarks:
            return {"error": "缺少 bookmarks 参数"}

        doc = fitz.open(path)
        total_pages = len(doc)

        # 获取现有 TOC
        existing_toc = doc.get_toc(simple=True)

        # 构建新的 TOC 条目（[level, title, page]）
        for bm in bookmarks:
            level = int(bm.get("level", 1))
            title = bm.get("title", "")
            page = int(bm.get("page", 1))
            if not 1 <= page <= total_pages:
                doc.close()
                return {"error": f"书签页码 {page} 超出范围（1-{total_pages}）"}
            existing_toc.append([level, title, page])

        doc.set_toc(existing_toc)
        self._save_fitz_doc(doc, output_path)
        doc.close()

        return {
            "operation": "add_bookmarks",
            "path": output_path,
            "added_bookmarks": bookmarks,
            "total_bookmarks": len(existing_toc),
            "message": f"已添加 {len(bookmarks)} 个书签",
        }

    def _op_set_toc(self, path: str, output_path: str, params: dict) -> dict:
        """设置目录大纲（覆盖现有 TOC）

        params:
            toc: 目录列表，每项为 {"level": 1, "title": "标题", "page": 1}
        """
        import fitz

        toc = params.get("toc", [])
        if not toc:
            return {"error": "缺少 toc 参数"}

        doc = fitz.open(path)
        total_pages = len(doc)

        # 构建新的 TOC 条目（[level, title, page]）
        new_toc = []
        for item in toc:
            level = int(item.get("level", 1))
            title = item.get("title", "")
            page = int(item.get("page", 1))
            if not 1 <= page <= total_pages:
                doc.close()
                return {"error": f"目录页码 {page} 超出范围（1-{total_pages}）"}
            new_toc.append([level, title, page])

        doc.set_toc(new_toc)
        self._save_fitz_doc(doc, output_path)
        doc.close()

        return {
            "operation": "set_toc",
            "path": output_path,
            "toc": new_toc,
            "message": f"已设置目录（共 {len(new_toc)} 个条目）",
        }

    # ------------------------------------------------------------------ #
    #  注释
    # ------------------------------------------------------------------ #

    def _op_add_annotation(self, path: str, output_path: str, params: dict) -> dict:
        """添加注释

        params:
            page: 页码（1-based）
            type: 注释类型，枚举：text/highlight/underline/strikethrough/squiggly/stamp
            rect: 注释区域 [x0, y0, x1, y1]（highlight/underline/strikethrough/squiggly 必填）
            point: 注释位置 [x, y]（text/stamp 类型必填）
            contents: 注释内容文字
            author: 注释作者
            color: 注释颜色（RGB 元组或十六进制字符串）
        """
        import fitz

        page_num = int(params.get("page", 1))
        annot_type = params.get("type", "text")
        contents = params.get("contents", "")
        author = params.get("author", "")

        # 解析颜色
        color = params.get("color", (1, 1, 0))
        if isinstance(color, str):
            color = color.lstrip("#")
            r = int(color[0:2], 16) / 255
            g = int(color[2:4], 16) / 255
            b = int(color[4:6], 16) / 255
            color = (r, g, b)

        doc = fitz.open(path)
        total_pages = len(doc)
        if not 1 <= page_num <= total_pages:
            doc.close()
            return {"error": f"页码 {page_num} 超出范围（1-{total_pages}）"}

        page = doc[page_num - 1]

        if annot_type == "text":
            point = params.get("point", [50, 50])
            annot = page.add_text_annot(fitz.Point(point[0], point[1]), contents)

        elif annot_type == "highlight":
            rect = params.get("rect")
            if not rect:
                doc.close()
                return {"error": "highlight 类型需要 rect 参数"}
            annot = page.add_highlight_annot(fitz.Rect(rect))

        elif annot_type == "underline":
            rect = params.get("rect")
            if not rect:
                doc.close()
                return {"error": "underline 类型需要 rect 参数"}
            annot = page.add_underline_annot(fitz.Rect(rect))

        elif annot_type == "strikethrough":
            rect = params.get("rect")
            if not rect:
                doc.close()
                return {"error": "strikethrough 类型需要 rect 参数"}
            annot = page.add_strikeout_annot(fitz.Rect(rect))

        elif annot_type == "squiggly":
            rect = params.get("rect")
            if not rect:
                doc.close()
                return {"error": "squiggly 类型需要 rect 参数"}
            annot = page.add_squiggly_annot(fitz.Rect(rect))

        elif annot_type == "stamp":
            rect = params.get("rect", [50, 50, 200, 100])
            annot = page.add_stamp_annot(fitz.Rect(rect), stamp=0)

        else:
            doc.close()
            return {"error": f"不支持的注释类型: {annot_type}"}

        # 设置注释属性
        if contents:
            annot.set_info(content=contents)
        if author:
            annot.set_info(title=author)
        annot.set_colors(stroke=color)
        annot.update()

        self._save_fitz_doc(doc, output_path)
        doc.close()

        return {
            "operation": "add_annotation",
            "path": output_path,
            "page": page_num,
            "type": annot_type,
            "message": f"已在第 {page_num} 页添加 {annot_type} 注释",
        }

    # ------------------------------------------------------------------ #
    #  表单填充
    # ------------------------------------------------------------------ #

    def _op_fill_form(self, path: str, output_path: str, params: dict) -> dict:
        """填充 PDF 表单

        params:
            fields: 字段值字典，如 {"name": "张三", "age": "25", "gender": "男"}
        """
        import fitz

        fields = params.get("fields", {})
        if not fields:
            return {"error": "缺少 fields 参数"}

        doc = fitz.open(path)
        filled_count = 0
        not_found = []

        # 单次遍历所有页面和 widget，同步填充表单和收集所有字段名
        all_field_names = set()
        for page_num in range(len(doc)):
            page = doc[page_num]
            widgets = page.widgets()
            if not widgets:
                continue
            for widget in widgets:
                field_name = widget.field_name
                all_field_names.add(field_name)
                if field_name in fields:
                    widget.field_value = str(fields[field_name])
                    widget.update()
                    filled_count += 1

        # 检查哪些字段未找到
        for field_name in fields:
            if field_name not in all_field_names:
                not_found.append(field_name)

        self._save_fitz_doc(doc, output_path)
        doc.close()

        return {
            "operation": "fill_form",
            "path": output_path,
            "filled_count": filled_count,
            "not_found_fields": not_found,
            "message": f"已填充 {filled_count} 个表单字段" + (
                f"，{len(not_found)} 个字段未找到" if not_found else ""),
        }

    # ------------------------------------------------------------------ #
    #  压缩
    # ------------------------------------------------------------------ #

    def _op_compress(self, path: str, output_path: str, params: dict) -> dict:
        """压缩 PDF

        params:
            level: 压缩级别（"default"/"max"/"none"，默认 "default"）
            garbage: 是否清除垃圾对象（默认 true）
            deflate: 是否使用 deflate 压缩流（默认 true）
            clean: 是否清理内容流（默认 true）
            subset_fonts: 是否子集化字体（默认 true）
        """
        import fitz

        doc = fitz.open(path)
        original_size = os.path.getsize(path)

        # 保存时使用压缩选项
        os.makedirs(os.path.dirname(os.path.abspath(output_path)) or ".", exist_ok=True)

        save_kwargs = {}
        if params.get("garbage", True):
            # garbage 级别：1=基本去重，2=更激进，3=最激进+子集化字体，4=3+清理内容流
            save_kwargs["garbage"] = 3
        if params.get("deflate", True):
            save_kwargs["deflate"] = True
        if params.get("clean", True):
            save_kwargs["clean"] = True

        # 子集化字体（减少字体文件大小）
        if params.get("subset_fonts", True):
            try:
                doc.subset_fonts()
            except Exception as e:
                self.logger.warning("compress: 字体子集化失败: %s", e)

        doc.save(output_path, **save_kwargs)
        doc.close()

        new_size = os.path.getsize(output_path)
        compression_ratio = (1 - new_size / original_size) * 100 if original_size > 0 else 0

        return {
            "operation": "compress",
            "path": output_path,
            "original_size": original_size,
            "compressed_size": new_size,
            "compression_ratio": round(compression_ratio, 2),
            "message": f"压缩完成：{original_size} → {new_size} 字节（节省 {compression_ratio:.1f}%）",
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
