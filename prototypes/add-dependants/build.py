#!/usr/bin/env python3
"""Прототип: «+ платит за…» — завести тех, за кого человек платит, прямо от него.

Сейчас, чтобы завести ребёнка или супругу, за которых платит Дима, надо нажать «+ Add
person», вписать имя, выбрать в «Paid by» Диму — и так на каждого. Здесь — кнопка у
самого Димы: раскрывается форма, строки добавляются сами, как в окне валют, внизу одна
кнопка на всех.

Собирается из настоящих стилей приложения и настоящей разметки вкладки People, снятых с
живой страницы скриптом capture.py (app.css, captured-*.html, 26.09.2026). Результат —
proto.html рядом.
"""
import html
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent
CSS = (HERE / 'app.css').read_text(encoding='utf-8')


def clean(markup):
    markup = re.sub(r'<!---->', '', markup)
    return markup


FULL = clean((HERE / 'captured-full.html').read_text(encoding='utf-8'))
MINI = clean((HERE / 'captured-mini-open.html').read_text(encoding='utf-8'))
OPEN = clean((HERE / 'captured-open-person.html').read_text(encoding='utf-8'))

# --- the full interface ----------------------------------------------------------------
# The People list as captured, with Дима Т. opened and the new button among his actions.
people = FULL[FULL.index('<div class="tcn-list tcn-people-list">'):]
people = people[:people.index('</main>')]
# Replace Дима's compact card (the second family) with the opened one.
families = [m.start() for m in re.finditer(r'<div class="tcn-family', people)]
second, third = families[1], families[2]
opened = OPEN.replace(
    '<button class="tcn-btn tcn-btn-sm" type="button">Edit</button>',
    '<button class="tcn-btn tcn-btn-sm" type="button">Edit</button>'
    '<button class="tcn-btn tcn-btn-sm is-new" type="button" data-open-form>+ Pays for…</button>')
# The form goes after the actions, inside the card.
opened = opened[:opened.rindex('</div></div>')] + '<div class="dep-slot" data-mode="full"></div></div></div>'
people = people[:second] + opened + people[third:]
# The family that already exists: one more "pays for" at the end of its list.
people = people.replace(
    '</div></div></div><div class="tcn-family"><div class="tcn-person is-compact"><div class="tcn-person-row">'
    '<button class="tcn-person-open" type="button" aria-expanded="false"><span style="background:linear-gradient'
    '(135deg, hsl(333',
    '</div><button class="tcn-btn tcn-btn-sm dep-more is-new" type="button" data-open-form>+ Pays for someone '
    'else</button><div class="dep-slot" data-mode="full"></div></div></div><div class="tcn-family"><div class='
    '"tcn-person is-compact"><div class="tcn-person-row"><button class="tcn-person-open" type="button" '
    'aria-expanded="false"><span style="background:linear-gradient(135deg, hsl(333', 1)

# --- mini -------------------------------------------------------------------------------
mini = MINI
# Дима Т. opened in mini, with the same button; Саша О.'s row gets it too.
dima_row = ('<div class="tcm-item"><div class="tcm-row"><button class="tcm-main tcm-open" type="button" '
            'aria-expanded="false"><span class="tcm-name">Дима Т.</span></button>')
assert dima_row in mini, 'Дима in mini'
mini = mini.replace(
    dima_row + '<span class="tcm-half is-pos"></span><span class="tcm-half is-neg">2\u202f974</span></div></div>',
    '<div class="tcm-item"><div class="tcm-row"><button class="tcm-main tcm-open" type="button" '
    'aria-expanded="true"><span class="tcm-name">Дима Т.</span></button><span class="tcm-half is-pos"></span>'
    '<span class="tcm-half is-neg">2\u202f974</span></div><div class="tcm-sub"><div class="tcm-facts-line"><span>'
    'paid <b class="tcm-money">38 437</b></span><span class="tcm-dot">·</span><span>charged <b class="tcm-money">'
    '41 412</b></span><span class="tcm-dot">·</span><span>weight <b>100</b></span></div><div class="tcm-fields">'
    '<button class="tcm-btn is-primary" type="button">spend</button><button class="tcm-btn" type="button">edit'
    '</button><button class="tcm-btn is-new" type="button" data-open-form>+ pays for…</button><button '
    'class="tcm-btn is-danger" type="button">delete</button></div><div class="dep-slot" data-mode="mini"></div>'
    '</div></div>', 1)
mini = mini.replace(
    '<div class="tcm-fields"><button class="tcm-btn is-primary" type="button">spend</button><button class="tcm-btn" '
    'type="button">edit</button><button class="tcm-btn is-danger" type="button">delete</button></div></div></div>'
    '<div class="tcm-item is-child is-fam">',
    '<div class="tcm-fields"><button class="tcm-btn is-primary" type="button">spend</button><button class="tcm-btn" '
    'type="button">edit</button><button class="tcm-btn is-new" type="button" data-open-form>+ pays for…</button>'
    '<button class="tcm-btn is-danger" type="button">delete</button></div><div class="dep-slot" data-mode="mini">'
    '</div></div></div><div class="tcm-item is-child is-fam">', 1)

# Inside each phone: the app's own page, in an iframe 400 px wide - the app lays itself out
# by the width of the window, and in a wide page the People cards took their desktop layout.
PHONE_CSS = '''
body { margin: 0; background: var(--tcn-bg, #f3f4f8); }
.tcn-list { padding: 12px; }
/* what is new, marked */
.is-new { outline: 2px dashed var(--tcn-primary, #4f46e5); outline-offset: 2px; }
/* the form: "Дима Т. pays for" */
.dep-form { margin-top: 12px; padding: 12px; border-radius: 12px;
  background: var(--tcn-primary-soft, #eef0ff); display: flex; flex-direction: column; gap: 8px; }
.dep-form h4, .dep-mini h4 { margin: 0; font-size: 14px; }
.dep-row { display: flex; align-items: center; gap: 8px; }
.dep-row .tcn-input { min-width: 0; }
.dep-row .dep-name { flex: 1 1 auto; }
.dep-row .dep-weight { flex: 0 0 72px; text-align: right; }
.dep-row .dep-x { flex: 0 0 auto; width: 30px; }
.dep-hint { font-size: 12px; color: var(--tcn-muted, #5b6472); }
.dep-foot { display: flex; gap: 8px; justify-content: flex-end; flex-wrap: wrap; }
.dep-more { margin: 8px 12px 12px; }
/* mini: the same, in mini's own terms */
.dep-mini { margin: 6px 0 2px; padding: 8px; border: 1px solid #d7dbe7; background: #fff;
  display: flex; flex-direction: column; gap: 6px; font-size: 13px; }
.dep-mini .dep-row { gap: 6px; }
.dep-mini input { font: inherit; padding: 4px 6px; border: 1px solid #c9cedb; border-radius: 3px; min-width: 0; }
.dep-mini .dep-name { flex: 1 1 auto; }
.dep-mini .dep-weight { flex: 0 0 52px; text-align: right; }
.dep-done { color: #15803d; font-size: 13px; padding: 4px 0; }
'''

PAGE_CSS = '''
:root { --page: #eef0f6; --ink: #1b1f2e; --soft: #5b6275; --frame: #1b1f2e; --line: #d7dbe7;
  --mark: #4f46e5; --mark-soft: #eef0ff; }
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) { --page: #14161f; --ink: #e8eaf2; --soft: #9aa1b5;
    --frame: #05060a; --line: #2a2e3d; --mark: #a5a2ff; --mark-soft: #23244a; color-scheme: dark; }
}
:root[data-theme="dark"] { --page: #14161f; --ink: #e8eaf2; --soft: #9aa1b5; --frame: #05060a;
  --line: #2a2e3d; --mark: #a5a2ff; --mark-soft: #23244a; color-scheme: dark; }
body { background: var(--page); color: var(--ink); margin: 0;
  font: 15px/1.55 "Helvetica Neue", Helvetica, Arial, sans-serif; }
.wrap { max-width: 1180px; margin: 0 auto; padding-inline: 16px; padding-block: 24px 48px; }
h1 { font-size: 22px; margin: 0 0 6px; text-wrap: balance; }
h2 { font-size: 17px; margin: 28px 0 8px; }
.lead, .note { color: var(--soft); max-width: 72ch; margin: 0 0 14px; }
.names { border-collapse: collapse; margin: 6px 0 18px; font-size: 14px; max-width: 100%; }
.names th, .names td { text-align: left; padding: 6px 10px; border-bottom: 1px solid var(--line); vertical-align: top; }
.names th { color: var(--soft); font-size: 12px; text-transform: uppercase; letter-spacing: .04em; }
.names tr.pick td { background: var(--mark-soft); }
.tablewrap { overflow-x: auto; }
.grid { display: flex; flex-wrap: wrap; gap: 24px; align-items: flex-start; }
.shot { margin: 0; width: 400px; max-width: 100%; }
.shot figcaption { margin-bottom: 10px; }
.shot figcaption span { display: block; color: var(--soft); font-size: 13.5px; }
.phone { border: 8px solid var(--frame); border-radius: 26px; overflow: hidden; height: 820px;
  background: var(--tcn-bg, #f3f4f8); color: var(--tcn-text, #1b1f2e); display: flex; flex-direction: column; }
.phone iframe { border: 0; width: 100%; height: 100%; display: block; }
'''

SCRIPT = '''
const T = {
  full: { title: n => n + ' pays for', name: 'name', add: n => n === 1 ? 'Add 1 person' : 'Add ' + n + ' people',
          none: 'Add', cancel: 'Cancel', hint: 'Weight: 100 is a grown-up’s share; 50 half of it, say for a child.' },
  mini: { title: n => n + ' pays for', name: 'name', add: n => 'add ' + n, none: 'add', cancel: 'cancel',
          hint: 'weight 100 = a grown-up’s share' },
};
function whose(slot) {
  const card = slot.closest('.tcn-family, .tcm-item');
  const n = card && card.querySelector('.tcn-person-name, .tcm-name');
  return n ? n.textContent.trim() : '';
}
function row(mode) {
  const r = document.createElement('div');
  r.className = 'dep-row';
  const cls = mode === 'full' ? 'tcn-input ' : '';
  r.innerHTML = `<input class="${cls}dep-name" type="text" placeholder="${T[mode].name}" aria-label="Name">` +
    `<input class="${cls}dep-weight" type="text" inputmode="numeric" value="100" aria-label="Weight">` +
    (mode === 'full' ? '<button type="button" class="tcn-btn tcn-btn-sm tcn-btn-danger dep-x" aria-label="Remove">✕</button>'
                     : '<button type="button" class="tcm-btn dep-x" aria-label="Remove">✕</button>');
  return r;
}
function count(form) {
  return [...form.querySelectorAll('.dep-name')].filter(i => i.value.trim()).length;
}
function refresh(form, mode) {
  const n = count(form);
  form.querySelector('.dep-add').textContent = n ? T[mode].add(n) : T[mode].none;
  form.querySelector('.dep-add').disabled = !n;
  // Always exactly one empty row at the end, as in the currencies dialog.
  const rows = [...form.querySelectorAll('.dep-row')];
  const last = rows[rows.length - 1];
  if (last.querySelector('.dep-name').value.trim()) form.querySelector('.dep-rows').appendChild(row(mode));
  rows.slice(0, -1).forEach(r => r.querySelector('.dep-x').hidden = false);
  [...form.querySelectorAll('.dep-row')].slice(-1)[0].querySelector('.dep-x').hidden = true;
}
function open(slot) {
  if (slot.firstChild) { slot.innerHTML = ''; return; }
  const mode = slot.dataset.mode;
  const form = document.createElement('div');
  form.className = mode === 'full' ? 'dep-form' : 'dep-mini';
  const btn = mode === 'full' ? 'tcn-btn tcn-btn-sm' : 'tcm-btn';
  form.innerHTML = `<h4>${T[mode].title(whose(slot))}</h4><div class="dep-rows"></div>` +
    `<div class="dep-hint">${T[mode].hint}</div>` +
    `<div class="dep-foot"><button type="button" class="${btn} dep-cancel">${T[mode].cancel}</button>` +
    `<button type="button" class="${btn} ${mode === 'full' ? 'tcn-btn-primary' : 'is-primary'} dep-add"></button></div>`;
  form.querySelector('.dep-rows').appendChild(row(mode));
  slot.appendChild(form);
  refresh(form, mode);
  form.querySelector('.dep-name').focus();
  form.addEventListener('input', () => refresh(form, mode));
  form.addEventListener('click', e => {
    if (e.target.closest('.dep-x')) { e.target.closest('.dep-row').remove(); refresh(form, mode); }
    if (e.target.closest('.dep-cancel')) slot.innerHTML = '';
    if (e.target.closest('.dep-add')) {
      const names = [...form.querySelectorAll('.dep-row')]
        .map(r => [r.querySelector('.dep-name').value.trim(), r.querySelector('.dep-weight').value.trim()])
        .filter(([n]) => n);
      slot.innerHTML = `<div class="dep-done">✓ ${names.map(([n, w]) => w === '100' ? n : n + ' ×' + w).join(', ')} — ` +
        (mode === 'full' ? 'paid for by ' : 'paid for by ') + whose(slot) + '</div>';
    }
  });
}
document.addEventListener('click', e => {
  const b = e.target.closest('[data-open-form]');
  if (!b) return;
  const host = b.closest('.tcn-family, .tcm-item');
  open(host.querySelector('.dep-slot'));
});
'''

def phone(markup):
    doc = (f'<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width">'
           f'<style>{CSS}{PHONE_CSS}</style><main class="tcn-main">{markup}</main><script>{SCRIPT}</script>')
    return f'<iframe title="phone" srcdoc="{html.escape(doc, quote=True)}"></iframe>'


page = f'''<title>Платит за…</title>
<style>
{PAGE_CSS}
</style>
<div class="wrap">
  <h1>«+ Платит за…»: завести тех, за кого человек платит, прямо от него</h1>
  <p class="lead">Сейчас, чтобы завести детей или супругу, за которых платит Дима, надо на каждого нажать
  «+ Add person», вписать имя и выбрать в «Paid by» Диму. Здесь — кнопка у самого Димы: раскрывается
  форма, строка для следующего добавляется сама (как в окне валют), внизу одна кнопка на всех. Нажмите
  пунктирные кнопки — всё кликается: печатайте имена, ✕ убирает строку, «Add» показывает итог.
  Настоящие стили и разметка приложения; новое обведено пунктиром.</p>

  <h2>Как назвать</h2>
  <p class="note">Это не всегда дети — бывают супруги, родители, друг, за которого платишь. В приложении
  уже есть «Pays for themselves» и «paid for by …», так что естественно продолжить этими словами.</p>
  <div class="tablewrap"><table class="names">
    <tr><th>кнопка (en / ru)</th><th>плюсы</th><th>минусы</th></tr>
    <tr class="pick"><td><b>+ Pays for…</b> / <b>+ Платит за…</b> — предлагаю</td><td>те же слова, что уже в
      приложении («paid for by», «pays for themselves»); говорит, что меняется в расчёте</td><td>—</td></tr>
    <tr><td>+ Family member / + Член семьи</td><td>понятно для детей</td><td>не всегда семья: друг, коллега</td></tr>
    <tr><td>+ Dependant / + Иждивенец</td><td>точно по смыслу</td><td>канцелярит, по-русски звучит обидно</td></tr>
    <tr><td>+ Companion / + Спутник</td><td>нейтрально</td><td>не говорит, что за него платят</td></tr>
  </table></div>

  <div class="grid">
    <figure class="shot">
      <figcaption><b>Обычный интерфейс</b><span>У раскрытого человека — «+ Pays for…» рядом с Edit. У семьи
      Саши О. — «+ Pays for someone else» в конце списка.</span></figcaption>
      <div class="phone">{phone(people)}</div>
    </figure>
    <figure class="shot">
      <figcaption><b>Mini</b><span>То же в строке раскрытого человека: «+ pays for…» между edit и delete;
      у Саши О. тоже.</span></figcaption>
      <div class="phone">{phone(mini)}</div>
    </figure>
  </div>

  <h2>Что ещё решено</h2>
  <ul class="note">
    <li>Вес по умолчанию — 100 (взрослая доля), его можно поменять в строке; подсказка — одной строкой.</li>
    <li>Имя фокусируется сразу, как форма раскрылась, — печатать можно тут же.</li>
    <li>«Добавить» заводит всех одной правкой тура: одна версия, одно уведомление.</li>
    <li>Кнопка есть у того, кто платит за себя, и у главы семьи. У того, за кого платят (Валечка),
      её нет: платить за других может только тот, кто платит сам.</li>
    <li>В текстовых страницах <code>/t</code> не делаем.</li>
  </ul>
</div>
'''

(HERE / 'proto.html').write_text(page, encoding='utf-8')
print('proto.html', len(page))
