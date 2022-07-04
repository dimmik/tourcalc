window.updateAvailable = new Promise((resolve, reject) => {
  if (!('serviceWorker' in navigator)) {
    const errorMessage = `This browser doesn't support service workers`;
    console.error(errorMessage);
    reject(errorMessage);
    return;
  }

  navigator.serviceWorker.register('/service-worker.js')
    .then(registration => {
      console.info(`Service worker registration successful (scope: ${registration.scope})`);

      setInterval(() => {
          console.info('Check for update');
          registration.update();
      }, 3 * 60 * 1000); // 3 * 60000ms -> check each 3 minutes

      registration.onupdatefound = () => {
        const installingServiceWorker = registration.installing;
        installingServiceWorker.onstatechange = () => {
          if (installingServiceWorker.state === 'installed') {
            resolve(!!navigator.serviceWorker.controller);
          }
        }
      };
    })
    .catch(error => {
      console.error('Service worker registration failed with error:', error);
      reject(error);
    });
});

window.registerForUpdateAvailableNotification = (caller, methodName) => {
  window.updateAvailable.then(isUpdateAvailable => {
    if (isUpdateAvailable) {
      caller.invokeMethodAsync(methodName).then();
    }
  });
};