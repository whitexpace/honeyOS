#!/usr/bin/env python3
"""Build the HoneyOs user manual as styled HTML and PDF.

No Pandoc, LaTeX, or Markdown package is required. The script implements the
small Markdown subset used by docs/USER_MANUAL.md and writes a self-contained
PDF with built-in PDF fonts and embedded PNG screenshots.
"""

from __future__ import annotations

import html
import re
import struct
import zlib
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MANUAL_MD = ROOT / "docs" / "USER_MANUAL.md"
MANUAL_HTML = ROOT / "docs" / "HoneyOs-User-Manual.html"
MANUAL_PDF = ROOT / "docs" / "HoneyOs-User-Manual.pdf"

PAGE_W = 595.28
PAGE_H = 841.89
MARGIN_L = 46.0
MARGIN_R = 46.0
MARGIN_T = 46.0
MARGIN_B = 50.0
CONTENT_W = PAGE_W - MARGIN_L - MARGIN_R
CODE_PAD_X = 2.2
CODE_PAD_Y = 1.1

# Pixel crop rectangles, measured from the top-left of the source screenshot.
# The source screenshots are full-desktop captures; these crops keep the PDF
# focused on the relevant VirtualBox dialog or HoneyOs screen.
IMAGE_CROPS: dict[str, tuple[int, int, int, int]] = {
    "images/VM-Config-2.png": (555, 340, 810, 400),
    "images/attach-os-vdi.png": (535, 292, 850, 498),
    "images/desktop-view.png": (650, 390, 620, 315),
    "images/new-file.png": (650, 380, 620, 335),
    "images/edit-file.png": (640, 333, 640, 385),
    "images/rename-file.png": (640, 333, 640, 385),
    "images/file-alloc-table.png": (650, 386, 620, 330),
    "images/file-alloc-table-after-deleting-a-file.png": (650, 386, 620, 330),
}


@dataclass
class Block:
    kind: str
    value: object


@dataclass
class TextItem:
    text: str
    style: str = "normal"
    link: str | None = None


@dataclass
class PngImage:
    width: int
    height: int
    colors: int
    color_space: str
    data: bytes


@dataclass
class LinkAnnotation:
    x1: float
    y1: float
    x2: float
    y2: float
    target: str


@dataclass
class Page:
    ops: list[str]
    images: set[str]
    annots: list[LinkAnnotation]


def strip_inline(text: str) -> str:
    text = re.sub(r"\[([^\]]+)\]\(#[^)]+\)", r"\1", text)
    text = re.sub(r"`([^`]+)`", r"\1", text)
    text = re.sub(r"\*\*([^*]+)\*\*", r"\1", text)
    return text


def inline_markup(text: str) -> str:
    escaped = html.escape(text)
    escaped = re.sub(r"\[([^\]]+)\]\((#[^)]+)\)", r"<a href=\"\2\">\1</a>", escaped)
    escaped = re.sub(r"`([^`]+)`", r"<code>\1</code>", escaped)
    escaped = re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", escaped)
    return escaped


def slugify(text: str) -> str:
    slug = re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")
    return slug or "section"


def is_table_separator(line: str) -> bool:
    cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
    return bool(cells) and all(re.fullmatch(r":?-{3,}:?", cell) for cell in cells)


def parse_table(lines: list[str], start: int) -> tuple[dict[str, object], int]:
    header = [cell.strip() for cell in lines[start].strip().strip("|").split("|")]
    rows: list[list[str]] = []
    i = start + 2
    while i < len(lines) and lines[i].strip().startswith("|"):
        rows.append([cell.strip() for cell in lines[i].strip().strip("|").split("|")])
        i += 1
    return {"header": header, "rows": rows}, i


def parse_markdown_blocks(md: str) -> list[Block]:
    lines = md.splitlines()
    blocks: list[Block] = []
    i = 0

    while i < len(lines):
        stripped = lines[i].strip()
        if not stripped:
            i += 1
            continue

        if stripped == "<!-- pagebreak -->":
            blocks.append(Block("pagebreak", None))
            i += 1
            continue

        if stripped.startswith("```"):
            language = stripped[3:].strip()
            code_lines: list[str] = []
            i += 1
            while i < len(lines) and not lines[i].strip().startswith("```"):
                code_lines.append(lines[i])
                i += 1
            i += 1
            blocks.append(Block("code", {"language": language, "text": "\n".join(code_lines)}))
            continue

        heading = re.match(r"^(#{1,3})\s+(.+)$", stripped)
        if heading:
            blocks.append(
                Block("heading", {"level": len(heading.group(1)), "text": heading.group(2).strip()})
            )
            i += 1
            continue

        image = re.match(r"!\[([^\]]*)\]\(([^)]+)\)", stripped)
        if image:
            blocks.append(Block("image", {"alt": image.group(1), "src": image.group(2)}))
            i += 1
            continue

        if (
            stripped.startswith("|")
            and i + 1 < len(lines)
            and lines[i + 1].strip().startswith("|")
            and is_table_separator(lines[i + 1])
        ):
            table, i = parse_table(lines, i)
            blocks.append(Block("table", table))
            continue

        if re.match(r"^\d+\.\s+(.+)$", stripped):
            items: list[str] = []
            while i < len(lines):
                match = re.match(r"^\d+\.\s+(.+)$", lines[i].strip())
                if not match:
                    break
                items.append(match.group(1))
                i += 1
            blocks.append(Block("ordered", items))
            continue

        if stripped.startswith("- "):
            items = []
            while i < len(lines) and lines[i].strip().startswith("- "):
                items.append(lines[i].strip()[2:])
                i += 1
            blocks.append(Block("unordered", items))
            continue

        paragraph = [stripped]
        i += 1
        while i < len(lines):
            next_line = lines[i].strip()
            if not next_line:
                break
            if (
                next_line.startswith("#")
                or next_line.startswith("```")
                or next_line == "<!-- pagebreak -->"
                or next_line.startswith("- ")
                or re.match(r"^\d+\.\s+", next_line)
                or next_line.startswith("|")
                or next_line.startswith("![")
            ):
                break
            paragraph.append(next_line)
            i += 1

        text = " ".join(paragraph)
        if text.startswith("Caption:"):
            blocks.append(Block("caption", text))
        elif text.startswith("Important:"):
            blocks.append(Block("important", text))
        else:
            blocks.append(Block("paragraph", text))

    return blocks


def blocks_to_html(blocks: list[Block]) -> str:
    out: list[str] = []
    for block in blocks:
        if block.kind == "heading":
            data = block.value
            assert isinstance(data, dict)
            level = int(data["level"])
            text = str(data["text"])
            out.append(f"<h{level} id=\"{slugify(text)}\">{inline_markup(text)}</h{level}>")
        elif block.kind == "paragraph":
            out.append(f"<p>{inline_markup(str(block.value))}</p>")
        elif block.kind == "important":
            out.append(f"<p class=\"callout important\">{inline_markup(str(block.value))}</p>")
        elif block.kind == "caption":
            out.append(f"<p class=\"caption\">{inline_markup(str(block.value))}</p>")
        elif block.kind == "unordered":
            out.append("<ul>")
            for item in block.value:
                out.append(f"<li>{inline_markup(str(item))}</li>")
            out.append("</ul>")
        elif block.kind == "ordered":
            out.append("<ol>")
            for item in block.value:
                out.append(f"<li>{inline_markup(str(item))}</li>")
            out.append("</ol>")
        elif block.kind == "code":
            data = block.value
            assert isinstance(data, dict)
            language = str(data["language"])
            lang_class = f" class=\"language-{html.escape(language)}\"" if language else ""
            out.append(f"<pre><code{lang_class}>{html.escape(str(data['text']))}</code></pre>")
        elif block.kind == "image":
            data = block.value
            assert isinstance(data, dict)
            out.append(
                "<figure>"
                f"<img src=\"{html.escape(str(data['src']))}\" alt=\"{html.escape(str(data['alt']))}\">"
                "</figure>"
            )
        elif block.kind == "table":
            data = block.value
            assert isinstance(data, dict)
            out.extend(["<table>", "<thead>", "<tr>"])
            for cell in data["header"]:
                out.append(f"<th>{inline_markup(str(cell))}</th>")
            out.extend(["</tr>", "</thead>", "<tbody>"])
            for row in data["rows"]:
                out.append("<tr>")
                for cell in row:
                    out.append(f"<td>{inline_markup(str(cell))}</td>")
                out.append("</tr>")
            out.extend(["</tbody>", "</table>"])
        elif block.kind == "pagebreak":
            out.append("<div class=\"pagebreak\"></div>")
    return "\n".join(out)


HTML_CSS = """
@page { size: A4; margin: 18mm 17mm 20mm; }
body {
  margin: 0;
  color: #172033;
  background: #ffffff;
  font: 10.5pt/1.45 "Times New Roman", Times, serif;
}
main { max-width: 780px; margin: 0 auto; }
h1, h2, h3 { color: #102a43; line-height: 1.2; page-break-after: avoid; }
h1 {
  margin: 0 0 10px;
  padding: 22px 24px;
  color: #ffffff;
  background: linear-gradient(135deg, #12355b, #087f8c);
  border-radius: 10px;
  font-size: 30pt;
}
h2 { margin-top: 26px; padding-bottom: 5px; border-bottom: 2px solid #d6e2f0; }
h3 { margin-top: 18px; }
p { margin: 8px 0; }
.callout { margin: 14px 0; padding: 10px 12px; border-left: 4px solid #087f8c; background: #eef9fb; border-radius: 6px; }
.important { border-left-color: #b45309; background: #fff7ed; }
ul, ol { margin: 7px 0 11px 22px; padding: 0; }
li { margin: 3px 0; }
table { width: 100%; margin: 11px 0 16px; border-collapse: collapse; page-break-inside: avoid; font-size: 9.5pt; }
th { color: #ffffff; background: #12355b; text-align: left; }
th, td { padding: 7px 8px; border: 1px solid #cbd5e1; vertical-align: top; }
tr:nth-child(even) td { background: #f8fafc; }
code {
  font-family: "JetBrains Mono", "JetBrainsMono Nerd Font", "Courier New", Courier, monospace;
  font-size: 0.92em;
  line-height: 1.25;
  color: #0f3d5f;
  background: #eef3f8;
  border: 1px solid #d7e0ea;
  border-radius: 4px;
  padding: 0.08em 0.30em;
  overflow-wrap: anywhere;
  box-decoration-break: clone;
  -webkit-box-decoration-break: clone;
}
pre {
  margin: 10px 0 14px;
  padding: 11px 13px;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  color: #e5eef8;
  background: #0b1220;
  border-radius: 8px;
  page-break-inside: avoid;
  font-family: "JetBrains Mono", "JetBrainsMono Nerd Font", "Courier New", Courier, monospace;
  font-size: 9pt;
  line-height: 1.35;
}
pre code { font-family: inherit; font-size: inherit; color: inherit; background: transparent; border: 0; padding: 0; }
figure { margin: 14px 0 6px; page-break-inside: avoid; }
img { display: block; width: 100%; max-height: 132mm; object-fit: contain; border: 1px solid #b9c6d3; border-radius: 8px; box-shadow: 0 2px 8px rgba(15, 23, 42, 0.12); }
.caption { margin: 4px 0 16px; color: #536579; font-size: 9pt; font-style: italic; }
.pagebreak { break-after: page; page-break-after: always; }
a { color: #075985; text-decoration: none; }
"""


def write_html(blocks: list[Block]) -> None:
    body = blocks_to_html(blocks)
    MANUAL_HTML.write_text(
        f"""<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>HoneyOs User Manual</title>
  <style>{HTML_CSS}</style>
</head>
<body>
  <main>
{body}
  </main>
</body>
</html>
""",
        encoding="utf-8",
    )


def pdf_escape(text: str) -> str:
    return (
        text.encode("cp1252", errors="replace")
        .decode("cp1252")
        .replace("\\", "\\\\")
        .replace("(", "\\(")
        .replace(")", "\\)")
        .replace("\r", "")
        .replace("\n", "\\n")
    )


def color(values: tuple[float, float, float]) -> str:
    return f"{values[0]:.3f} {values[1]:.3f} {values[2]:.3f}"


def text_width(text: str, font: str, size: float) -> float:
    if font in {"F4", "F5"}:
        return len(text) * size * 0.60
    total = 0.0
    for ch in text:
        if ch == " ":
            total += 0.34
        elif ch in "il.,;:!|":
            total += 0.25
        elif ch in "fjrt":
            total += 0.36
        elif ch in "mwMW@#%&":
            total += 0.88
        elif ch.isupper():
            total += 0.68
        elif ch.isdigit():
            total += 0.56
        else:
            total += 0.56
    return total * size


def tokenize_inline(text: str) -> list[TextItem]:
    items: list[TextItem] = []
    pattern = re.compile(r"(`[^`]+`|\*\*[^*]+\*\*|\[[^\]]+\]\(#[^)]+\))")
    pos = 0
    for match in pattern.finditer(text):
        if match.start() > pos:
            items.extend(split_words(text[pos : match.start()], "normal"))
        token = match.group(0)
        if token.startswith("`"):
            items.append(TextItem(token[1:-1], "code"))
        elif token.startswith("["):
            link = re.match(r"\[([^\]]+)\]\(#([^)]+)\)", token)
            if link:
                items.extend(split_words(link.group(1), "link", link.group(2)))
        else:
            items.extend(split_words(token[2:-2], "bold"))
        pos = match.end()
    if pos < len(text):
        items.extend(split_words(text[pos:], "normal"))
    return [item for item in items if item.text]


def split_words(text: str, style: str, link: str | None = None) -> list[TextItem]:
    return [TextItem(part, style, link) for part in re.split(r"\s+", text.strip()) if part]


def item_font(item: TextItem) -> str:
    if item.style == "bold":
        return "F2"
    if item.style == "link":
        return "F2"
    if item.style == "code":
        return "F4"
    return "F1"


def item_size(item: TextItem, size: float) -> float:
    if item.style == "code":
        return size * 0.92
    return size


def item_width(item: TextItem, size: float) -> float:
    width = text_width(item.text, item_font(item), item_size(item, size))
    if item.style == "code":
        return width + CODE_PAD_X * 2
    return width


def wrap_text(text: str, width: float, size: float) -> list[list[TextItem]]:
    source = tokenize_inline(text)
    lines: list[list[TextItem]] = []
    line: list[TextItem] = []
    line_w = 0.0
    space_w = text_width(" ", "F1", size)

    for item in source:
        w = item_width(item, size)
        extra = space_w if line else 0.0
        if line and line_w + extra + w > width:
            lines.append(line)
            line = [item]
            line_w = w
        else:
            line.append(item)
            line_w += extra + w

    if line:
        lines.append(line)
    return lines or [[TextItem("")]]


def wrap_plain(text: str, width: float, font: str, size: float) -> list[str]:
    words = re.split(r"\s+", strip_inline(text).strip())
    lines: list[str] = []
    line = ""
    for word in words:
        candidate = word if not line else f"{line} {word}"
        if line and text_width(candidate, font, size) > width:
            lines.append(line)
            line = word
        else:
            line = candidate
    if line:
        lines.append(line)
    return lines or [""]


def wrap_code(text: str, width: float, size: float) -> list[str]:
    max_chars = max(12, int(width / (size * 0.60)))
    out: list[str] = []
    for raw in text.splitlines() or [""]:
        line = raw
        while len(line) > max_chars:
            out.append(line[:max_chars])
            line = line[max_chars:]
        out.append(line)
    return out


def paeth_predictor(left: int, up: int, upper_left: int) -> int:
    p = left + up - upper_left
    pa = abs(p - left)
    pb = abs(p - up)
    pc = abs(p - upper_left)
    if pa <= pb and pa <= pc:
        return left
    if pb <= pc:
        return up
    return upper_left


def read_png(path: Path, crop: tuple[int, int, int, int] | None = None) -> PngImage:
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError(f"not a PNG file: {path}")
    pos = 8
    width = height = bit_depth = color_type = interlace = None
    compressed = bytearray()
    while pos < len(data):
        length = struct.unpack(">I", data[pos : pos + 4])[0]
        chunk_type = data[pos + 4 : pos + 8]
        chunk_data = data[pos + 8 : pos + 8 + length]
        pos += 12 + length
        if chunk_type == b"IHDR":
            width, height, bit_depth, color_type, _, _, interlace = struct.unpack(
                ">IIBBBBB", chunk_data
            )
        elif chunk_type == b"IDAT":
            compressed.extend(chunk_data)
        elif chunk_type == b"IEND":
            break

    if width is None or height is None or bit_depth != 8 or interlace != 0:
        raise ValueError(f"unsupported PNG encoding: {path}")
    if color_type == 2:
        colors = 3
        color_space = "/DeviceRGB"
    elif color_type == 0:
        colors = 1
        color_space = "/DeviceGray"
    else:
        raise ValueError(f"unsupported PNG color type {color_type}: {path}")

    raw = zlib.decompress(bytes(compressed))
    stride = width * colors
    expected = (stride + 1) * height
    if len(raw) != expected:
        raise ValueError(f"unexpected PNG data length in {path}")

    rows: list[bytes] = []
    prev = bytearray(stride)
    pos = 0
    for _ in range(height):
        filter_type = raw[pos]
        pos += 1
        scanline = raw[pos : pos + stride]
        pos += stride
        recon = bytearray(stride)
        for i, value in enumerate(scanline):
            left = recon[i - colors] if i >= colors else 0
            up = prev[i]
            upper_left = prev[i - colors] if i >= colors else 0
            if filter_type == 0:
                recon[i] = value
            elif filter_type == 1:
                recon[i] = (value + left) & 0xFF
            elif filter_type == 2:
                recon[i] = (value + up) & 0xFF
            elif filter_type == 3:
                recon[i] = (value + ((left + up) // 2)) & 0xFF
            elif filter_type == 4:
                recon[i] = (value + paeth_predictor(left, up, upper_left)) & 0xFF
            else:
                raise ValueError(f"unsupported PNG filter {filter_type} in {path}")
        rows.append(bytes(recon))
        prev = recon

    crop_x, crop_y, crop_w, crop_h = crop or (0, 0, width, height)
    crop_x = max(0, min(crop_x, width - 1))
    crop_y = max(0, min(crop_y, height - 1))
    crop_w = max(1, min(crop_w, width - crop_x))
    crop_h = max(1, min(crop_h, height - crop_y))
    start = crop_x * colors
    end = start + crop_w * colors
    cropped = b"".join(row[start:end] for row in rows[crop_y : crop_y + crop_h])
    return PngImage(crop_w, crop_h, colors, color_space, zlib.compress(cropped, 6))


class ManualPdf:
    def __init__(self) -> None:
        self.pages: list[Page] = []
        self.page = self.blank_page()
        self.pages.append(self.page)
        self.y = PAGE_H - MARGIN_T
        self.images: dict[str, PngImage] = {}
        self.image_names: dict[str, str] = {}
        self.destinations: dict[str, tuple[int, float]] = {}
        self.title_mode = False
        self.toc_mode = False

    def blank_page(self) -> Page:
        return Page([f"q 1 1 1 rg 0 0 {PAGE_W:.2f} {PAGE_H:.2f} re f Q"], set(), [])

    def new_page(self) -> None:
        self.page = self.blank_page()
        self.pages.append(self.page)
        self.y = PAGE_H - MARGIN_T
        self.title_mode = False
        self.toc_mode = False

    def ensure(self, height: float) -> None:
        if self.y - height < MARGIN_B:
            self.new_page()

    def op(self, value: str) -> None:
        self.page.ops.append(value)

    def rect(
        self,
        x: float,
        y: float,
        w: float,
        h: float,
        fill: tuple[float, float, float] | None = None,
        stroke: tuple[float, float, float] | None = None,
        width: float = 0.8,
    ) -> None:
        parts = ["q"]
        if fill:
            parts.append(f"{color(fill)} rg")
        if stroke:
            parts.append(f"{color(stroke)} RG")
            parts.append(f"{width:.2f} w")
        parts.append(f"{x:.2f} {y:.2f} {w:.2f} {h:.2f} re")
        parts.append("B" if fill and stroke else "f" if fill else "S")
        parts.append("Q")
        self.op(" ".join(parts))

    def line(
        self,
        x1: float,
        y1: float,
        x2: float,
        y2: float,
        stroke: tuple[float, float, float],
        width: float = 0.8,
    ) -> None:
        self.op(
            f"q {color(stroke)} RG {width:.2f} w {x1:.2f} {y1:.2f} m "
            f"{x2:.2f} {y2:.2f} l S Q"
        )

    def text(
        self,
        x: float,
        y: float,
        text: str,
        font: str = "F1",
        size: float = 10.0,
        fill: tuple[float, float, float] = (0.09, 0.13, 0.20),
    ) -> None:
        self.op(
            f"BT /{font} {size:.2f} Tf {color(fill)} rg "
            f"1 0 0 1 {x:.2f} {y:.2f} Tm ({pdf_escape(text)}) Tj ET"
        )

    def centered_text(
        self,
        y: float,
        text: str,
        font: str = "F1",
        size: float = 10.0,
        fill: tuple[float, float, float] = (0.09, 0.13, 0.20),
    ) -> None:
        x = (PAGE_W - text_width(text, font, size)) / 2
        self.text(x, y, text, font, size, fill)

    def add_link_annotation(self, x: float, y: float, w: float, h: float, target: str) -> None:
        self.page.annots.append(LinkAnnotation(x, y, x + w, y + h, target))

    def draw_runs(
        self,
        x: float,
        y: float,
        line: list[TextItem],
        size: float,
        fill: tuple[float, float, float] = (0.09, 0.13, 0.20),
    ) -> None:
        cx = x
        space_w = text_width(" ", "F1", size)
        for idx, item in enumerate(line):
            if idx:
                cx += space_w
            font = item_font(item)
            draw_size = item_size(item, size)
            w = item_width(item, size)
            if item.style == "code" and item.text:
                text_w = text_width(item.text, font, draw_size)
                box_w = text_w + CODE_PAD_X * 2
                box_h = draw_size + CODE_PAD_Y * 2
                self.rect(
                    cx,
                    y - CODE_PAD_Y,
                    box_w,
                    box_h,
                    fill=(0.93, 0.96, 0.99),
                    stroke=(0.83, 0.89, 0.95),
                    width=0.35,
                )
                self.text(cx + CODE_PAD_X, y, item.text, font, draw_size, fill=(0.06, 0.24, 0.37))
            elif item.link:
                link_color = (0.03, 0.36, 0.53)
                self.text(cx, y, item.text, font, draw_size, fill=link_color)
                self.line(cx, y - 1.8, cx + w, y - 1.8, link_color, 0.45)
                self.add_link_annotation(cx, y - 2.5, w, draw_size + 4.0, item.link)
            else:
                self.text(cx, y, item.text, font, draw_size, fill)
            cx += w

    def paragraph(
        self,
        text: str,
        size: float = 10.2,
        leading: float = 14.0,
        color_value: tuple[float, float, float] = (0.09, 0.13, 0.20),
    ) -> None:
        if self.title_mode:
            clean = strip_inline(text)
            font = "F2" if clean.startswith(("By ", "Members:")) else "F1"
            lines = wrap_plain(clean, CONTENT_W, font, size)
            height = len(lines) * leading + 5
            self.ensure(height)
            for line in lines:
                self.centered_text(self.y, line, font, size, color_value)
                self.y -= leading
            self.y -= 4
            return

        lines = wrap_text(text, CONTENT_W, size)
        height = len(lines) * leading + 5
        self.ensure(height)
        for line in lines:
            self.draw_runs(MARGIN_L, self.y, line, size, color_value)
            self.y -= leading
        self.y -= 4

    def callout(self, text: str) -> None:
        size = 9.8
        leading = 13.2
        pad = 9.0
        lines = wrap_text(text, CONTENT_W - pad * 2, size)
        height = len(lines) * leading + pad * 2
        self.ensure(height + 8)
        bottom = self.y - height + 5
        self.rect(MARGIN_L, bottom, CONTENT_W, height, fill=(1.0, 0.965, 0.91), stroke=(0.93, 0.78, 0.56))
        self.rect(MARGIN_L, bottom, 4.0, height, fill=(0.70, 0.33, 0.04))
        ty = self.y - pad
        for line in lines:
            self.draw_runs(MARGIN_L + pad, ty, line, size)
            ty -= leading
        self.y = bottom - 11

    def caption(self, text: str) -> None:
        clean = text.replace("Caption:", "Caption:", 1)
        lines = wrap_text(clean, CONTENT_W, 8.5)
        height = len(lines) * 11.0 + 7
        self.ensure(height)
        for line in lines:
            self.draw_runs(MARGIN_L, self.y, line, 8.5, fill=(0.32, 0.40, 0.48))
            self.y -= 11
        self.y -= 6

    def heading(self, level: int, text: str) -> None:
        anchor = slugify(text)
        if level == 1:
            if len(self.pages) == 1 and len(self.page.ops) == 1:
                self.title_mode = True
                self.y = PAGE_H - 230
                self.centered_text(self.y, "HoneyOs", "F2", 34, fill=(0.07, 0.21, 0.36))
                self.y -= 42
                self.centered_text(self.y, "User Manual", "F1", 23, fill=(0.03, 0.50, 0.55))
                self.y -= 22
                self.line(PAGE_W / 2 - 130, self.y, PAGE_W / 2 + 130, self.y, (0.80, 0.86, 0.92), 1.2)
                self.y -= 62
            else:
                self.ensure(92)
                h = 66.0
                bottom = self.y - h
                self.rect(MARGIN_L, bottom, CONTENT_W, h, fill=(0.07, 0.21, 0.36))
                self.rect(MARGIN_L, bottom, CONTENT_W, 11.0, fill=(0.03, 0.50, 0.55))
                self.text(MARGIN_L + 18, bottom + 23, text, "F2", 28, fill=(1, 1, 1))
                self.y = bottom - 24
        elif level == 2:
            self.ensure(42)
            self.y -= 6
            if text == "Table of Contents":
                self.toc_mode = True
            self.destinations[anchor] = (len(self.pages) - 1, self.y + 18)
            self.text(MARGIN_L, self.y, text, "F2", 15.5, fill=(0.06, 0.16, 0.26))
            self.line(MARGIN_L, self.y - 7, MARGIN_L + CONTENT_W, self.y - 7, (0.80, 0.86, 0.92), 1.2)
            self.y -= 24
        else:
            self.ensure(28)
            self.y -= 3
            self.destinations[anchor] = (len(self.pages) - 1, self.y + 16)
            self.text(MARGIN_L, self.y, text, "F2", 12.2, fill=(0.06, 0.16, 0.26))
            self.y -= 18

    def list_block(self, items: list[str], ordered: bool) -> None:
        size = 10.0
        leading = 13.5
        marker_w = 22.0
        if self.toc_mode:
            link_color = (0.03, 0.36, 0.53)
            for item in items:
                match = re.fullmatch(r"\[([^\]]+)\]\(#([^)]+)\)", item)
                if match:
                    label = match.group(1)
                    target = match.group(2)
                else:
                    label = strip_inline(item)
                    target = ""
                self.ensure(24)
                x = MARGIN_L + 18
                w = text_width(label, "F2", 10.6)
                self.text(x, self.y, label, "F2", 10.6, fill=link_color)
                self.line(x, self.y - 1.8, x + w, self.y - 1.8, link_color, 0.45)
                if target:
                    self.add_link_annotation(x, self.y - 2.5, w, 14.0, target)
                self.y -= 22
            self.y -= 6
            return

        for idx, item in enumerate(items, 1):
            if self.title_mode:
                lines = wrap_plain(strip_inline(item), CONTENT_W, "F1", size)
                self.ensure(max(1, len(lines)) * leading + 2)
                for line in lines:
                    self.centered_text(self.y, line, "F1", size)
                    self.y -= leading
                continue
            lines = wrap_text(item, CONTENT_W - marker_w, size)
            height = max(1, len(lines)) * leading + 2
            self.ensure(height)
            marker = f"{idx}." if ordered else "-"
            self.text(MARGIN_L + 4, self.y, marker, "F2", size, fill=(0.07, 0.21, 0.36))
            first = True
            for line in lines:
                x = MARGIN_L + marker_w
                self.draw_runs(x, self.y, line, size)
                self.y -= leading
                first = False
            if first:
                self.y -= leading
        self.y -= 5

    def code_block(self, text: str) -> None:
        size = 8.5
        leading = 11.4
        pad = 9.0
        lines = wrap_code(text, CONTENT_W - pad * 2, size)
        height = len(lines) * leading + pad * 2
        self.ensure(height + 10)
        bottom = self.y - height + 3
        self.rect(MARGIN_L, bottom, CONTENT_W, height, fill=(0.04, 0.07, 0.13))
        ty = self.y - pad
        for line in lines:
            self.text(MARGIN_L + pad, ty, line, "F4", size, fill=(0.90, 0.94, 0.97))
            ty -= leading
        self.y = bottom - 12

    def table(self, header: list[str], rows: list[list[str]]) -> None:
        all_rows = [header] + rows
        cols = max(len(row) for row in all_rows)
        col_w = CONTENT_W / cols
        size = 8.2
        leading = 10.8
        pad_x = 5.0
        pad_y = 6.0

        for row_idx, row in enumerate(all_rows):
            wrapped = [
                wrap_plain(row[col] if col < len(row) else "", col_w - pad_x * 2, "F1", size)
                for col in range(cols)
            ]
            row_h = max(len(cell) for cell in wrapped) * leading + pad_y * 2
            self.ensure(row_h + 5)
            bottom = self.y - row_h
            for col_idx, cell_lines in enumerate(wrapped):
                x = MARGIN_L + col_idx * col_w
                fill = (0.07, 0.21, 0.36) if row_idx == 0 else (0.97, 0.98, 0.99) if row_idx % 2 == 0 else (1, 1, 1)
                self.rect(x, bottom, col_w, row_h, fill=fill, stroke=(0.78, 0.84, 0.90), width=0.55)
                ty = self.y - pad_y - size
                font = "F2" if row_idx == 0 else "F1"
                text_color = (1, 1, 1) if row_idx == 0 else (0.09, 0.13, 0.20)
                for cell_line in cell_lines:
                    self.text(x + pad_x, ty, cell_line, font, size, fill=text_color)
                    ty -= leading
            self.y = bottom
        self.y -= 13

    def register_image(self, src: str) -> str:
        path = (MANUAL_MD.parent / src).resolve()
        key = src
        if key not in self.images:
            self.images[key] = read_png(path, IMAGE_CROPS.get(src))
            self.image_names[key] = f"Im{len(self.image_names) + 1}"
        return key

    def image(self, src: str) -> None:
        key = self.register_image(src)
        img = self.images[key]
        draw_w = CONTENT_W
        draw_h = draw_w * img.height / img.width
        max_h = 280.0
        if draw_h > max_h:
            draw_h = max_h
            draw_w = draw_h * img.width / img.height
        self.ensure(draw_h + 22)
        x = MARGIN_L + (CONTENT_W - draw_w) / 2
        y = self.y - draw_h
        self.rect(x - 1.5, y - 1.5, draw_w + 3, draw_h + 3, fill=(1, 1, 1), stroke=(0.72, 0.78, 0.84))
        name = self.image_name(key)
        self.page.images.add(key)
        self.op(f"q {draw_w:.2f} 0 0 {draw_h:.2f} {x:.2f} {y:.2f} cm /{name} Do Q")
        self.y = y - 13

    def image_name(self, key: str) -> str:
        return self.image_names[key]

    def render(self, blocks: list[Block]) -> None:
        for block in blocks:
            if block.kind == "heading":
                data = block.value
                assert isinstance(data, dict)
                self.heading(int(data["level"]), str(data["text"]))
            elif block.kind == "paragraph":
                self.paragraph(str(block.value))
            elif block.kind == "important":
                self.callout(str(block.value))
            elif block.kind == "caption":
                self.caption(str(block.value))
            elif block.kind == "unordered":
                self.list_block(list(block.value), ordered=False)
            elif block.kind == "ordered":
                self.list_block(list(block.value), ordered=True)
            elif block.kind == "code":
                data = block.value
                assert isinstance(data, dict)
                self.code_block(str(data["text"]))
            elif block.kind == "table":
                data = block.value
                assert isinstance(data, dict)
                self.table(list(data["header"]), list(data["rows"]))
            elif block.kind == "image":
                data = block.value
                assert isinstance(data, dict)
                self.image(str(data["src"]))
            elif block.kind == "pagebreak":
                self.new_page()

        self.add_footers()

    def add_footers(self) -> None:
        total = len(self.pages)
        for idx, page in enumerate(self.pages, 1):
            if idx == 1:
                continue
            self.page = page
            self.line(MARGIN_L, 32, MARGIN_L + CONTENT_W, 32, (0.82, 0.86, 0.90), 0.6)
            self.text(MARGIN_L, 20, "HoneyOs User Manual", "F1", 8.2, fill=(0.42, 0.49, 0.57))
            page_label = f"Page {idx} of {total}"
            self.text(
                MARGIN_L + CONTENT_W - text_width(page_label, "F1", 8.2),
                20,
                page_label,
                "F1",
                8.2,
                fill=(0.42, 0.49, 0.57),
            )

    def write(self, path: Path) -> None:
        image_keys = sorted(self.images)
        font_objects = [
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Times-Roman /Encoding /WinAnsiEncoding >>",
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Times-Bold /Encoding /WinAnsiEncoding >>",
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Times-Italic /Encoding /WinAnsiEncoding >>",
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Courier /Encoding /WinAnsiEncoding >>",
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Courier-Bold /Encoding /WinAnsiEncoding >>",
        ]

        objects: list[bytes] = []
        objects.extend(font_objects)

        image_obj_ids: dict[str, int] = {}
        for key in image_keys:
            image_obj_ids[key] = len(objects) + 1
            img = self.images[key]
            header = (
                f"<< /Type /XObject /Subtype /Image /Width {img.width} /Height {img.height} "
                f"/ColorSpace {img.color_space} /BitsPerComponent 8 /Filter /FlateDecode "
                f"/Length {len(img.data)} >>\nstream\n"
            ).encode("ascii")
            objects.append(header + img.data + b"\nendstream")

        first_content_id = len(objects) + 1
        pages_id = first_content_id + len(self.pages) * 2
        page_ids: list[int] = []

        def page_obj_id(page_index: int) -> int:
            return first_content_id + page_index * 2 + 1

        def annotations_for(page: Page) -> str:
            if not page.annots:
                return ""
            annot_dicts = []
            for annot in page.annots:
                target = self.destinations.get(annot.target)
                if target is None:
                    continue
                target_page_index, target_y = target
                target_page_id = page_obj_id(target_page_index)
                annot_dicts.append(
                    "<< /Type /Annot /Subtype /Link "
                    f"/Rect [{annot.x1:.2f} {annot.y1:.2f} {annot.x2:.2f} {annot.y2:.2f}] "
                    "/Border [0 0 0] "
                    f"/A << /S /GoTo /D [{target_page_id} 0 R /XYZ 0 {target_y:.2f} null] >> >>"
                )
            if not annot_dicts:
                return ""
            return f"/Annots [ {' '.join(annot_dicts)} ]"

        for page in self.pages:
            stream = "\n".join(page.ops).encode("latin-1")
            content_id = len(objects) + 1
            objects.append(
                f"<< /Length {len(stream)} >>\nstream\n".encode("ascii")
                + stream
                + b"\nendstream"
            )
            page_id = len(objects) + 1
            page_ids.append(page_id)
            xobjects = ""
            if page.images:
                pairs = []
                for key in sorted(page.images):
                    pairs.append(f"/{self.image_name(key)} {image_obj_ids[key]} 0 R")
                xobjects = f"/XObject << {' '.join(pairs)} >>"
            page_dict = (
                f"<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 {PAGE_W:.2f} {PAGE_H:.2f}] "
                f"/Resources << /Font << /F1 1 0 R /F2 2 0 R /F3 3 0 R /F4 4 0 R /F5 5 0 R >> "
                f"{xobjects} >> /Contents {content_id} 0 R {annotations_for(page)} >>"
            )
            objects.append(page_dict.encode("ascii"))

        kids = " ".join(f"{page_id} 0 R" for page_id in page_ids)
        objects.append(f"<< /Type /Pages /Count {len(page_ids)} /Kids [ {kids} ] >>".encode("ascii"))
        catalog_id = len(objects) + 1
        objects.append(f"<< /Type /Catalog /Pages {pages_id} 0 R >>".encode("ascii"))

        output = bytearray(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n")
        offsets = [0]
        for idx, obj in enumerate(objects, 1):
            offsets.append(len(output))
            output.extend(f"{idx} 0 obj\n".encode("ascii"))
            output.extend(obj)
            output.extend(b"\nendobj\n")

        xref_pos = len(output)
        output.extend(f"xref\n0 {len(objects) + 1}\n".encode("ascii"))
        output.extend(b"0000000000 65535 f \n")
        for offset in offsets[1:]:
            output.extend(f"{offset:010d} 00000 n \n".encode("ascii"))
        output.extend(
            f"trailer\n<< /Size {len(objects) + 1} /Root {catalog_id} 0 R >>\n"
            f"startxref\n{xref_pos}\n%%EOF\n".encode("ascii")
        )
        path.write_bytes(output)


def main() -> None:
    blocks = parse_markdown_blocks(MANUAL_MD.read_text(encoding="utf-8"))
    write_html(blocks)
    pdf = ManualPdf()
    pdf.render(blocks)
    pdf.write(MANUAL_PDF)
    print(f"html: {MANUAL_HTML}")
    print(f"pdf:  {MANUAL_PDF}")


if __name__ == "__main__":
    main()
