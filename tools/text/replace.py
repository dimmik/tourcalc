"""Exact text replacements in the repository's files, keeping each file's line endings.

The working copy on Windows is CRLF and the repository is LF (see CLAUDE.md), so a plain
read-replace-write either misses every multi-line match or rewrites a file's endings. This
reads with the endings normalised, replaces, and writes back the endings the file had.

    import sys; sys.path.insert(0, 'tools/text')
    from replace import edit
    edit('rust/crates/tc-web/src/ui.rs', [
        ('old text', 'new text'),          # must occur exactly once
        ('another', 'replacement', 3),     # ... or exactly this many times
    ])

Paths are relative to the repository root, wherever the script is run from. A replacement
whose old text is not found the expected number of times stops everything before anything is
written - a half-applied edit is worse than none.
"""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def load(path):
    # newline='' - read_text would turn CRLF into LF before we could see which it was.
    with open(ROOT / path, encoding='utf-8', newline='') as f:
        raw = f.read()
    newline = '\r\n' if '\r\n' in raw else '\n'
    return raw.replace('\r\n', '\n'), newline


def save(path, text, newline):
    (ROOT / path).write_text(text.replace('\n', newline), encoding='utf-8', newline='')


def edit(path, pairs):
    text, newline = load(path)
    for pair in pairs:
        old, new = pair[0], pair[1]
        times = pair[2] if len(pair) > 2 else 1
        found = text.count(old)
        if found != times:
            raise SystemExit(f'{path}: expected {times} of {old[:80]!r}, found {found}')
        text = text.replace(old, new)
    save(path, text, newline)
