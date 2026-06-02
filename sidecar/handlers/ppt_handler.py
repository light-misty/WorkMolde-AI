"""PPT Handler - Professional presentation generation
Based on python-pptx, uses white background with dark text for business-style slides.
"""

import os
import re
import logging
from typing import Any, Optional, List

from pptx import Presentation
from pptx.util import Inches, Pt, Emu
from pptx.enum.text import PP_ALIGN
from pptx.dml.color import RGBColor
from pptx.oxml.ns import qn as pptx_qn
from pptx.enum.shapes import MSO_SHAPE
from lxml import etree


class PptHandler:
    """PowerPoint (.pptx) document processor"""

    logger = logging.getLogger(__name__)

    COLOR_SCHEMES = {
        "ocean": {
            "primary": "065A82",
            "secondary": "4A7C8F",
            "accent": "1C7293",
            "body": "333333",
            "bg": "FFFFFF",
            "name": "Ocean",
        },
        "midnight": {
            "primary": "1E2761",
            "secondary": "3D4A7A",
            "accent": "5B6AAF",
            "body": "333333",
            "bg": "FFFFFF",
            "name": "Midnight",
        },
        "forest": {
            "primary": "2C5F2D",
            "secondary": "5A8A5B",
            "accent": "97BC62",
            "body": "333333",
            "bg": "FFFFFF",
            "name": "Forest",
        },
        "coral": {
            "primary": "C0392B",
            "secondary": "D35400",
            "accent": "F96167",
            "body": "333333",
            "bg": "FFFFFF",
            "name": "Coral",
        },
        "charcoal": {
            "primary": "2C3E50",
            "secondary": "5D6D7E",
            "accent": "85929E",
            "body": "333333",
            "bg": "FFFFFF",
            "name": "Charcoal",
        },
    }

    FONT_SIZES = {
        "cover_title": Pt(48),
        "cover_subtitle": Pt(24),
        "slide_title": Pt(36),
        "section_title": Pt(28),
        "body": Pt(18),
        "body_small": Pt(16),
        "note": Pt(12),
        "slide_number": Pt(10),
    }

    LATIN_FONT = "Calibri"
    EAST_ASIAN_FONT = "\u5fae\u8f6f\u96c5\u9ed1"

    def generate(self, params: dict) -> dict:
        path = params.get("path", "")
        title = params.get("title", "")
        subtitle = params.get("subtitle", "")
        author = params.get("author", "")
        color_scheme = params.get("colorScheme", "ocean")
        slides = params.get("slides", []) or []
        auto_title = params.get("titleSlide", True)

        if not path:
            self.logger.error("generate: missing output path")
            return {"error": "missing output path"}

        self.logger.info("generate: creating PPT at %s", path)
        os.makedirs(os.path.dirname(path) or ".", exist_ok=True)

        prs = Presentation()
        scheme = self.COLOR_SCHEMES.get(color_scheme, self.COLOR_SCHEMES["ocean"])
        sw, sh = prs.slide_width, prs.slide_height

        # 设置文档作者属性
        if author:
            prs.core_properties.author = author
        if title:
            prs.core_properties.title = title

        ml, mr, mt, mb = Inches(0.6), Inches(0.6), Inches(0.6), Inches(0.8)
        tw = sw - ml - mr

        slide_idx = 0
        total = len(slides) + (1 if auto_title and title else 0)

        if auto_title and title:
            self._make_cover(prs, title, subtitle or "", scheme, sw, sh, slide_idx, total)
            slide_idx += 1

        for sd in slides:
            st = sd.get("title", "")
            sc = sd.get("content", "")
            sl = sd.get("layout", "content")
            bl = sd.get("bullets", [])

            if sl == "section":
                self._make_section(prs, st, scheme, sw, sh, slide_idx, total)
                slide_idx += 1
                continue

            if sl == "title" and slide_idx == 0:
                self._make_cover(prs, st, sc or "", scheme, sw, sh, slide_idx, total)
                slide_idx += 1
                continue

            self._make_content(prs, st, sc, bl, scheme, sw, sh, ml, mr, mt, mb, tw, slide_idx, total)
            slide_idx += 1

        prs.save(path)
        self.logger.info("generate: PPT saved, %d slides", slide_idx)
        return {"path": path, "slide_count": slide_idx, "message": "PPT document generated: " + path}

    def _make_cover(self, prs, title, subtitle, scheme, sw, sh, idx, total):
        blank = prs.slide_layouts[6]
        slide = prs.slides.add_slide(blank)
        self._set_bg(slide, scheme["bg"])
        self._top_bar(slide, sw, scheme["primary"])

        tt = sh * 0.35
        tb = slide.shapes.add_textbox(Inches(1.0), tt, sw - Inches(2.0), Inches(1.5))
        p = tb.text_frame.paragraphs[0]
        p.alignment = PP_ALIGN.CENTER
        r = p.add_run()
        r.text = title
        r.font.size = self.FONT_SIZES["cover_title"]
        r.font.bold = True
        r.font.color.rgb = RGBColor.from_string(scheme["primary"])
        self._font(r)

        if subtitle:
            st = tt + Inches(1.3)
            sb = slide.shapes.add_textbox(Inches(1.0), st, sw - Inches(2.0), Inches(1.0))
            p = sb.text_frame.paragraphs[0]
            p.alignment = PP_ALIGN.CENTER
            r = p.add_run()
            r.text = subtitle
            r.font.size = self.FONT_SIZES["cover_subtitle"]
            r.font.color.rgb = RGBColor.from_string(scheme["secondary"])
            self._font(r)

        self._page_num(slide, idx, total, sw, sh, Inches(0.5), Inches(0.5), scheme)

    def _make_section(self, prs, st, scheme, sw, sh, idx, total):
        blank = prs.slide_layouts[6]
        slide = prs.slides.add_slide(blank)
        self._set_bg(slide, scheme["primary"])

        tt = sh * 0.4
        tb = slide.shapes.add_textbox(Inches(1.0), tt, sw - Inches(2.0), Inches(1.5))
        p = tb.text_frame.paragraphs[0]
        p.alignment = PP_ALIGN.CENTER
        r = p.add_run()
        r.text = st
        r.font.size = self.FONT_SIZES["section_title"]
        r.font.bold = True
        r.font.color.rgb = RGBColor(0xFF, 0xFF, 0xFF)
        self._font(r)

        self._page_num(slide, idx, total, sw, sh, Inches(0.5), Inches(0.5), scheme, white=True)

    def _make_content(self, prs, title, content, bullets, scheme, sw, sh, ml, mr, mt, mb, tw, idx, total):
        blank = prs.slide_layouts[6]
        slide = prs.slides.add_slide(blank)
        self._set_bg(slide, scheme["bg"])
        self._top_bar(slide, sw, scheme["primary"])

        # Title
        tb = slide.shapes.add_textbox(ml, mt, tw, Inches(0.9))
        p = tb.text_frame.paragraphs[0]
        r = p.add_run()
        r.text = title
        r.font.size = self.FONT_SIZES["slide_title"]
        r.font.bold = True
        r.font.color.rgb = RGBColor.from_string(scheme["primary"])
        self._font(r)

        # Separator line under title
        lt = mt + Inches(0.8)
        ln = slide.shapes.add_shape(MSO_SHAPE.RECTANGLE, ml, lt, Inches(1.0), Emu(25000))
        ln.fill.solid()
        ln.fill.fore_color.rgb = RGBColor.from_string(scheme["accent"])
        ln.line.fill.background()

        ct = lt + Inches(0.3)
        ch = sh - ct - mb

        if bullets:
            self._bullet_list(slide, bullets, scheme, ml, ct, tw, ch)
        elif content:
            self._add_body(slide, content, scheme, ml, ct, tw, ch)

        self._page_num(slide, idx, total, sw, sh, mr, mb, scheme)

    def _add_body(self, slide, text, scheme, left, top, width, height):
        lines = [l.strip() for l in text.strip().split('\n') if l.strip()]
        if len(lines) > 1:
            cleaned = []
            for line in lines:
                c = line
                for pfx in ['1.', '2.', '3.', '4.', '5.', '6.', '7.', '8.', '9.', '0.']:
                    if c.startswith(pfx):
                        c = c[len(pfx):].strip()
                        break
                c = c.lstrip('\u2022\u25cf- ').strip()
                if c:
                    cleaned.append(c)
            if cleaned:
                self._bullet_list(slide, cleaned, scheme, left, top, width, height)
                return
        self._para(slide, text, scheme, left, top, width, height)

    def _bullet_list(self, slide, items, scheme, left, top, width, height):
        tb = slide.shapes.add_textbox(left=left, top=top, width=width, height=height)
        tf = tb.text_frame
        tf.word_wrap = True
        for i, item in enumerate(items):
            p = tf.paragraphs[0] if i == 0 else tf.add_paragraph()
            p.level = 0
            p.space_after = Pt(12)
            self._fmt_run(p, item, scheme)

    def _para(self, slide, text, scheme, left, top, width, height):
        tb = slide.shapes.add_textbox(left=left, top=top, width=width, height=height)
        tf = tb.text_frame
        tf.word_wrap = True
        p = tf.paragraphs[0]
        p.space_after = Pt(6)
        self._fmt_run(p, text, scheme)

    def _fmt_run(self, para, text, scheme):
        pat = r'(\*\*(.+?)\*\*|\*(.+?)\*)'
        last = 0
        for m in re.finditer(pat, text):
            if m.start() > last:
                plain = text[last:m.start()]
                if plain:
                    r = para.add_run()
                    r.text = plain
                    self._sty(r, scheme["body"], False, False)
            if m.group(2):
                r = para.add_run()
                r.text = m.group(2)
                self._sty(r, scheme["primary"], True, False)
            elif m.group(3):
                r = para.add_run()
                r.text = m.group(3)
                self._sty(r, scheme["secondary"], False, True)
            last = m.end()
        if last < len(text):
            rem = text[last:]
            if rem:
                r = para.add_run()
                r.text = rem
                self._sty(r, scheme["body"], False, False)
        if not para.runs and text:
            r = para.add_run()
            r.text = text
            self._sty(r, scheme["body"], False, False)

    def _sty(self, run, color, bold, italic):
        run.font.color.rgb = RGBColor.from_string(color)
        run.font.size = self.FONT_SIZES["body"]
        run.font.bold = bold
        run.font.italic = italic
        self._font(run)

    def _top_bar(self, slide, sw, color):
        bar = slide.shapes.add_shape(MSO_SHAPE.RECTANGLE, 0, 0, sw, Emu(50000))
        bar.fill.solid()
        bar.fill.fore_color.rgb = RGBColor.from_string(color)
        bar.line.fill.background()

    def _set_bg(self, slide, color):
        bg = slide.background
        bg.fill.solid()
        bg.fill.fore_color.rgb = RGBColor.from_string(color)

    def _font(self, run):
        run.font.name = self.LATIN_FONT
        rPr = run._r.get_or_add_rPr()
        for child in list(rPr):
            if child.tag.endswith('}ea') or child.tag == 'ea':
                rPr.remove(child)
        ea = etree.SubElement(rPr, pptx_qn('a:ea'))
        ea.set('typeface', self.EAST_ASIAN_FONT)

    def _page_num(self, slide, idx, total, sw, sh, mr, mb, scheme, white=False):
        txt = f"{idx + 1} / {total}"
        nb = slide.shapes.add_textbox(sw - mr - Inches(0.8), sh - mb, Inches(0.8), Inches(0.3))
        p = nb.text_frame.paragraphs[0]
        p.alignment = PP_ALIGN.RIGHT
        r = p.add_run()
        r.text = txt
        r.font.size = self.FONT_SIZES["slide_number"]
        r.font.color.rgb = RGBColor(0xCC, 0xCC, 0xCC) if white else RGBColor(0x99, 0x99, 0x99)
        self._font(r)

    def read(self, params: dict) -> dict:
        path = params.get("path", "")
        if not path:
            self.logger.error("read: missing path")
            return {"error": "missing path"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)
        self.logger.info("read: loading PPT %s", path)
        prs = Presentation(path)
        slides = []
        for slide in prs.slides:
            info = {"shapes": []}
            for shape in slide.shapes:
                si = {"name": shape.name, "type": str(shape.shape_type)}
                if shape.has_text_frame:
                    si["text"] = shape.text
                info["shapes"].append(si)
            slides.append(info)
        self.logger.info("read: done, %d slides", len(slides))
        return {"slides": slides, "slide_count": len(slides)}

    def modify(self, params: dict) -> dict:
        path = params.get("path", "")
        operations = params.get("operations", [])
        if not path:
            self.logger.error("modify: missing path")
            return {"error": "missing path"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)
        self.logger.info("modify: loading PPT %s, %d ops", path, len(operations))
        prs = Presentation(path)
        count = 0
        for op in operations:
            ot = op.get("type", "")
            if ot == "addSlide":
                sd = op.get("slide", {})
                t = sd.get("title", "")
                lt = sd.get("layout", "content_slide")
                if len(prs.slide_layouts) > 0:
                    if lt == "title_slide":
                        ly = prs.slide_layouts[0]
                    elif lt == "section_slide":
                        ly = prs.slide_layouts[2] if len(prs.slide_layouts) > 2 else prs.slide_layouts[0]
                    else:
                        ly = prs.slide_layouts[1] if len(prs.slide_layouts) > 1 else prs.slide_layouts[0]
                    slide = prs.slides.add_slide(ly)
                    if slide.placeholders:
                        for ph in slide.placeholders:
                            if ph.placeholder_format.idx == 0:
                                ph.text = t
                                count += 1
                                break
            elif ot == "replaceText":
                si = op.get("slideIndex", -1)
                old = op.get("old", "")
                new = op.get("new", "")
                if 0 <= si < len(prs.slides):
                    for shape in prs.slides[si].shapes:
                        if shape.has_text_frame:
                            for para in shape.text_frame.paragraphs:
                                for run in para.runs:
                                    if old in run.text:
                                        run.text = run.text.replace(old, new)
                                        count += 1
            elif ot == "deleteSlide":
                si = op.get("slideIndex", -1)
                if 0 <= si < len(prs.slides):
                    rId = prs.slides._sldIdLst[si].rId
                    prs.part.drop_rel(rId)
                    del prs.slides._sldIdLst[si]
                    count += 1
        prs.save(path)
        self.logger.info("modify: done, %d changes", count)
        return {"path": path, "modified_count": count, "message": "Executed " + str(count) + " changes"}

    def convert(self, params: dict) -> dict:
        path = params.get("path", "")
        output_path = params.get("output_path", "")
        target = params.get("format", "pdf")
        if not path:
            self.logger.error("convert: missing path")
            return {"error": "missing path"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)
        self.logger.info("convert: %s -> %s", path, target)
        if target == "pdf":
            out = output_path or os.path.splitext(path)[0] + ".pdf"
            return {"path": out, "format": target, "message": "PPT to PDF requires LibreOffice"}
        return {"error": "unsupported format: " + target}

    def analyze(self, params: dict) -> dict:
        path = params.get("path", "")
        if not path:
            self.logger.error("analyze: missing path")
            return {"error": "missing path"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)
        self.logger.info("analyze: loading PPT %s", path)
        prs = Presentation(path)
        ts = 0
        tts = 0
        for slide in prs.slides:
            for shape in slide.shapes:
                ts += 1
                if shape.has_text_frame:
                    tts += 1
        self.logger.info("analyze: done, %d slides", len(prs.slides))
        return {"file_size": os.path.getsize(path), "slide_count": len(prs.slides), "total_shapes": ts, "total_text_shapes": tts}
