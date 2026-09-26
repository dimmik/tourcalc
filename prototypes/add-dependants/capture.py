#!/usr/bin/env python3
"""Снимает живую разметку вкладки People — в обычном интерфейсе и в mini — и стили приложения.

Нужен запущенный Tourcalc (rust/build-and-run.sh) и headless Chrome с портом отладки, как в
шапке tools/browser/shot.py. Результат — рядом: captured-full.html, captured-mini.html (то,
что внутри <main> на вкладке People) и app.css (все стили страницы одним файлом).

    python3 capture.py http://localhost:5411/goto/<код>/<тур>
"""
import json
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent.parent / 'tools' / 'browser'))
from shot import Tab, open_tab, go  # noqa: E402

GRAB = r"""
(async () => {
  const pause = ms => new Promise(r => setTimeout(r, ms));
  const tab = [...document.querySelectorAll('button, a, span')]
    .find(e => /^\s*(people|люди)\s*\d*\s*$/i.test(e.textContent));
  tab?.click();
  await pause(900);
  // Opened: somebody paying for themselves, and a family's list of those paid for.
  document.querySelectorAll('.tcn-person-open')[1]?.click();
  await pause(700);
  [...document.querySelectorAll('button, a, span')]
    .find(e => /paid for by|платит/i.test(e.textContent) && e.tagName === 'BUTTON')?.click();
  await pause(600);
  const main = document.querySelector('main') || document.body;
  return main.outerHTML;
})()
"""

CSS = r"""
(async () => {
  const parts = [];
  for (const node of document.querySelectorAll('link[rel=stylesheet], style')) {
    if (node.tagName === 'STYLE') parts.push(node.textContent);
    else { try { parts.push(await (await fetch(node.href)).text()); } catch (e) {} }
  }
  return parts.join('\n');
})()
"""


def evaluate(tab, js):
    r = tab.call("Runtime.evaluate", expression=js, awaitPromise=True, returnByValue=True)
    return r.get("result", {}).get("value", "")


def main():
    link = sys.argv[1]
    info = open_tab(9240)
    tab = Tab(info["webSocketDebuggerUrl"])
    try:
        tab.call("Page.enable")
        tab.call("Emulation.setDeviceMetricsOverride", width=420, height=900,
                 deviceScaleFactor=2, mobile=True)
        go(tab, link, 6)
        tour = link.rstrip('/').split('/')[-1]
        base = link.split('/goto/')[0]
        for mode in ('new', 'mini'):
            evaluate(tab, f"localStorage.setItem('__tc_ui_mode', '{mode}')")
            go(tab, f"{base}/tour/{tour}", 5)
            html = evaluate(tab, GRAB)
            name = 'full' if mode == 'new' else 'mini'
            (HERE / f'captured-{name}.html').write_text(html, encoding='utf-8')
            print(name, len(html))
        (HERE / 'app.css').write_text(evaluate(tab, CSS), encoding='utf-8')
        evaluate(tab, "localStorage.setItem('__tc_ui_mode', 'new')")
    finally:
        import urllib.request
        urllib.request.urlopen(f"http://localhost:9240/json/close/{info['id']}")


if __name__ == '__main__':
    main()
