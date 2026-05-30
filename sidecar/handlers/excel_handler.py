"""Excel 文档处理器
基于 openpyxl 实现 Excel 文档的生成、读取、修改
遵循 xlsx Skill 规范：公式优先、数字格式标准、颜色编码、条件格式、多工作表
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
from openpyxl.formatting.rule import CellIsRule


class ExcelHandler:
    """Excel (.xlsx) 文档处理器"""

    logger = logging.getLogger(__name__)

    # 数字格式映射表，遵循 Skill 规范
    NUMBER_FORMAT_MAP = {
        "currency": "$#,##0",
        "currency_decimal": "$#,##0.00",
        "percent": "0.0%",
        "text": "@",
        "number": "#,##0",
        "number_decimal": "#,##0.00",
        # 零值显示为 "-"
        "zero_dash": '#,##0;(#,##0);"-"',
        # 倍数格式
        "multiple": "0.0x",
        # 负数使用括号
        "negative_paren": "#,##0;(#,##0)",
    }

    # 颜色编码映射表：类型 -> (字体颜色, 背景填充)
    # 遵循 Skill 规范：蓝色字体=输入、黑色字体=公式、绿色字体=跨表引用、红色字体=外部链接、黄色背景=假设
    COLOR_CODING_MAP = {
        "input": ("0000FF", None),
        "formula": ("000000", None),
        "cross_ref": ("008000", None),
        "external": ("FF0000", None),
        "assumption": (None, "FFFF00"),
    }

    # 表头样式：粗体居中
    HEADER_FONT = Font(bold=True)
    HEADER_ALIGNMENT = Alignment(horizontal="center")

    def generate(self, params: dict) -> dict:
        """生成 Excel 文档

        遵循 Skill 规范核心原则：使用公式而非硬编码值

        params:
            path: 输出文件路径
            sheets: 工作表列表
                [{"name": "Sheet1",
                  "data": [[...]],
                  "headers": [...],
                  "cells": [{row, col, value, formula, colorType}],
                  "formulas": [{row, col, formula, colorType}]}]
            content: 文档内容（当 sheets 为空时，从 content 构建）
            title: 文档标题（当 sheets 为空时，作为默认工作表名）
            useFormulas: 是否使用公式（默认 true）
            numberFormats: 数字格式列表 [{range, format, sheet}]
            colorCoding: 是否启用颜色编码（默认 true）
            conditionalFormats: 条件格式列表 [{range, rule, value, color, sheet}]
        """
        path = params.get("path", "")
        sheets = params.get("sheets", [])
        content = params.get("content", "")
        title = params.get("title", "")
        use_formulas = params.get("useFormulas", True)
        number_formats = params.get("numberFormats", [])
        color_coding = params.get("colorCoding", True)
        conditional_formats = params.get("conditionalFormats", [])

        if not path:
            self.logger.error("generate: 缺少输出文件路径")
            return {"error": "缺少输出文件路径"}

        # 当 sheets 为空但 content 非空时，从 content 构建默认工作表
        if not sheets and content:
            self.logger.info("generate: sheets 为空，从 content 参数构建默认工作表")
            sheet_name = title if title else "Sheet1"
            rows = []
            for line in content.split("\n"):
                line = line.strip()
                if line:
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
            # cells 字段，支持 {row, col, value, formula, colorType}
            cells = sheet_info.get("cells", [])
            # formulas 字段，支持 {row, col, formula, colorType}
            formulas = sheet_info.get("formulas", [])

            ws = wb.create_sheet(title=sheet_name)

            # 写入表头（粗体居中）
            if headers:
                for col_idx, header in enumerate(headers, 1):
                    cell = ws.cell(row=1, column=col_idx, value=header)
                    cell.font = self.HEADER_FONT
                    cell.alignment = self.HEADER_ALIGNMENT

            # 写入数据
            start_row = 2 if headers else 1
            for row_idx, row_data in enumerate(data, start_row):
                for col_idx, value in enumerate(row_data, 1):
                    ws.cell(row=row_idx, column=col_idx, value=value)

            # 写入 cells 字段中的单元格（支持公式和颜色编码）
            if cells:
                for cell_info in cells:
                    row = cell_info.get("row", 1)
                    col = cell_info.get("col", 1)
                    formula = cell_info.get("formula", "")
                    value = cell_info.get("value", "")
                    cell_color_type = cell_info.get("colorType", None)

                    # 当 formula 存在且启用公式时，优先写入公式
                    if formula and use_formulas:
                        ws.cell(row=row, column=col, value=formula)
                    else:
                        ws.cell(row=row, column=col, value=value)

                    # 应用颜色编码
                    if color_coding:
                        color_type = cell_color_type
                        if not color_type:
                            # 自动推断：有公式为 formula，否则为 input
                            if formula and use_formulas:
                                color_type = "formula"
                            else:
                                color_type = "input"
                        self._apply_cell_color(ws.cell(row=row, column=col), color_type)

            # 批量写入 formulas 字段中的公式
            if formulas and use_formulas:
                for formula_info in formulas:
                    row = formula_info.get("row", 1)
                    col = formula_info.get("col", 1)
                    formula = formula_info.get("formula", "")
                    cell_color_type = formula_info.get("colorType", "formula")
                    if formula:
                        cell = ws.cell(row=row, column=col, value=formula)
                        # 公式单元格默认黑色字体
                        if color_coding:
                            self._apply_cell_color(cell, cell_color_type)

            # 自动调整列宽
            self._auto_adjust_column_width(ws)

        # 应用数字格式（按工作表指定范围）
        if number_formats:
            for fmt_info in number_formats:
                fmt_sheet = fmt_info.get("sheet", None)
                for ws in wb.worksheets:
                    if fmt_sheet and ws.title != fmt_sheet:
                        continue
                    self._apply_number_formats(ws, [fmt_info])

        # 应用条件格式（按工作表指定范围）
        if conditional_formats:
            for fmt_info in conditional_formats:
                fmt_sheet = fmt_info.get("sheet", None)
                for ws in wb.worksheets:
                    if fmt_sheet and ws.title != fmt_sheet:
                        continue
                    self._apply_conditional_formats(ws, [fmt_info])

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
                rows = []
                for row in ws.iter_rows(min_row=1, max_row=ws.max_row, max_col=ws.max_column, values_only=False):
                    row_data = []
                    for cell in row:
                        val = cell.value
                        if val is None and cell.data_type == "f":
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
                - set_cell: 设置单元格值
                - add_sheet: 添加工作表
                - delete_sheet: 删除工作表
                - set_range: 设置区域数据
                - setFormula: 设置公式
                - setFormat: 设置数字格式
                - setColorCoding: 设置颜色编码
                - addConditionalFormat: 添加条件格式
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

            elif op_type == "setFormula":
                sheet = op.get("sheet", wb.active.title if wb.active else "Sheet1")
                if sheet in wb.sheetnames:
                    ws = wb[sheet]
                    row = op.get("row", 1)
                    col = op.get("col", 1)
                    formula = op.get("formula", "")
                    if formula:
                        ws.cell(row=row, column=col, value=formula)
                        modified_count += 1
                        self.logger.info("modify: setFormula - sheet=%s, row=%d, col=%d, formula=%s",
                                         sheet, row, col, formula)

            elif op_type == "setFormat":
                sheet = op.get("sheet", wb.active.title if wb.active else "Sheet1")
                if sheet in wb.sheetnames:
                    ws = wb[sheet]
                    fmt_range = op.get("range", "")
                    fmt = op.get("format", "")
                    if fmt_range and fmt:
                        self._apply_number_formats(ws, [{"range": fmt_range, "format": fmt}])
                        modified_count += 1
                        self.logger.info("modify: setFormat - sheet=%s, range=%s, format=%s",
                                         sheet, fmt_range, fmt)

            elif op_type == "setColorCoding":
                sheet = op.get("sheet", wb.active.title if wb.active else "Sheet1")
                if sheet in wb.sheetnames:
                    ws = wb[sheet]
                    color_range = op.get("range", "")
                    color_type = op.get("colorType", "input")
                    if color_range:
                        self._apply_color_coding_by_range(ws, color_range, color_type)
                        modified_count += 1
                        self.logger.info("modify: setColorCoding - sheet=%s, range=%s, colorType=%s",
                                         sheet, color_range, color_type)

            elif op_type == "addConditionalFormat":
                sheet = op.get("sheet", wb.active.title if wb.active else "Sheet1")
                if sheet in wb.sheetnames:
                    ws = wb[sheet]
                    fmt_range = op.get("range", "")
                    rule = op.get("rule", "")
                    value = op.get("value", "")
                    color = op.get("color", "FF0000")
                    if fmt_range and rule:
                        self._apply_conditional_formats(ws, [{
                            "range": fmt_range,
                            "rule": rule,
                            "value": value,
                            "color": color,
                        }])
                        modified_count += 1
                        self.logger.info("modify: addConditionalFormat - sheet=%s, range=%s, rule=%s",
                                         sheet, fmt_range, rule)

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
            sheet: 工作表名称（可选，默认转换所有）
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

        # PDF 为二进制格式，必须写入文件
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

    # ------------------------------------------------------------------ #
    #  颜色编码辅助方法（Skill 规范）
    # ------------------------------------------------------------------ #

    def _apply_cell_color(self, cell, color_type: str):
        """为单个单元格应用颜色编码

        遵循 Skill 规范：
        - 蓝色字体(0000FF): 输入值
        - 黑色字体(000000): 公式
        - 绿色字体(008000): 跨表引用
        - 红色字体(FF0000): 外部链接
        - 黄色背景(FFFF00): 假设值

        Args:
            cell: openpyxl 单元格对象
            color_type: 颜色类型
        """
        font_color, bg_color = self.COLOR_CODING_MAP.get(color_type, (None, None))

        if font_color:
            cell.font = Font(color=font_color)
        if bg_color:
            cell.fill = PatternFill(start_color=bg_color, end_color=bg_color, fill_type="solid")

    def _apply_color_coding_by_range(self, ws, cell_range: str, color_type: str):
        """按范围应用颜色编码

        Args:
            ws: openpyxl 工作表对象
            cell_range: 单元格范围（如 "B2" 或 "B2:D10"）
            color_type: 颜色类型
        """
        font_color, bg_color = self.COLOR_CODING_MAP.get(color_type, (None, None))
        if not font_color and not bg_color:
            self.logger.warning("_apply_color_coding_by_range: 未知颜色类型: %s", color_type)
            return

        try:
            for row in ws[cell_range]:
                for cell in row:
                    if font_color:
                        cell.font = Font(color=font_color)
                    if bg_color:
                        cell.fill = PatternFill(start_color=bg_color, end_color=bg_color, fill_type="solid")
        except Exception as e:
            self.logger.warning("_apply_color_coding_by_range: 应用颜色编码失败, range=%s, colorType=%s, error=%s",
                                cell_range, color_type, e)

    # ------------------------------------------------------------------ #
    #  数字格式（Skill 规范）
    # ------------------------------------------------------------------ #

    def _apply_number_formats(self, ws, formats: list):
        """应用数字格式到工作表

        遵循 Skill 规范：
        - 货币: $#,##0
        - 百分比: 0.0%
        - 文本: @
        - 数字: #,##0
        - 零值显示: "-"
        - 倍数: 0.0x

        Args:
            ws: openpyxl 工作表对象
            formats: 数字格式列表 [{range, format}]
        """
        for fmt_info in formats:
            fmt_range = fmt_info.get("range", "")
            fmt_name = fmt_info.get("format", "")
            if not fmt_range or not fmt_name:
                continue

            # 查找预设格式，未找到则视为自定义格式字符串
            number_format = self.NUMBER_FORMAT_MAP.get(fmt_name, fmt_name)

            try:
                for row in ws[fmt_range]:
                    for cell in row:
                        cell.number_format = number_format
            except Exception as e:
                self.logger.warning("_apply_number_formats: 应用数字格式失败, range=%s, format=%s, error=%s",
                                    fmt_range, fmt_name, e)

    # ------------------------------------------------------------------ #
    #  条件格式
    # ------------------------------------------------------------------ #

    def _apply_conditional_formats(self, ws, formats: list):
        """应用条件格式到工作表

        Args:
            ws: openpyxl 工作表对象
            formats: 条件格式列表 [{range, rule, value, color}]
                     rule 支持: greaterThan, lessThan, equal, notEqual,
                     greaterThanOrEqual, lessThanOrEqual, between, notBetween
        """
        rule_map = {
            "greaterThan": "greaterThan",
            "lessThan": "lessThan",
            "equal": "equal",
            "notEqual": "notEqual",
            "greaterThanOrEqual": "greaterThanOrEqual",
            "lessThanOrEqual": "lessThanOrEqual",
            "between": "between",
            "notBetween": "notBetween",
        }

        for fmt_info in formats:
            fmt_range = fmt_info.get("range", "")
            rule = fmt_info.get("rule", "")
            value = fmt_info.get("value", "")
            color = fmt_info.get("color", "FF0000")

            if not fmt_range or not rule:
                continue

            operator = rule_map.get(rule)
            if not operator:
                self.logger.warning("_apply_conditional_formats: 不支持的条件规则: %s", rule)
                continue

            try:
                fill = PatternFill(start_color=color, end_color=color, fill_type="solid")
                font = Font(color="FFFFFF")

                # between/notBetween 需要 formula 为列表形式 [val1, val2]
                if operator in ("between", "notBetween"):
                    if isinstance(value, list) and len(value) >= 2:
                        formula = [str(value[0]), str(value[1])]
                    else:
                        self.logger.warning("_apply_conditional_formats: between/notBetween 规则需要 value 为列表 [val1, val2]")
                        continue
                else:
                    formula = [str(value)]

                conditional_rule = CellIsRule(
                    operator=operator,
                    formula=formula,
                    fill=fill,
                    font=font,
                )
                ws.conditional_formatting.add(fmt_range, conditional_rule)
                self.logger.info("_apply_conditional_formats: 已添加条件格式, range=%s, rule=%s, value=%s",
                                 fmt_range, rule, value)
            except Exception as e:
                self.logger.warning("_apply_conditional_formats: 添加条件格式失败, range=%s, rule=%s, error=%s",
                                    fmt_range, rule, e)

    # ------------------------------------------------------------------ #
    #  列宽自动调整
    # ------------------------------------------------------------------ #

    def _auto_adjust_column_width(self, ws):
        """自动调整工作表列宽

        Args:
            ws: openpyxl 工作表对象
        """
        for col in ws.columns:
            max_length = 0
            col_letter = get_column_letter(col[0].column)
            for cell in col:
                if cell.value:
                    max_length = max(max_length, len(str(cell.value)))
            ws.column_dimensions[col_letter].width = min(max_length + 2, 50)

    # ------------------------------------------------------------------ #
    #  格式转换辅助方法
    # ------------------------------------------------------------------ #

    def _convert_to_csv(self, all_sheets_data: dict) -> str:
        """将工作表数据转换为 CSV 格式"""
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

    def _convert_to_pdf(self, all_sheets_data: dict, output_path: str):
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

        from handlers.font_utils import register_chinese_font
        font_name = register_chinese_font()

        os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)

        doc = SimpleDocTemplate(output_path, pagesize=A4)
        styles = getSampleStyleSheet()

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
            elements.append(Paragraph(html.escape(sheet_name), sheet_title_style))
            elements.append(Spacer(1, 0.5 * cm))

            if rows:
                table_data = [[str(cell) for cell in row] for row in rows]

                table = Table(table_data)
                table.setStyle(TableStyle([
                    ("GRID", (0, 0), (-1, -1), 0.5, colors.grey),
                    ("FONTNAME", (0, 0), (-1, -1), font_name),
                    ("FONTSIZE", (0, 0), (-1, -1), 8),
                    ("ALIGN", (0, 0), (-1, -1), "CENTER"),
                    ("VALIGN", (0, 0), (-1, -1), "MIDDLE"),
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
            parts.append("")

        return "\n".join(parts)
