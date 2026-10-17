#!/usr/bin/env python3
"""Время и контекст задачи для строки ai-worklog.md — из лога сессии Claude Code.

    python3 tools/worklog/span.py <лог.jsonl>                # последние 15 просьб
    python3 tools/worklog/span.py <лог.jsonl> "текст просьбы"  # одна задача

Лог — ~/.claude/projects/<папка репозитория>/<сессия>.jsonl. Список показывает просьбы
пользователя: местное время и занятость контекста к первому ответу. С текстом берётся
последняя просьба, где он встречается: начало, контекст в начале, последний ответ перед
следующей просьбой и пуши (`git push`) между ними. Контекст = input_tokens +
cache_read_input_tokens + cache_creation_input_tokens у ответа, в процентах окна.
"""
import argparse
import json
from datetime import datetime, timedelta, timezone

SKIP = ('<task-notification', '<local-command', '<command-name>', '<command-message>')


def entries(path):
    seen = set()
    out = []
    with open(path, encoding='utf-8') as f:
        for line in f:
            try:
                j = json.loads(line)
            except ValueError:
                continue
            key = j.get('uuid') or line
            if key in seen:
                continue
            seen.add(key)
            if j.get('timestamp'):
                out.append(j)
    return sorted(out, key=lambda j: j['timestamp'])


def request_text(j):
    """Текст просьбы пользователя, или None, если запись — не она."""
    if j.get('type') != 'user' or j.get('isMeta') or j.get('isCompactSummary'):
        return None
    content = j.get('message', {}).get('content')
    if isinstance(content, str):
        text = content
    elif isinstance(content, list):
        if any(b.get('type') == 'tool_result' for b in content):
            return None
        text = ' '.join(b.get('text', '') for b in content if b.get('type') == 'text')
        if any(b.get('type') == 'image' for b in content):
            text = '[снимок] ' + text
    else:
        return None
    text = text.strip()
    if not text or text.startswith(SKIP):
        return None
    return text


def used(j):
    u = j.get('message', {}).get('usage') if j.get('type') == 'assistant' else None
    if not u:
        return None
    return (u.get('input_tokens', 0) + u.get('cache_read_input_tokens', 0)
            + u.get('cache_creation_input_tokens', 0))


def pushes(j):
    if j.get('type') != 'assistant':
        return False
    for b in j.get('message', {}).get('content') or []:
        if b.get('type') == 'tool_use' and 'git push' in str(b.get('input', {}).get('command', '')):
            return True
    return False


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('log')
    ap.add_argument('text', nargs='?')
    ap.add_argument('--last', type=int, default=15)
    ap.add_argument('--utc-offset', type=float, default=2, help='часы, по умолчанию +2 (CEST)')
    ap.add_argument('--window', type=int, default=1_000_000)
    a = ap.parse_args()
    tz = timezone(timedelta(hours=a.utc_offset))

    def at(j):
        t = datetime.fromisoformat(j['timestamp'].replace('Z', '+00:00')).astimezone(tz)
        return t.strftime('%d.%m %H:%M')

    def pct(n):
        return '—' if n is None else f'{100 * n / a.window:.0f} %'

    log = entries(a.log)
    asks = [(i, request_text(j)) for i, j in enumerate(log)]
    asks = [(i, t) for i, t in asks if t]

    def span(n):
        """Ответы на просьбу n: до следующей просьбы."""
        start = asks[n][0]
        stop = asks[n + 1][0] if n + 1 < len(asks) else len(log)
        return log[start + 1:stop]

    def context_at(n):
        return next((used(j) for j in span(n) if used(j) is not None), None)

    if not a.text:
        for n in range(max(0, len(asks) - a.last), len(asks)):
            i, t = asks[n]
            print(f'{at(log[i])}  {pct(context_at(n)):>5}  {t[:70]}')
        return
    match = [n for n, (_, t) in enumerate(asks) if a.text.lower() in t.lower()]
    if not match:
        raise SystemExit(f'нет просьбы с «{a.text}»')
    n = match[-1]
    replies = [j for j in span(n) if j.get('type') == 'assistant']
    print(f'просьба:  {asks[n][1][:100]}')
    print(f'начало:   {at(log[asks[n][0]])}')
    print(f'контекст: {pct(context_at(n))}')
    print(f'конец:    {at(replies[-1]) if replies else "—"} (последний ответ)')
    for j in (j for j in replies if pushes(j)):
        print(f'пуш:      {at(j)}')


if __name__ == '__main__':
    main()
