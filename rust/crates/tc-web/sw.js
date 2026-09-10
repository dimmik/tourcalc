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

// --- push notifications -------------------------------------------------------------------
//
// A page that is closed cannot show anything, which is the whole reason a service worker
// receives these. The payload is what the server encrypted for this subscription: a message
// saying what changed, and the tour it changed in.

self.addEventListener('push', (event) => {
    let message = 'A tour has changed';
    let tourId = '';
    try {
        const data = event.data ? event.data.json() : {};
        if (data.message) message = data.message;
        if (data.tourId) tourId = data.tourId;
    } catch (e) {
        // Not JSON, or nothing at all. A notification still has to appear: the browser
        // demands one for every push it delivers, and swallowing it silently would end
        // with the subscription being revoked.
        if (event.data) message = event.data.text();
    }

    event.waitUntil(
        self.registration.showNotification('Tourcalc', {
            body: message,
            // The tag collapses repeats: ten edits to one tour leave one notification
            // rather than ten, which is what somebody watching a busy tour wants.
            tag: tourId ? `tc-${tourId}` : 'tc',
            renotify: true,
            data: { tourId },
        })
    );
});

self.addEventListener('notificationclick', (event) => {
    event.notification.close();
    const tourId = (event.notification.data || {}).tourId;
    const target = tourId ? `/tour/${tourId}` : '/';

    // Bring an open tab to the front rather than opening a second one.
    event.waitUntil(
        self.clients.matchAll({ type: 'window', includeUncontrolled: true }).then((clients) => {
            for (const client of clients) {
                if (client.url.includes(target) && 'focus' in client) return client.focus();
            }
            if (clients.length && 'navigate' in clients[0]) {
                return clients[0].navigate(target).then((c) => c && c.focus());
            }
            return self.clients.openWindow(target);
        })
    );
});
