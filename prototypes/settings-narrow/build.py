#!/usr/bin/env python3
"""Прототип страницы настроек на узком экране: как сейчас и три варианта.

Собирает одну страницу из настоящего newui.css приложения и настоящих русских текстов
(из rust/crates/tc-web/src/i18n/ru.rs на 24.09.2026). Каждый вариант — телефонный экран
шириной 360 px. Результат — proto.html рядом, публикуется как артефакт.

  current  — нынешняя раскладка: текст слева, контрол справа в одну строку
  a        — название и контрол в строку, описание под ними на всю ширину
  b        — всё в столбик: название, описание, контрол на всю ширину
  c        — как a, плюс описание в одну фразу, остальное под «подробнее»
"""
from pathlib import Path
import html

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
NEWUI = (ROOT / 'TCBlazor/Client/wwwroot/css/newui.css').read_text(encoding='utf-8')

# Строки настроек: название, полное описание, короткое (для варианта c), контрол.
ROWS = [
    ('Язык · Language',
     'На каком языке приложение на этом устройстве. Названия туров, имена и категории '
     'остаются такими, как их ввели; уведомления и журнал изменений — по-английски.',
     'Язык приложения на этом устройстве.',
     'select:Русский'),
    ('Не считать долги меньше',
     'Всё, что меньше, считается погашенным: копеечные остатки от округления перестают '
     'показываться долгами. В туре с несколькими валютами порог пересчитывается, чтобы '
     '«мелочью» были одни и те же деньги, в какой бы валюте вы ни смотрели.',
     'Меньшие остатки считаются погашенными.',
     'number:49'),
    ('Цвет акцента',
     'Шапка, кнопки и выделения. Всё остальное остаётся нейтрально-серым, так что меняется '
     'только акцент, но не текст. Последний образец открывает палитру, если ни один из семи '
     'не подошёл; слишком светлый цвет чуть затемняется, чтобы белый текст на нём оставался '
     'читаемым.',
     'Шапка, кнопки и выделения.',
     'accents'),
]
DEVICE = [
    ('Проверять чужие изменения',
     'Пока тур открыт и на экране, приложение спрашивает сервер, не менял ли его кто-то ещё, '
     'и если менял — подтягивает изменения. Оно всегда спрашивает, когда вы возвращаетесь во '
     'вкладку; здесь — как часто спрашивать в промежутке. Каждый вопрос — несколько байт, и '
     'пока вкладка скрыта или телефон заблокирован, ничего не спрашивается. Действует со '
     'следующего открытого тура.',
     'Как часто открытый тур спрашивает сервер о чужих правках.',
     'select:каждые 10 с'),
    ('Установить на это устройство',
     'Tourcalc как отдельное приложение: своё окно без адресной строки, своя иконка и всё '
     'уже скачанное, так что открывается и без сети.',
     'Отдельное приложение со своей иконкой; открывается и без сети.',
     'button:Установить'),
]

ACCENTS = ['#4f46e5,#6d28d9', '#2563eb,#226c91', '#0f766e,#226881', '#15803d,#1b6a5a',
           '#7e22ce,#8d358d', '#b91c1c,#922a51', '#475569,#425367']


def control(kind, uid):
    what, _, value = kind.partition(':')
    if what == 'select':
        return (f'<select class="tcn-input tcw-ctl" id="{uid}" aria-label="{html.escape(value)}">'
                f'<option>{html.escape(value)}</option></select>')
    if what == 'number':
        return (f'<input class="tcn-input tcn-setnum tcw-ctl" id="{uid}" type="number" '
                f'value="{value}" aria-label="Порог">')
    if what == 'button':
        return f'<button type="button" class="tcn-btn tcn-btn-primary tcw-ctl">{value}</button>'
    return ''


def accents():
    dots = ''.join(
        f'<button type="button" class="tcn-accent-dot{" is-on" if i == 0 else ""}" '
        f'style="background:linear-gradient(120deg,{c})" aria-label="Цвет {i + 1}"></button>'
        for i, c in enumerate(ACCENTS))
    return (f'<div class="tcn-accents" style="padding:0 14px 14px">{dots}'
            f'<span class="tcn-accent-dot" style="background:conic-gradient(red,yellow,lime,'
            f'aqua,blue,magenta,red)" aria-label="Любой другой"></span></div>')


def row(variant, name, full, short, kind, uid):
    ctl = '' if kind == 'accents' else control(kind, uid)
    if variant == 'c':
        desc = (f'<div class="tcn-setdesc">{html.escape(short)} '
                f'<details class="tcw-more"><summary>подробнее</summary>'
                f'<div>{html.escape(full)}</div></details></div>')
    else:
        desc = f'<div class="tcn-setdesc">{html.escape(full)}</div>'
    out = (f'<div class="tcn-setrow"><div class="tcn-settext">'
           f'<div class="tcn-setname">{html.escape(name)}</div>{desc}</div>{ctl}</div>')
    if kind == 'accents':
        out += accents()
    return out


def screen(variant, title, note):
    body = ''.join(row(variant, *r, f'{variant}-{i}') for i, r in enumerate(ROWS))
    device = ''.join(row(variant, *r, f'{variant}-d{i}') for i, r in enumerate(DEVICE))
    return f'''
<figure class="shot">
  <figcaption><b>{title}</b><span>{note}</span></figcaption>
  <div class="phone v-{variant}">
    <div class="bar">🧭<span class="dot"></span><span class="back">‹ Август на Дунае</span><span class="more">⋯</span></div>
    <div class="screen">
      <div class="tcn-section-title">Настройки</div>
      <div class="tcn-card">{body}</div>
      <div class="tcn-section-title">Это устройство</div>
      <div class="tcn-card">{device}</div>
    </div>
  </div>
</figure>'''


PAGE_CSS = '''
:root {
  --page: #eef0f6; --ink: #1b1f2e; --soft: #5b6275; --frame: #1b1f2e; --line: #d7dbe7;
}
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) { --page: #14161f; --ink: #e8eaf2; --soft: #9aa1b5;
    --frame: #05060a; --line: #2a2e3d; color-scheme: dark; }
}
:root[data-theme="dark"] { --page: #14161f; --ink: #e8eaf2; --soft: #9aa1b5;
  --frame: #05060a; --line: #2a2e3d; color-scheme: dark; }
body { background: var(--page); color: var(--ink);
  font: 15px/1.5 "Helvetica Neue", Helvetica, Arial, sans-serif; }
.wrap { max-width: 1560px; margin: 0 auto; padding: 24px 16px 40px; }
h1 { font-size: 22px; margin: 0 0 6px; text-wrap: balance; }
.lead { color: var(--soft); max-width: 70ch; margin: 0 0 22px; }
.grid { display: flex; flex-wrap: wrap; gap: 20px; align-items: flex-start; }
.shot { margin: 0; width: 360px; max-width: 100%; }
.shot figcaption { display: flex; flex-direction: column; gap: 2px; margin-bottom: 10px;
  min-height: 4.6em; }
.shot figcaption span { color: var(--soft); font-size: 13.5px; }
.phone { border: 8px solid var(--frame); border-radius: 26px; overflow: hidden;
  background: var(--tcn-bg, #f3f4f8); height: 720px; display: flex; flex-direction: column;
  font-family: "Helvetica Neue", Helvetica, Arial, sans-serif; color: var(--tcn-text); }
.bar { flex: 0 0 auto; display: flex; align-items: center; gap: 8px; padding: 10px 12px;
  color: #fff; background: linear-gradient(120deg, var(--tcn-primary), var(--tcn-primary-dark, #6d28d9)); }
.bar .dot { width: 8px; height: 8px; border-radius: 50%; background: #22c55e; }
.bar .back { flex: 1 1 auto; min-width: 0; white-space: nowrap; overflow: hidden;
  text-overflow: ellipsis; font-weight: 700; background: rgba(255,255,255,.16);
  border-radius: 999px; padding: 3px 10px; font-size: 14px; }
.bar .more { font-weight: 700; }
.screen { flex: 1 1 auto; overflow-y: auto; padding: 12px; }
.screen .tcn-section-title { margin: 6px 2px 8px; }
.screen .tcn-card { margin-bottom: 14px; }

/* ---- the variants: only what differs from newui.css ---- */

/* a and c: the name and the control share a line; the description runs under both */
.v-a .tcn-setrow, .v-c .tcn-setrow {
  display: grid; grid-template-columns: minmax(0, 1fr) auto; column-gap: 12px; row-gap: 0;
  align-items: center; }
.v-a .tcn-settext, .v-c .tcn-settext { display: contents; }
.v-a .tcn-setname, .v-c .tcn-setname { grid-column: 1; grid-row: 1; }
.v-a .tcw-ctl, .v-c .tcw-ctl { grid-column: 2; grid-row: 1; width: auto; max-width: 11em; }
.v-a .tcn-setdesc, .v-c .tcn-setdesc { grid-column: 1 / -1; grid-row: 2; margin-top: 6px; }
.v-a .tcn-setnum, .v-c .tcn-setnum { width: 96px; }
.tcw-ctl { box-sizing: border-box; }

/* b: one column; the control is the last thing in the row, full width */
.v-b .tcn-setrow { flex-direction: column; align-items: stretch; gap: 8px; }
.v-b .tcw-ctl { width: 100%; }
.v-b .tcn-setnum { text-align: left; }

/* c: one sentence, the rest folded */
.tcw-more { display: inline; }
.tcw-more summary { display: inline; cursor: pointer; color: var(--tcn-primary);
  list-style: none; font-weight: 600; }
.tcw-more summary::-webkit-details-marker { display: none; }
.tcw-more[open] summary { display: none; }
.tcw-more > div { display: inline; }
.tcw-more summary:focus-visible { outline: 2px solid var(--tcn-primary); outline-offset: 2px; }
'''

page = f'''<title>Настройки на узком экране</title>
<style>
{NEWUI}
{PAGE_CSS}
</style>
<div class="wrap">
  <h1>Настройки на узком экране</h1>
  <p class="lead">Настоящие стили приложения и настоящие русские тексты, экран шириной 360 px.
  Всё прокручивается внутри телефона; в варианте C «подробнее» раскрывается по нажатию.
  На широком экране варианты A и C почти не отличаются от нынешнего, B — становится столбиком везде,
  если не ограничить его узкими экранами.</p>
  <div class="grid">
    {screen("current", "Как сейчас", "Текст слева, контрол справа: описание сжато в колонку.")}
    {screen("a", "A. Контрол у названия", "Название и контрол в строку, описание под ними на всю ширину.")}
    {screen("b", "B. Всё в столбик", "Название, описание, контрол под ними на всю ширину.")}
    {screen("c", "C. Как A, тексты короче", "Одна фраза, остальное — «подробнее».")}
  </div>
</div>
'''

(HERE / 'proto.html').write_text(page, encoding='utf-8')
print('готово:', len(page), 'символов')
