// DBG In development, always fetch from the network and do not enable offline support.
// This is because caching would make development more difficult (changes would not
// be reflected on the first load after each change).
self.addEventListener('fetch', () => { });
self.addEventListener('push', event => {
    //alert("push sw-dbg");
    const payload = event.data.json();
    event.waitUntil(
        self.registration.showNotification('Tourcalc', {
            body: payload.message,
            vibrate: [100, 50, 100]
        })
    );
});
