// Makes the app open without a network.
//
// Deliberately small. There is no list of files to pre-cache: the build renames them on
// every change (the hash is in the name), so a hardcoded list would be wrong the moment it
// was written. Instead whatever is fetched successfully is kept, and served from there when
// the network is gone.
//
// The API is never cached. Stale money is worse than no money, and what the app should show
// when offline is not "yesterday's answer from the server" but "the tour this device has,
// with the edits this device has made" - which the queue in localStorage already knows.

const CACHE = 'tcw-v1';

self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', (e) => e.waitUntil(self.clients.claim()));

self.addEventListener('fetch', (event) => {
    const req = event.request;
    if (req.method !== 'GET') return;

    const url = new URL(req.url);
    if (url.origin !== self.location.origin) return;
    if (url.pathname.startsWith('/api/')) return;

    // Opening a link: prefer the network, so a new build is picked up, and fall back to the
    // page we kept. Every route is the same document - the app reads the path itself.
    if (req.mode === 'navigate') {
        event.respondWith(
            fetch(req)
                .then((resp) => {
                    const copy = resp.clone();
                    caches.open(CACHE).then((c) => c.put('/', copy));
                    return resp;
                })
                .catch(() => caches.match('/').then((hit) => hit || offlinePage()))
        );
        return;
    }

    // Everything else - wasm, js, css - is content-addressed by its name, so a hit is
    // always the right answer and is worth taking before the network.
    event.respondWith(
        caches.match(req).then(
            (hit) =>
                hit ||
                fetch(req).then((resp) => {
                    if (resp.ok) {
                        const copy = resp.clone();
                        caches.open(CACHE).then((c) => c.put(req, copy));
                    }
                    return resp;
                })
        )
    );
});

function offlinePage() {
    return new Response(
        '<!doctype html><meta charset="utf-8"><style>body{font:15px system-ui;padding:40px;color:#111827}</style>' +
            '<h1>Offline</h1><p>This device has not opened Tourcalc before, so there is nothing here to show yet.</p>',
        { headers: { 'Content-Type': 'text/html; charset=utf-8' } }
    );
}
