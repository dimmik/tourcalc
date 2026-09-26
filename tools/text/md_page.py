#!/usr/bin/env python3
"""A review or plan written in Markdown, as one HTML page to publish as an artifact.

Standard library only - no `markdown` package on a fresh machine. Covers what the repository's
own .md files use: headings, paragraphs, lists (one level, with wrapped lines), tables, rules,
**bold**, *italic*, ~~struck~~, `code` and [links](url).

    tools/text/md_page.py currencies-review-20260926.md out.html --title "Ревью валют"

Reviews get one extra: a severity word ("высокая", "средняя", "низкая", "косметика", "снято", with
"(…)" after it allowed) at the end of a `## ` heading after "·", or alone in a table cell,
becomes a coloured pill - so what needs attention reads at a glance.
"""
import argparse
import html
import re
from pathlib import Path

SEVERITY = {'высокая': 'high', 'средняя': 'mid', 'низкая': 'low', 'косметика': 'cos', 'снято': 'cos'}


def inline(text):
    out = html.escape(text, quote=False)
    out = re.sub(r'`([^`]+)`', r'<code>\1</code>', out)
    out = re.sub(r'~~([^~]+)~~', r'<s>\1</s>', out)
    out = re.sub(r'\*\*([^*]+)\*\*', r'<b>\1</b>', out)
    out = re.sub(r'(?<![\w*])\*([^*\s][^*]*)\*(?![\w*])', r'<i>\1</i>', out)
    # A path inside the repository means nothing on a published page: its text stays.
    out = re.sub(r'\[([^\]]+)\]\(([^)]+)\)',
                 lambda m: f'<a href="{m.group(2)}">{m.group(1)}</a>'
                 if m.group(2).startswith('http') else m.group(1), out)
    return out


def pill(text):
    """A severity pill, if `text` is one."""
    plain = re.sub(r'<[^>]+>', '', text).strip()
    word = plain.split(' ')[0].split(',')[0]
    kind = SEVERITY.get(word)
    return f'<span class="sev sev-{kind}">{text}</span>' if kind else None


def convert(md):
    lines = md.replace('\r\n', '\n').split('\n')
    out, para, items, i = [], [], None, 0

    def flush():
        nonlocal para, items
        if para:
            out.append(f'<p>{inline(" ".join(para))}</p>')
            para = []
        if items is not None:
            out.append('<ul>' + ''.join(f'<li>{inline(" ".join(x))}</li>' for x in items) + '</ul>')
            items = None

    while i < len(lines):
        line = lines[i]
        if line.startswith('|'):
            flush()
            rows = []
            while i < len(lines) and lines[i].startswith('|'):
                rows.append([c.strip() for c in lines[i].strip().strip('|').split('|')])
                i += 1
            head, body = rows[0], [r for r in rows[1:] if not all(set(c) <= set('-: ') for c in r)]
            t = '<div class="table"><table><thead><tr>' + ''.join(f'<th>{inline(c)}</th>' for c in head) + '</tr></thead><tbody>'
            for r in body:
                cells = []
                for c in r:
                    rendered = inline(c)
                    cells.append(f'<td>{pill(rendered) or rendered}</td>')
                t += '<tr>' + ''.join(cells) + '</tr>'
            out.append(t + '</tbody></table></div>')
            continue
        m = re.match(r'(#{1,3}) (.*)', line)
        if m:
            flush()
            level, text = len(m.group(1)), m.group(2)
            badge = ''
            if level == 2 and ' · ' in text:
                head, tail = text.rsplit(' · ', 1)
                p = pill(inline(tail))
                if p:
                    text, badge = head, ' ' + p
            anchor = re.match(r'(\d+)\.', text)
            ident = f' id="f{anchor.group(1)}"' if anchor else ''
            out.append(f'<h{level}{ident}>{inline(text)}{badge}</h{level}>')
        elif line.strip() == '---':
            flush()
            out.append('<hr>')
        elif line.startswith('- '):
            if para:
                flush()
            if items is None:
                items = []
            items.append([line[2:]])
        elif line.startswith('  ') and items is not None and line.strip():
            items[-1].append(line.strip())
        elif not line.strip():
            flush()
        else:
            if items is not None:
                flush()
            para.append(line.strip())
        i += 1
    flush()
    return '\n'.join(out)


STYLE = '''
:root {
  --ground: #f5f6fa; --paper: #ffffff; --ink: #1c2030; --soft: #5a6173; --line: #dfe2ec;
  --accent: #4338ca; --code: #eef0f7;
  --high: #b4233f; --high-bg: #fde8ec; --mid: #9a5a06; --mid-bg: #fdf1dc;
  --low: #1f6b5c; --low-bg: #e1f3ee; --cos: #555c6e; --cos-bg: #eceef3;
}
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    color-scheme: dark;
    --ground: #13151d; --paper: #1b1e29; --ink: #e6e8f0; --soft: #9da3b6; --line: #2d3142;
    --accent: #a5a2ff; --code: #262a38;
    --high: #ff8fa3; --high-bg: #3a1a22; --mid: #f3bf6b; --mid-bg: #33280f;
    --low: #7fd8c3; --low-bg: #14302a; --cos: #b3b8c7; --cos-bg: #282c38;
  }
}
:root[data-theme="dark"] {
  color-scheme: dark;
  --ground: #13151d; --paper: #1b1e29; --ink: #e6e8f0; --soft: #9da3b6; --line: #2d3142;
  --accent: #a5a2ff; --code: #262a38;
  --high: #ff8fa3; --high-bg: #3a1a22; --mid: #f3bf6b; --mid-bg: #33280f;
  --low: #7fd8c3; --low-bg: #14302a; --cos: #b3b8c7; --cos-bg: #282c38;
}
body { background: var(--ground); color: var(--ink); margin: 0;
  font: 16px/1.6 "IBM Plex Sans", "Segoe UI", system-ui, sans-serif; }
.page { max-width: 76ch; margin: 0 auto; padding-inline: 16px; padding-block: 32px 64px; }
h1 { font-size: 1.75rem; line-height: 1.25; margin: 0 0 .6em; text-wrap: balance; }
h2 { font-size: 1.2rem; line-height: 1.35; margin: 2.2em 0 .6em; text-wrap: balance;
  display: flex; flex-wrap: wrap; align-items: baseline; gap: 6px 10px; }
h3 { font-size: 1rem; margin: 1.6em 0 .4em; }
p, ul { margin: 0 0 1em; }
ul { padding-left: 1.3em; }
li { margin: .3em 0; }
p > i:only-child { color: var(--soft); font-style: normal; font-size: .92rem; display: block; }
a { color: var(--accent); }
code { font: .86em/1.4 "IBM Plex Mono", ui-monospace, Consolas, monospace; background: var(--code);
  padding: .1em .35em; border-radius: 4px; overflow-wrap: anywhere; }
hr { border: 0; border-top: 1px solid var(--line); margin: 2.4em 0; }
.table { overflow-x: auto; margin: 0 0 1.2em; background: var(--paper);
  border: 1px solid var(--line); border-radius: 10px; }
table { border-collapse: collapse; width: 100%; font-size: .92rem; }
th, td { text-align: left; vertical-align: top; padding: 8px 12px; border-bottom: 1px solid var(--line); }
tr:last-child td { border-bottom: 0; }
th { color: var(--soft); font-weight: 600; font-size: .78rem; letter-spacing: .04em; text-transform: uppercase; }
td:first-child { font-variant-numeric: tabular-nums; color: var(--soft); }
.sev { display: inline-block; font-size: .78rem; font-weight: 600; line-height: 1.3; letter-spacing: .02em;
  padding: 2px 9px; border-radius: 999px; white-space: nowrap; }
.sev-high { color: var(--high); background: var(--high-bg); }
.sev-mid { color: var(--mid); background: var(--mid-bg); }
.sev-low { color: var(--low); background: var(--low-bg); }
.sev-cos { color: var(--cos); background: var(--cos-bg); }
'''


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('src')
    ap.add_argument('out')
    ap.add_argument('--title', required=True)
    a = ap.parse_args()
    body = convert(Path(a.src).read_text(encoding='utf-8'))
    page = (f'<title>{html.escape(a.title)}</title>\n'
            '<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=IBM+Plex+Mono'
            '&family=IBM+Plex+Sans:ital,wght@0,400;0,600;1,400&display=swap">\n'
            f'<style>{STYLE}</style>\n<main class="page">\n{body}\n</main>\n')
    Path(a.out).write_text(page, encoding='utf-8')
    print(a.out, len(page), 'bytes')


if __name__ == '__main__':
    main()
