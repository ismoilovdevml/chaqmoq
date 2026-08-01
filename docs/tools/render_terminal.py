#!/usr/bin/env python3
"""Render captured terminal output (ANSI escapes included) as an SVG.

The screenshots in the README are produced by this script from real command
output, so they cannot drift away from what the runtime actually prints. See
``docs/tools/capture.sh`` for the commands that feed it.

Usage:
    render_terminal.py --title "chaqmoq run app.ts" --out shot.svg < output.txt
"""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass, replace
from html import escape

# A dark palette with enough contrast for both GitHub themes.
BACKGROUND = "#15161e"
CHROME = "#1f2335"
FOREGROUND = "#c0caf5"
TITLE_COLOR = "#7f849c"

PALETTE = {
    30: "#414868", 31: "#f7768e", 32: "#9ece6a", 33: "#e0af68",
    34: "#7aa2f7", 35: "#bb9af7", 36: "#7dcfff", 37: "#c0caf5",
    90: "#6c7394", 91: "#ff9eb1", 92: "#b9f27c", 93: "#f0c987",
    94: "#9db8ff", 95: "#d3b9ff", 96: "#a5e2ff", 97: "#ffffff",
}

# Terminal metrics, in pixels, for the font size below.
FONT_SIZE = 14.0
CHAR_WIDTH = 8.4
LINE_HEIGHT = 20.0
PADDING_X = 20.0
PADDING_TOP = 46.0
PADDING_BOTTOM = 18.0

SGR = re.compile(r"\x1b\[([0-9;]*)m")

# Every other escape sequence is discarded: cursor moves, line clears and
# bracketed-paste markers carry no meaning in a static screenshot. The final
# byte class deliberately omits a lowercase `m` so that this does not also eat
# the colour codes SGR is about to parse.
OTHER_ESCAPES = re.compile(r"\x1b\[[0-9;?]*[A-Za-ln-z]|\x1b\][^\x07]*\x07|\x1b[()][B0]|\r")


@dataclass(frozen=True)
class Style:
    color: str = FOREGROUND
    bold: bool = False
    dim: bool = False


@dataclass
class Span:
    text: str
    style: Style


def column_of(chars: list[str]) -> int:
    """Current column, counting back to the last newline."""
    column = 0
    for char in reversed(chars):
        if char == "\n":
            break
        column += 1
    return column


def sanitise(text: str) -> str:
    """Reduces a PTY capture to printable text, newlines and colour codes.

    A capture carries more than colour: cursor moves, bracketed-paste markers
    and backspaces all end up in the stream, and a single stray control byte is
    enough to make the resulting SVG malformed XML. The escape character itself
    survives, because by this point the only sequences left are the SGR ones
    that `parse_ansi` is about to read.
    """
    text = OTHER_ESCAPES.sub("", text)

    out: list[str] = []
    for char in text:
        if char == "\b":
            # A backspace erases the character the terminal just drew.
            if out and out[-1] != "\n":
                out.pop()
        elif char == "\t":
            # Tabs advance to the next 8-column stop.
            out.extend(" " * (8 - (column_of(out) % 8)))
        elif char in ("\n", "\x1b") or char >= " ":
            out.append(char)
        # Any other C0 control byte is dropped.
    return "".join(out)


def printable(text: str) -> str:
    """Last line of defence: no control byte may reach the SVG."""
    return "".join(char for char in text if char >= " ")


def parse_ansi(text: str) -> list[list[Span]]:
    """Splits ANSI-coloured text into styled spans, one list per line."""
    text = sanitise(text)

    lines: list[list[Span]] = []
    current: list[Span] = []
    style = Style()
    position = 0

    for match in SGR.finditer(text):
        chunk = text[position : match.start()]
        position = match.end()

        for index, piece in enumerate(chunk.split("\n")):
            if index > 0:
                lines.append(current)
                current = []
            piece = printable(piece)
            if piece:
                current.append(Span(piece, style))

        style = apply_codes(style, match.group(1))

    for index, piece in enumerate(text[position:].split("\n")):
        if index > 0:
            lines.append(current)
            current = []
        piece = printable(piece)
        if piece:
            current.append(Span(piece, style))
    lines.append(current)

    # Trailing blank lines are just noise in a screenshot.
    while lines and not lines[-1]:
        lines.pop()
    return lines


def apply_codes(style: Style, raw: str) -> Style:
    codes = [int(part) for part in raw.split(";") if part != ""] or [0]
    for code in codes:
        if code == 0:
            style = Style()
        elif code == 1:
            style = replace(style, bold=True)
        elif code == 2:
            style = replace(style, dim=True)
        elif code == 22:
            style = replace(style, bold=False, dim=False)
        elif code in PALETTE:
            style = replace(style, color=PALETTE[code])
    return style


def render(lines: list[list[Span]], title: str) -> str:
    columns = max((sum(len(span.text) for span in line) for line in lines), default=0)
    columns = max(columns, len(title) + 8)

    width = PADDING_X * 2 + columns * CHAR_WIDTH
    height = PADDING_TOP + len(lines) * LINE_HEIGHT + PADDING_BOTTOM

    out: list[str] = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0f}" '
        f'height="{height:.0f}" viewBox="0 0 {width:.0f} {height:.0f}" '
        f'font-family="ui-monospace, SFMono-Regular, Menlo, Consolas, monospace" '
        f'font-size="{FONT_SIZE}">',
        f'<rect width="{width:.0f}" height="{height:.0f}" rx="10" fill="{BACKGROUND}"/>',
        f'<path d="M0 10a10 10 0 0 1 10-10h{width - 20:.0f}a10 10 0 0 1 10 10v22H0z" '
        f'fill="{CHROME}"/>',
    ]

    # Window buttons.
    for index, colour in enumerate(("#f7768e", "#e0af68", "#9ece6a")):
        out.append(f'<circle cx="{18 + index * 18}" cy="16" r="5.5" fill="{colour}"/>')

    out.append(
        f'<text x="{width / 2:.0f}" y="21" fill="{TITLE_COLOR}" font-size="12" '
        f'text-anchor="middle">{escape(title)}</text>'
    )

    for row, line in enumerate(lines):
        y = PADDING_TOP + row * LINE_HEIGHT
        if not line:
            continue
        out.append(f'<text y="{y:.1f}" xml:space="preserve">')
        column = 0
        for span in line:
            # Position every span explicitly: proportional fallback fonts would
            # otherwise let the columns drift.
            x = PADDING_X + column * CHAR_WIDTH
            attrs = f'x="{x:.1f}" fill="{span.style.color}"'
            if span.style.bold:
                attrs += ' font-weight="bold"'
            if span.style.dim:
                attrs += ' opacity="0.62"'
            out.append(f"<tspan {attrs}>{escape(span.text)}</tspan>")
            column += len(span.text)
        out.append("</text>")

    out.append("</svg>")
    return "\n".join(out)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--title", default="chaqmoq", help="text shown in the title bar")
    parser.add_argument("--out", required=True, help="path of the SVG to write")
    args = parser.parse_args()

    raw = sys.stdin.buffer.read().decode("utf-8", errors="replace")
    svg = render(parse_ansi(raw), args.title)

    with open(args.out, "w", encoding="utf-8") as handle:
        handle.write(svg)
    print(f"wrote {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
