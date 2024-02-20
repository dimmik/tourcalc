// JUST In development, always fetch from the network and do not enable offline support.
// This is because caching would make development more difficult (changes would not
// be reflected on the first load after each change).
self.addEventListener('fetch', () => { });
self.addEventListener('push', event => {
    const payload = event.data.json();
    event.waitUntil(
        self.registration.showNotification('Tourcalc', {
            body: payload.message,
            icon: '/icon-192.png',
            data: { tourId: payload.tourId },
            vibrate: [800, 50, 10, 100, 50]
        })
    );
});
self.addEventListener('notificationclick', (event) => {
    const url = '/tour/' + event.notification.data.tourId
    console.log('On notification click: ', event.notification.data.tourId);
    event.notification.close();
    // This looks to see if the current is already open and
    // focuses if it is
    event.waitUntil(
        clients.matchAll({ type: 'window', includeUncontrolled: true }).then(windowClients => {
            // Check if there is already a window/tab open with the target URL
            for (var i = 0; i < windowClients.length; i++) {
                var client = windowClients[i];
                // If so, just focus it.
                //console.log('client.url: ' + client.url + ' URL: ' + url + ' === ' + (client.url.endsWith(url)));
                if (client.url.endsWith(url) && 'focus' in client) {
                    //console.log('Found!!!');
                    return client.focus();
                }
            }
            // If not, then open the target URL in a new window/tab.
            if (clients.openWindow) {
                return clients.openWindow(url);
            }
        })
    );
});