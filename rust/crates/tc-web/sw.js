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

// Which notification handling this copy has, for a test to be sure which copy answered.
self.TC_NOTIFY = 5;

self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', (e) => e.waitUntil(self.clients.claim()));

self.addEventListener('fetch', (event) => {
    const req = event.request;
    if (req.method !== 'GET') return;

    const url = new URL(req.url);
    if (url.origin !== self.location.origin) return;
    if (url.pathname.startsWith('/api/')) return;
    // The text pages are the server's, not the app's: a document of their own for every
    // address. Kept as "the app" they became what this device showed offline in place of
    // it - and the app, what it showed in place of them.
    if (url.pathname === '/t' || url.pathname.startsWith('/t/')) return;

    // Opening a link: prefer the network, so a new build is picked up, and fall back to the
    // page we kept. Every route is the same document - the app reads the path itself.
    if (req.mode === 'navigate') {
        event.respondWith(
            fetch(req)
                .then((resp) => {
                    if (resp.ok) {
                        const copy = resp.clone();
                        event.waitUntil(keepThePage(copy));
                    }
                    return resp;
                })
                .catch(() => caches.match('/').then((hit) => hit || offlinePage()))
        );
        return;
    }

    // The build renames what it compiles - `tc-web-163cb1d9…_bg.wasm` - so a file with a
    // hash in its name can only ever mean one thing, and a hit is always the right answer.
    if (HASHED.test(url.pathname)) {
        event.respondWith(caches.match(req).then((hit) => hit || fromNetwork(req)));
        return;
    }

    // Everything else keeps its name from one build to the next: the manifest, the icons,
    // this file. Cache-first would freeze them in the browser of anybody who has been here
    // before - the file is fetched once and never asked for again, so a change to it never
    // arrives. Found exactly that way: after a server swapped one client for another, the
    // page went on being handed the *old* app's manifest out of this cache. So the network
    // is asked first, and the copy kept here is what answers when there is no network.
    event.respondWith(
        fromNetwork(req).catch(() => caches.match(req).then((hit) => hit || Response.error()))
    );
});

/// The name of anything the build has renamed after its contents. `_bg` is wasm-bindgen's:
/// `tc-web-<hash>_bg.wasm`. Without it the wasm - the one file that matters - never matched,
/// and was asked of the network on every start like any file that keeps its name.
const HASHED = /-[0-9a-f]{8,}(_bg)?\.[a-z0-9]+$/;

/// Anything that belongs to one build: a hashed file, or a file in a hashed directory.
const BUILT = /-[0-9a-f]{8,}(_bg)?(\.[a-z0-9]+$|\/)/;

/// Keeps the app's page for offline use, and lets go of the builds it no longer names.
///
/// Every build's wasm - a megabyte or so - used to stay here for good: its name is its
/// contents, so nothing ever replaced it, and a phone that had seen thirty deploys carried
/// thirty of them. The page names the files of the build it belongs to; anything renamed by
/// the build and not named there is an older build's.
async function keepThePage(resp) {
    const page = await resp.clone().text();
    const cache = await caches.open(CACHE);
    await cache.put('/', resp);

    // Every address in the page with a build's hash in it: the wasm and its glue, the
    // stylesheets, and the snippets, whose hash is in the directory rather than the file.
    const named = new Set(page.match(/\/[\w./-]*-[0-9a-f]{8,}[\w./-]*/g) || []);
    // A page that names no wasm is not the app's page - an error page from a proxy, say -
    // and is no guide to what is current.
    if (![...named].some((n) => n.endsWith('_bg.wasm'))) return;

    for (const req of await cache.keys()) {
        const path = new URL(req.url).pathname;
        if (BUILT.test(path) && !named.has(path)) await cache.delete(req);
    }
}

function fromNetwork(req) {
    return fetch(req).then((resp) => {
        if (resp.ok) {
            const copy = resp.clone();
            caches.open(CACHE).then((c) => c.put(req, copy));
        }
        return resp;
    });
}

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
            data: { tourId, message },
        })
    );
});

self.addEventListener('notificationclick', (event) => {
    event.notification.close();
    const data = event.notification.data || {};
    const tourId = data.tourId;
    const target = tourId ? `/tour/${tourId}` : '/';
    const showing = (client) => {
        try {
            const path = new URL(client.url).pathname;
            return path === target || path.startsWith(`${target}/`);
        } catch (e) {
            return false;
        }
    };

    // Coming to the front is the browser's to allow, and it does not always: in a window
    // it will not focus, the tour should still open.
    const bringToFront = async (client) => {
        try {
            if ('focus' in client) await client.focus();
        } catch (e) {}
    };

    event.waitUntil(
        self.clients.matchAll({ type: 'window', includeUncontrolled: true }).then(async (clients) => {
            // The window the reader is looking at, or the one that looks like it is on the
            // tour, or whichever there is.
            //
            // `client.url` is the address the window was *loaded* at: moving between screens
            // inside the app changes the address bar without telling the worker, so a window
            // that has walked from one tour to another still looks as if it were on the
            // first. It is worth a guess and not worth a decision - which is why what
            // follows asks the app rather than reading this.
            const first = clients.find((c) => c.focused) || clients.find(showing) || clients[0];
            if (!first) return self.clients.openWindow(target);
            await bringToFront(first);

            // Tell the app where to go rather than moving its window there. `navigate()` is
            // refused for a window this worker does not control - which is why a
            // notification used to bring the app to the front and leave it wherever it was
            // - and even when it works it reloads the whole app. The app knows how to go to
            // a tour without any of that, and how not to throw away a form somebody is in
            // the middle of.
            const told = new Promise((resolve) => {
                const channel = new MessageChannel();
                channel.port1.onmessage = () => resolve(true);
                first.postMessage({ tc: 'open-tour', tourId, message: data.message || '' }, [channel.port2]);
                // A page loaded before this worker existed does not listen; after a moment,
                // do it the old way, which may still be refused, and then there is nothing
                // more to try.
                setTimeout(() => resolve(false), 600);
            });
            if (await told) return undefined;
            // Nobody listening: a page from before this worker. It may be on the tour
            // already - all this has to go on is the address it was loaded at.
            if (showing(first)) return undefined;
            if ('navigate' in first) {
                return first.navigate(target).then((c) => c && c.focus(), () => undefined);
            }
            return undefined;
        })
    );
});
