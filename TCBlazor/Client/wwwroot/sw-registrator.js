window.updateAvailableP = () => new Promise((resolve, reject) => {
    console.info("in promise updateAvailable");
  if (!('serviceWorker' in navigator)) {
    const errorMessage = `This browser doesn't support service workers`;
    console.error(errorMessage);
    reject(errorMessage);
    return;
  }
  console.info("ServiceWorker is available");
  navigator.serviceWorker.register('/service-worker.js')
    .then(registration => {
      console.info(`Service worker registration successful (scope: ${registration.scope})`);

      setInterval(() => {
          console.info('Check for update');
          registration.update();
      }, 3 * 60 * 1000); // 3 * 60000ms -> check each 3 minutes
      

      registration.onupdatefound = () => {
        console.info("update found");
        const installingServiceWorker = registration.installing;
        installingServiceWorker.onstatechange = () => {
          console.info("installingServiceWorker.state: " + installingServiceWorker.state);
          console.info("navigator.serviceWorker.controller: " + navigator.serviceWorker.controller);
          console.info("!!navigator.serviceWorker.controller: " + (!!navigator.serviceWorker.controller));
          controllerOk = !!navigator.serviceWorker.controller;
          if (controllerOk) {
            console.info("Calling resolve with " + controllerOk);
            resolve(controllerOk);
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
    window.updateAvailableP().then(isUpdateAvailable => {
        console.info("window.updateAvailable.then(available: " + isUpdateAvailable + " => { ... })");
        if (isUpdateAvailable) {
          console.info("well, update IS available");
          caller.invokeMethodAsync(methodName).then();
        } else {
            console.info("NO UPDATE!!!");
            //caller.invokeMethodAsync(methodName).then();
        }
    });
};