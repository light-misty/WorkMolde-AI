"""Excel 文档处理器
基于 openpyxl 实现 Excel 文档的生成、读取、修改
"""

import os
import csv
import io
import html
import logging
from typing import Any

from openpyxl import Workbook, load_workbook
from openpyxl.styles import Font, Alignment, Border, Side, PatternFill
from openpyxl.utils import get_column_letter


class ExcelHandler:
    """Excel (.xlsx) 文档处理器"""

    logger = logging.getLogger(__name__)

    def generate(self, params: dict) -> dict:
        """生成 Excel 文档

        params:
            path: 输出文件路径
            sheets: 工作表列表
                [{"name": "Sheet1", "data": [[...]], "headers": [...]}]
            content: 文档内容（当 sheets 为空时，从 content 构建）
            title: 文档标题（当 sheets 为空时，作为默认工作表名）
        """
        path = params.get("path", "")
        sheets = params.get("sheets", [])
        content = params.get("content", "")
        title = params.get("title", "")
        if not path:
            self.logger.error("generate: 缺少输出文件路径")
            return {"error": "缺少输出文件路径"}

        # 当 sheets 为空但 content 非空时，从 content 构建默认工作表
        if not sheets and content:
            self.logger.info("generate: sheets 为空，从 content 参数构建默认工作表")
            sheet_name = title if title else "Sheet1"
            # 将 content 按行拆分，每行按制表符或逗号拆分为列
            rows = []
            for line in content.split("\n"):
                line = line.strip()
                if line:
                    # 优先按制表符拆分，其次按逗号拆分
                    if "\t" in line:
                        rows.append(line.split("\t"))
                    else:
                        rows.append(line.split(","))
            sheets = [{"name": sheet_name, "data": rows}]

        self.logger.info("generate: 开始生成 Excel 文档, path=%s, 工作表数=%d", path, len(sheets))

        os.makedirs(os.path.dirname(path) or ".", exist_ok=True)

        wb = Workbook()
        # 删除默认工作表
        default_sheet = wb.active
        if default_sheet:
            wb.remove(default_sheet)

        for sheet_info in sheets:
            sheet_name = sheet_info.get("name", "Sheet1")
            data = sheet_info.get("data", [])
            headers = sheet_info.get("headers", [])

            ws = wb.create_sheet(title=sheet_name)

            # 写入表头
            if headers:
                for col_idx, header in enumerate(headers, 1):
                    cell = ws.cell(row=1, column=col_idx, value=header)
                    cell.font = Font(bold=True)
                    cell.alignment = Alignment(horizontal="center")

            # 写入数据
            start_row = 2 if headers else 1
            for row_idx, row_data in enumerate(data, start_row):
                for col_idx, value in enumerate(row_data, 1):
                    ws.cell(row=row_idx, column=col_idx, value=value)

            # 自动调整列宽
            for col in ws.columns:
                max_length = 0
                col_letter = get_column_letter(col[0].column)
                for cell in col:
                    if cell.value:
                        max_length = max(max_length, len(str(cell.value)))
                ws.column_dimensions[col_letter].width = min(max_length + 2, 50)

        wb.save(path)
        self.logger.info("generate: Excel 文档已生成, path=%s, 工作表数=%d", path, len(sheets))
        return {
            "path": path,
            "sheet_count": len(sheets),
            "message": f"Excel 文档已生成: {path}",
        }

    def read(self, params: dict) -> dict:
        """读取 Excel 文档

        params:
            path: 文件路径
            sheet: 工作表名称（可选，默认读取所有）
            range: 读取范围（可选，如 "A1:D10"）
        """
        path = params.get("path", "")
        sheet_name = params.get("sheet", None)
        read_range = params.get("range", None)
        if not path:
            self.logger.error("read: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("read: 开始读取 Excel 文档, path=%s", path)

        # 不使用 data_only 模式，以正确读取通过 ws.cell() 写入的值
        wb = load_workbook(path, data_only=False)
        result = {"sheets": {}}

        if sheet_name:
            sheets_to_read = [sheet_name]
        else:
            sheets_to_read = wb.sheetnames

        for name in sheets_to_read:
            if name not in wb.sheetnames:
                continue
            ws = wb[name]

            if read_range:
                rows = []
                for row in ws[read_range]:
                    rows.append([cell.value for cell in row])
            else:
                # 使用 iter_rows 遍历实际有数据的区域
                rows = []
                for row in ws.iter_rows(min_row=1, max_row=ws.max_row, max_col=ws.max_column, values_only=False):
                    row_data = []
                    for cell in row:
                        # 优先取 value，如果 value 为 None 则取缓存的计算值
                        val = cell.value
                        if val is None and cell.data_type == "f":
                            # 公式单元格，尝试取缓存值
                            val = cell.internal_value
                        row_data.append(val)
                    rows.append(row_data)

            result["sheets"][name] = {
                "data": rows,
                "row_count": ws.max_row,
                "col_count": ws.max_column,
            }

        result["sheet_names"] = wb.sheetnames
        self.logger.info("read: Excel 文档读取完成, path=%s, 工作表数=%d", path, len(result["sheets"]))
        return result

    def modify(self, params: dict) -> dict:
        """修改 Excel 文档

        params:
            path: 文件路径
            operations: 修改操作列表
        """
        path = params.get("path", "")
        operations = params.get("operations", [])
        if not path:
            self.logger.error("modify: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("modify: 开始修改 Excel 文档, path=%s, 操作数=%d", path, len(operations))

        wb = load_workbook(path)
        modified_count = 0

        for op in operations:
            op_type = op.get("type", "")

            if op_type == "set_cell":
                sheet = op.get("sheet", wb.active.title if wb.active else "Sheet1")
                if sheet in wb.sheetnames:
                    ws = wb[sheet]
                    row = op.get("row", 1)
                    col = op.get("col", 1)
                    value = op.get("value", "")
                    ws.cell(row=row, column=col, value=value)
                    modified_count += 1

            elif op_type == "add_sheet":
                name = op.get("name", f"Sheet{len(wb.sheetnames) + 1}")
                if name not in wb.sheetnames:
                    wb.create_sheet(title=name)
                    modified_count += 1

            elif op_type == "delete_sheet":
                name = op.get("name", "")
                if name in wb.sheetnames and len(wb.sheetnames) > 1:
                    wb.remove(wb[name])
                    modified_count += 1

            elif op_type == "set_range":
                sheet = op.get("sheet", wb.active.title if wb.active else "Sheet1")
                if sheet in wb.sheetnames:
                    ws = wb[sheet]
                else:
                    ws = wb.create_sheet(title=sheet)
                start_row = op.get("start_row", 1)
                start_col = op.get("start_col", 1)
                data = op.get("data", [])
                for i, row_data in enumerate(data):
                    for j, value in enumerate(row_data):
                        ws.cell(row=start_row + i, column=start_col + j, value=value)
                    modified_count += 1

        wb.save(path)
        self.logger.info("modify: Excel 文档修改完成, path=%s, 修改数=%d", path, modified_count)
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
            format: 目标格式（csv, pdf, html, txt）
            sheet: 工作表名称（可选，默认转换所有工作表）
        """
        path = params.get("path", "")
        output_path = params.get("output_path", "")
        target_format = params.get("format", "").lower()
        sheet_name = params.get("sheet", None)

        if not path:
            self.logger.error("convert: 缺少源文件路径")
            return {"error": "缺少源文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)
        if target_format not in ("csv", "pdf", "html", "txt"):
            self.logger.error("convert: 不支持的目标格式: %s", target_format)
            return {"error": f"不支持的目标格式: {target_format}，支持的格式: csv, pdf, html, txt"}

        self.logger.info("convert: 开始格式转换, path=%s, format=%s", path, target_format)

        # 读取 Excel 文件
        wb = load_workbook(path, data_only=True)

        # 确定要转换的工作表
        if sheet_name:
            if sheet_name not in wb.sheetnames:
                self.logger.error("convert: 工作表不存在: %s", sheet_name)
                return {"error": f"工作表不存在: {sheet_name}"}
            sheets_to_convert = [sheet_name]
        else:
            sheets_to_convert = wb.sheetnames

        # 读取所有工作表数据
        all_sheets_data = {}
        for name in sheets_to_convert:
            ws = wb[name]
            rows = []
            for row in ws.iter_rows(min_row=1, max_row=ws.max_row, max_col=ws.max_column, values_only=True):
                rows.append([cell if cell is not None else "" for cell in row])
            all_sheets_data[name] = rows

        # PDF 为二进制格式，必须写入文件；若未提供 output_path 则自动生成
        if target_format == "pdf":
            if not output_path:
                base, _ = os.path.splitext(path)
                output_path = base + ".pdf"
            self._convert_to_pdf(all_sheets_data, output_path)
            self.logger.info("convert: 格式转换完成, output_path=%s, format=%s", output_path, target_format)
            return {
                "path": output_path,
                "format": target_format,
                "message": f"已转换为 {target_format} 格式",
            }

        # 根据目标格式生成文本内容
        if target_format == "csv":
            content = self._convert_to_csv(all_sheets_data)
        elif target_format == "html":
            content = self._convert_to_html(all_sheets_data)
        elif target_format == "txt":
            content = self._convert_to_txt(all_sheets_data)

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

    def analyze(self, params: dict) -> dict:
        """分析 Excel 文档"""
        path = params.get("path", "")
        if not path:
            self.logger.error("analyze: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("analyze: 开始分析 Excel 文档, path=%s", path)

        wb = load_workbook(path, data_only=True)
        sheets_info = []
        for name in wb.sheetnames:
            ws = wb[name]
            sheets_info.append({
                "name": name,
                "rows": ws.max_row,
                "cols": ws.max_column,
            })

        self.logger.info("analyze: Excel 文档分析完成, path=%s, 工作表数=%d", path, len(wb.sheetnames))
        return {
            "file_size": os.path.getsize(path),
            "sheet_count": len(wb.sheetnames),
            "sheets": sheets_info,
        }

    def _convert_to_csv(self, all_sheets_data: dict) -> str:
        """将工作表数据转换为 CSV 格式

        多个工作表时，以注释行标注工作表名称分隔
        """
        parts = []
        for sheet_name, rows in all_sheets_data.items():
            if len(all_sheets_data) > 1:
                parts.append(f"# 工作表: {sheet_name}")
            output = io.StringIO()
            writer = csv.writer(output)
            for row in rows:
                writer.writerow(row)
            parts.append(output.getvalue().rstrip("\r\n"))

        return "\n".join(parts)

    def _convert_to_pdf(self, all_sheets_data: dict, output_path: str) -> None:
        """将工作表数据转换为 PDF 格式（使用 reportlab 渲染表格）"""
        try:
            from reportlab.lib.pagesizes import A4
            from reportlab.lib.styles import getSampleStyleSheet, ParagraphStyle
            from reportlab.lib.units import cm
            from reportlab.lib import colors
            from reportlab.platypus import SimpleDocTemplate, Paragraph, Spacer, Table, TableStyle
        except ImportError:
            self.logger.error("convert: reportlab 未安装，无法转换为 PDF")
            raise RuntimeError("reportlab 未安装，无法转换为 PDF")

        # 注册中文字体
        from handlers.font_utils import register_chinese_font
        font_name = register_chinese_font()

        os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)

        doc = SimpleDocTemplate(output_path, pagesize=A4)
        styles = getSampleStyleSheet()

        # 工作表标题样式
        sheet_title_style = ParagraphStyle(
            "SheetTitle",
            parent=styles["Normal"],
            fontName=font_name,
            fontSize=14,
            spaceAfter=10,
            spaceBefore=20,
        )

        elements = []
        for sheet_name, rows in all_sheets_data.items():
            # 添加工作表名称作为标题
            elements.append(Paragraph(html.escape(sheet_name), sheet_title_style))
            elements.append(Spacer(1, 0.5 * cm))

            if rows:
                # 将所有单元格转为字符串
                table_data = [[str(cell) for cell in row] for row in rows]

                # 创建 PDF 表格
                table = Table(table_data)
                table.setStyle(TableStyle([
                    ("GRID", (0, 0), (-1, -1), 0.5, colors.grey),
                    ("FONTNAME", (0, 0), (-1, -1), font_name),
                    ("FONTSIZE", (0, 0), (-1, -1), 8),
                    ("ALIGN", (0, 0), (-1, -1), "CENTER"),
                    ("VALIGN", (0, 0), (-1, -1), "MIDDLE"),
                    # 首行（表头）加粗背景
                    ("BACKGROUND", (0, 0), (-1, 0), colors.Color(0.9, 0.9, 0.9)),
                    ("FONTNAME", (0, 0), (-1, 0), font_name),
                    ("FONTSIZE", (0, 0), (-1, 0), 9),
                ]))
                elements.append(table)
                elements.append(Spacer(1, 1 * cm))

        doc.build(elements)

    def _convert_to_html(self, all_sheets_data: dict) -> str:
        """将工作表数据转换为 HTML 表格"""
        parts = [
            "<!DOCTYPE html>",
            "<html>",
            "<head>",
            '<meta charset="utf-8">',
            "<title>Excel 转换</title>",
            "<style>",
            "body { font-family: sans-serif; margin: 20px; }",
            "table { border-collapse: collapse; margin-bottom: 20px; width: 100%; }",
            "th, td { border: 1px solid #ccc; padding: 6px 10px; text-align: left; }",
            "th { background-color: #f0f0f0; font-weight: bold; }",
            "h2 { margin-top: 30px; color: #333; }",
            "</style>",
            "</head>",
            "<body>",
        ]

        for sheet_name, rows in all_sheets_data.items():
            parts.append(f"<h2>{html.escape(sheet_name)}</h2>")
            parts.append("<table>")
            for i, row in enumerate(rows):
                # 首行使用 <th> 标签，其余使用 <td>
                tag = "th" if i == 0 else "td"
                parts.append("<tr>")
                for cell in row:
                    parts.append(f"<{tag}>{html.escape(str(cell))}</{tag}>")
                parts.append("</tr>")
            parts.append("</table>")

        parts.extend(["</body>", "</html>"])
        return "\n".join(parts)

    def _convert_to_txt(self, all_sheets_data: dict) -> str:
        """将工作表数据转换为纯文本（制表符分隔）"""
        parts = []
        for sheet_name, rows in all_sheets_data.items():
            if len(all_sheets_data) > 1:
                parts.append(f"=== {sheet_name} ===")
            for row in rows:
                parts.append("\t".join(str(cell) for cell in row))
            # 工作表之间空行分隔
            parts.append("")

        return "\n".join(parts)
