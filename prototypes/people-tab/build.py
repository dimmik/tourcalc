#!/usr/bin/env python3
"""Собирает прототип вкладки People из настоящей разметки и стилей приложения.

Правки, которые прототип показывает (по review-2026-09-22.md):
  1.2  раскрытие не подменяет строку — строка остаётся, блок раскрывается под ней
  1.1  режима «compact» больше нет, кнопка убрана
  1.3  баланс не повторяется: он в плашке строки, под ней — из чего он вышел
  1.4  ×вес только у тех, чей вес отличается от общего
  1.5  подсказка про Paid/Charged живёт внутри раскрытого блока
  2.1  💸 из строки убрана (остаётся в раскрытом блоке и плавающей кнопкой)
  2.2  «10 people» — текст, а не чип
  0.1  плашки считаются с порогом: получатель больше не «settled»
"""
import html
import json
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent

CHROME = open(HERE / 'chrome-head.html', encoding='utf-8').read()
PEOPLE = json.loads(open(HERE / 'people.json', encoding='utf-8').read())

COMMON_WEIGHT = 100  # то, что в mini.rs считает common_weight

# Баланс после правки 0.1: сторона выбирается с тем же порогом, что и показ.
FIXED = {
    'Андрей':  ('gets 5 057',  'green'),
    'Дима А.': ('gets 904',    'green'),
    'Дима Т.': ('owes 3 401',  'red'),
    'Женя К.': ('owes 28 387', 'red'),
    'Паша':    ('gets 15 628', 'green'),
    'Саша О.': ('settled',     ''),
    'Хомяк':   ('gets 10 199', 'green'),
    'Валечка': ('settled',     ''),
    'Игоряша': ('settled',     ''),
    'Олежка':  ('settled',     ''),
}
WEIGHT = {'Андрей': 100, 'Дима А.': 100, 'Дима Т.': 100, 'Женя К.': 100, 'Паша': 100,
          'Саша О.': 100, 'Хомяк': 100, 'Валечка': 100, 'Игоряша': 30, 'Олежка': 60}
FAMILY = {'Саша О.': (3, 290)}
PAID_BY = {'Валечка': 'Саша О.', 'Игоряша': 'Саша О.', 'Олежка': 'Саша О.'}
ORDER = ['Андрей', 'Дима А.', 'Дима Т.', 'Женя К.', 'Паша', 'Саша О.', 'Хомяк']
KIDS = {'Саша О.': ['Валечка', 'Игоряша', 'Олежка']}

by_name = {p['name']: p for p in PEOPLE}


def figures(name):
    """Paid и Charged из настоящего приложения."""
    st = by_name[name]['stats']
    return st[0]['value'], st[1]['value']


def chip(name):
    text, kind = FIXED[name]
    cls = {'green': 'tcn-chip tcn-chip-green', 'red': 'tcn-chip tcn-chip-red'}.get(kind, 'tcn-chip')
    return f'<span class="{cls}">{text}</span>'


def row(name, kid=False):
    p = by_name[name]
    w = WEIGHT[name]
    # 1.4: вес показывается, только если он не общий
    if name in FAMILY:
        n, fw = FAMILY[name]
        meta = (f'<span class="tcn-chip tcn-chip-kids" title="Pays for {n}, {fw} of weight between them">👥{n} ×{fw}</span>')
    elif w != COMMON_WEIGHT:
        meta = f'<span class="tcn-person-meta" title="Weight {w}">×{w}</span>'
    else:
        meta = ''

    paid, charged = figures(name)
    bal, _ = FIXED[name]
    extra = []
    if name in PAID_BY:
        extra.append(f'paid for by <b>{PAID_BY[name]}</b>')
    if name in FAMILY:
        n, fw = FAMILY[name]
        extra.append(f'pays for <b>{n}</b>')
        extra.append(f'family weight <b>{fw}</b>')
    extra.append(f'weight <b>{w}</b>')
    extra_html = ' · '.join(extra)

    return f'''
<div class="tcn-person{' is-child' if kid else ''} is-compact" data-who="{html.escape(name)}">
  <div class="tcn-person-row">
    <button class="tcn-person-open" type="button" aria-expanded="false">
      <span style="{p['colour'].replace('style=','')}" class="tcn-avatar tcn-avatar-sm">{p['initials']}</span>
      <span class="tcn-person-id">
        <span class="tcn-person-name">{html.escape(name)}</span>
        {meta}
      </span>
      {chip(name)}
      <span class="tcn-person-caret">
        <svg class="tcn-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
             stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6"></polyline></svg>
      </span>
    </button>
  </div>
  <div class="tcp-open">
    <div class="tcp-facts">{extra_html}</div>
    <div class="tcn-person-stats tcp-two">
      <div class="tcn-stat">
        <span class="tcn-stat-label">Paid</span>
        <button type="button" class="tcn-statbtn is-paid">{paid}</button>
      </div>
      <div class="tcn-stat">
        <span class="tcn-stat-label">Charged</span>
        <button type="button" class="tcn-statbtn">{charged}</button>
      </div>
    </div>
    <div class="tcp-sum">
      Difference — <b>{bal}</b>
      <button type="button" class="tcp-link">itemised ›</button>
    </div>
    <div class="tcp-hint">
      <b>Paid</b> — what they put in · <b>Charged</b> — what their part of the spending cost.
      Tap a figure for the itemised list.
    </div>
    <div class="tcn-person-actions">
      <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-primary">💸 Spend</button>
      <button type="button" class="tcn-btn tcn-btn-sm">Edit</button>
      <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-danger">Delete</button>
    </div>
  </div>
</div>'''


blocks = []
for name in ORDER:
    kids = KIDS.get(name)
    if kids:
        inner = ''.join(row(k, kid=True) for k in kids)
        blocks.append(f'''
<div class="tcn-family has-kids">
  {row(name)}
  <button class="tcn-familybar" type="button" aria-expanded="false">
    <span><svg class="tcn-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
          stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6"></polyline></svg></span>
    <span class="tcn-familybar-names">{', '.join(kids)}</span>
    <span class="tcn-familybar-note">paid for by {name}</span>
  </button>
  <div class="tcn-family-kids tcp-kids">{inner}</div>
</div>''')
    else:
        blocks.append(f'<div class="tcn-family">{row(name)}</div>')

TOOLBAR = '''
<div class="tcn-section">
  <div class="tcn-toolbar">
    <button type="button" class="tcn-btn tcn-btn-primary">+ Add person</button>
    <div class="tcn-search"><span class="tcn-search-icon">🔎</span>
      <input type="text" placeholder="Find a person"></div>
    <button type="button" class="tcn-btn tcn-btn-sm">Expand all</button>
    <span class="tcp-count">10 people · total weight <span class="tcp-why">890</span></span>
  </div>
</div>'''

STYLE = '''
<style>
  /* Прототип: только то, чего нет в стилях приложения. */
  .tcp-open { display: none; padding: 2px 12px 12px 12px; }
  .tcn-person.is-open > .tcp-open { display: block; }
  .tcn-person.is-open { box-shadow: 0 0 0 2px var(--tcn-primary, #4338ca) inset; }
  .tcp-facts { font-size: 12px; color: var(--tcn-faint, #6b7280); margin: 2px 0 10px 2px; }
  .tcp-two { grid-template-columns: 1fr 1fr !important; }
  .tcp-sum { margin-top: 10px; font-size: 13px; display: flex; align-items: center; gap: 8px;
             flex-wrap: wrap; }
  .tcp-link { border: 0; background: none; color: var(--tcn-primary, #4338ca); font: inherit;
              cursor: pointer; padding: 0; text-decoration: underline; }
  .tcp-hint { font-size: 11px; color: var(--tcn-faint, #6b7280); margin-top: 8px; line-height: 1.5; }
  .tcp-count { font-size: 12px; color: var(--tcn-faint, #6b7280); margin-left: auto; }
  .tcp-why { border-bottom: 1px dotted currentColor; cursor: help; }
  .tcp-kids { display: none; }
  /* каретка поворачивается вместо того, чтобы меняться на другую иконку */
  .tcn-person-caret svg { transition: transform .15s ease; }
  .tcn-person.is-open .tcn-person-caret svg { transform: rotate(90deg); }
  /* место под плавающей кнопкой — чтобы она не садилась на последнюю строку */
  .tcn-people-list { padding-bottom: 72px; }
  .tcn-family.is-open > .tcp-kids { display: block; }
</style>'''

SCRIPT = '''
<script>
  document.addEventListener('click', (e) => {
    const open = e.target.closest('.tcn-person-open');
    if (open) {
      const p = open.closest('.tcn-person');
      p.classList.toggle('is-open');
      open.setAttribute('aria-expanded', p.classList.contains('is-open'));
      return;
    }
    const bar = e.target.closest('.tcn-familybar');
    if (bar) {
      const f = bar.closest('.tcn-family');
      f.classList.toggle('is-open');
      bar.setAttribute('aria-expanded', f.classList.contains('is-open'));
    }
  });
</script>'''

page = f'''<!doctype html>
<html lang="ru"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Tourcalc · People, прототип</title>
<link rel="stylesheet" href="newui.css">
<link rel="stylesheet" href="app-inline.css">
{STYLE}
</head>
{CHROME}
{TOOLBAR}
<div class="tcn-section"><div class="tcn-list tcn-people-list">
{''.join(blocks)}
</div></div>
<button type="button" class="tcn-btn tcn-btn-primary tcn-fab">+ Spend</button>
{SCRIPT}
</body></html>'''

open(HERE / 'proto.html', 'w', encoding='utf-8').write(page)
print('готово:', len(page), 'символов')
