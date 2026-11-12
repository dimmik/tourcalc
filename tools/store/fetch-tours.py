#!/usr/bin/env python3
"""Fetch tours, with all their versions, from a running Tourcalc server by share links.

    python3 tools/store/fetch-tours.py <out.json> <share link>...

A share link is the app's "/goto/<code md5>/<tour id>". The output is a JSON array of tours -
each tour followed by its versions - in the shape of the InMemory seed file, which is what
`tc-server import` reads. The tours are people's data: write the file outside the repository.
Only reads; standard library only.
"""
import json
import sys
import urllib.parse
import urllib.request


def get(url, token=None):
    req = urllib.request.Request(url, headers={"User-Agent": "tourcalc-fetch-tours"})
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    with urllib.request.urlopen(req, timeout=60) as r:
        return r.read().decode("utf-8")


def field(d, name):
    """Either spelling: the C# server answers camelCase, the seed file is PascalCase."""
    if name in d:
        return d[name]
    return d.get(name[0].lower() + name[1:])


def fetch(link):
    u = urllib.parse.urlparse(link)
    parts = [p for p in u.path.split("/") if p]
    if len(parts) != 3 or parts[0] != "goto":
        raise SystemExit(f"not a share link (/goto/<code md5>/<tour id>): {link}")
    base = f"{u.scheme}://{u.netloc}"
    md5, tour_id = parts[1], parts[2]
    token = get(f"{base}/api/Auth/token/code/{md5}/md5").strip().strip('"')
    tour = json.loads(get(f"{base}/api/Tour/{tour_id}", token))
    out = [tour]
    start = 0
    while True:
        page = json.loads(get(f"{base}/api/Tour/{tour_id}/versions?from={start}&count=50", token))
        items = field(page, "Tours") or []
        for v in items:
            out.append(json.loads(get(f"{base}/api/Tour/{field(v, 'Id')}", token)))
        start += len(items)
        if not items or start >= (field(page, "TotalCount") or 0):
            break
    print(f"{field(tour, 'Name')!r}: the tour and {len(out) - 1} version(s)", file=sys.stderr)
    return out


def main():
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    tours = []
    for link in sys.argv[2:]:
        tours.extend(fetch(link))
    with open(sys.argv[1], "w", encoding="utf-8") as f:
        json.dump(tours, f, ensure_ascii=False)


if __name__ == "__main__":
    main()
