#!/usr/bin/env python3
"""Screenshots of a running Tourcalc, through a headless Chrome's DevTools protocol.

Standard library only: the WebSocket handshake and frames are done by hand, which is
fifty lines and saves installing anything on a new machine.

    # a Chrome to drive (Windows Chrome from WSL works; so does a Linux one)
    chrome --headless=new --remote-debugging-port=9240 --remote-allow-origins=* \
           --user-data-dir=<a throwaway folder> --window-size=420,900 about:blank

    tools/browser/shot.py http://localhost:5401/help out.png
    tools/browser/shot.py http://localhost:5401/help out.png \
        --js "document.getElementById('changes').open = true" --full

A fresh profile has no sign-in: pages behind it need `/goto/<code>/<tour id>` opened first
(`--first URL`). The profile folder is ~200 MB - delete it after the push, see CLAUDE.md.
"""
import argparse
import base64
import json
import os
import socket
import struct
import time
import urllib.parse
import urllib.request


class Tab:
    def __init__(self, ws_url):
        u = urllib.parse.urlparse(ws_url)
        self.sock = socket.create_connection((u.hostname, u.port), timeout=30)
        key = base64.b64encode(os.urandom(16)).decode()
        self.sock.sendall(
            (f"GET {u.path} HTTP/1.1\r\nHost: {u.hostname}:{u.port}\r\n"
             "Upgrade: websocket\r\nConnection: Upgrade\r\n"
             f"Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n").encode())
        reply = b""
        while b"\r\n\r\n" not in reply:
            reply += self.sock.recv(4096)
        if b" 101 " not in reply.split(b"\r\n", 1)[0]:
            raise RuntimeError(reply.decode(errors="replace"))
        self.buf = reply.split(b"\r\n\r\n", 1)[1]
        self.next_id = 0

    def _read(self, n):
        while len(self.buf) < n:
            chunk = self.sock.recv(1 << 20)
            if not chunk:
                raise ConnectionError("DevTools closed the connection")
            self.buf += chunk
        out, self.buf = self.buf[:n], self.buf[n:]
        return out

    def _frame(self):
        b1, b2 = self._read(2)
        n = b2 & 0x7F
        if n == 126:
            n = struct.unpack(">H", self._read(2))[0]
        elif n == 127:
            n = struct.unpack(">Q", self._read(8))[0]
        return b1 & 0x0F, self._read(n)

    def call(self, method, **params):
        self.next_id += 1
        data = json.dumps({"id": self.next_id, "method": method, "params": params}).encode()
        mask = os.urandom(4)
        head = bytes([0x81])
        n = len(data)
        head += bytes([0x80 | n]) if n < 126 else (
            bytes([0x80 | 126]) + struct.pack(">H", n) if n < 65536
            else bytes([0x80 | 127]) + struct.pack(">Q", n))
        self.sock.sendall(head + mask + bytes(c ^ mask[i % 4] for i, c in enumerate(data)))
        text = b""
        while True:
            op, payload = self._frame()
            text += payload
            if op == 0x0 or op == 0x1:
                try:
                    msg = json.loads(text)
                except ValueError:
                    continue  # a continuation frame follows
                text = b""
                if msg.get("id") == self.next_id:
                    if "error" in msg:
                        raise RuntimeError(msg["error"])
                    return msg.get("result", {})


def open_tab(port):
    # /json/new wants PUT; a GET is refused with 405.
    req = urllib.request.Request(f"http://localhost:{port}/json/new?about:blank", method="PUT")
    return json.load(urllib.request.urlopen(req))


def go(tab, url, wait):
    tab.call("Page.navigate", url=url)
    time.sleep(wait)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("url")
    ap.add_argument("out")
    ap.add_argument("--port", type=int, default=9240, help="Chrome's debugging port")
    ap.add_argument("--first", help="open this before the page, e.g. a /goto link")
    ap.add_argument("--js", action="append", default=[], help="run in the page before the shot")
    ap.add_argument("--wait", type=float, default=3.0, help="seconds to let a page settle")
    ap.add_argument("--width", type=int, default=420)
    ap.add_argument("--height", type=int, default=900)
    ap.add_argument("--full", action="store_true", help="the whole page, not the window")
    ap.add_argument("--dark", action="store_true", help="prefers-color-scheme: dark")
    a = ap.parse_args()

    info = open_tab(a.port)
    tab = Tab(info["webSocketDebuggerUrl"])
    try:
        tab.call("Page.enable")
        tab.call("Emulation.setDeviceMetricsOverride", width=a.width, height=a.height,
                 deviceScaleFactor=2, mobile=a.width < 600)
        if a.dark:
            tab.call("Emulation.setEmulatedMedia",
                     features=[{"name": "prefers-color-scheme", "value": "dark"}])
        if a.first:
            go(tab, a.first, a.wait)
        go(tab, a.url, a.wait)
        for js in a.js:
            tab.call("Runtime.evaluate", expression=js, awaitPromise=True)
        time.sleep(0.5)
        params = {"format": "png", "captureBeyondViewport": a.full}
        if a.full:
            size = tab.call("Page.getLayoutMetrics")["cssContentSize"]
            params["clip"] = {"x": 0, "y": 0, "width": size["width"],
                              "height": size["height"], "scale": 1}
        png = base64.b64decode(tab.call("Page.captureScreenshot", **params)["data"])
        with open(a.out, "wb") as f:
            f.write(png)
        print(a.out, len(png), "bytes")
    finally:
        urllib.request.urlopen(f"http://localhost:{a.port}/json/close/{info['id']}")


if __name__ == "__main__":
    main()
