using System.Text.Json;
using TCalcCore.Storage;
using TCalcCore.UI;
using TCBlazor.Client.SharedCode;
using TourCalcWebApp;
using WebPush;

namespace TCBlazor.Server
{
    public class WebPushNotifier : INotifier
    {
        private readonly ISubscriptionStorage _storage;
        private readonly string publicKey;
        private readonly string privateKey;
        private readonly string mailto;

        public WebPushNotifier(ISubscriptionStorage storage, ITcConfiguration config)
        {
            _storage = storage;
            publicKey = config.GetValue<string>("PushNotificationPublicKey");
            privateKey = config.GetValue<string>("PushNotificationPrivateKey");
            mailto = config.GetValue<string>("PushNotificationMailto");
        }

        public async void Notify(string tourId, string message)
        {
            var subs = _storage.GetSubscriptions(tourId);
            foreach (var sub in subs)
            {
                await SendNotificationAsync(sub, tourId, message);
            }
        }

        private async Task SendNotificationAsync(NotificationSubscription subscription, string tourId, string message)
        {
            var pushSubscription = new PushSubscription(subscription.Url, subscription.P256dh, subscription.Auth);
            var vapidDetails = new VapidDetails(mailto, publicKey, privateKey);
            var webPushClient = new WebPushClient();
            try
            {
                //Console.WriteLine($"SEND NOTIFICATION: start ({message})");
                var payload = JsonSerializer.Serialize(new
                {
                    message, tourId
                });
                await webPushClient.SendNotificationAsync(pushSubscription, payload, vapidDetails);
                //Console.WriteLine("SEND NOTIFICATION: Done");
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine("SEND NOTIFICATION: Error sending push notification: " + ex.Message);
            }
        }
    }
}
