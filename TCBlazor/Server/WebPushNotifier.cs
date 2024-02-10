using System.Text.Json;
using TCalcCore.Storage;
using TCalcCore.UI;
using TCBlazor.Client.SharedCode;
using WebPush;

namespace TCBlazor.Server
{
    public class WebPushNotifier : INotifier
    {
        private readonly ISubscriptionStorage _storage;

        public WebPushNotifier(ISubscriptionStorage storage)
        {
            _storage = storage;
        }

        public async void Notify(string tourId, string message)
        {
            var subs = _storage.GetSubscriptions(tourId);
            foreach (var sub in subs)
            {
                await SendNotificationAsync(sub, message);
            }
        }

        private static async Task SendNotificationAsync(NotificationSubscription subscription, string message)
        {
            // For a real application, generate your own
            var publicKey = "BLZ-598ZLl7rRa98qQcQicg2E4OOBUDIZUt14ZANf6HTGMKVm-2bYuL00JQjC1Hicio7P1kpnIC4SuPfrnKLJeI";
            var privateKey = "rGzYSdZf1K1ppjpKXqsEtBCwE-SHucoFgmn5d7PUUiA";

            var pushSubscription = new PushSubscription(subscription.Url, subscription.P256dh, subscription.Auth);
            var vapidDetails = new VapidDetails("mailto:tourcalc@dimmik.org", publicKey, privateKey);
            var webPushClient = new WebPushClient();
            try
            {
                Console.WriteLine($"SEND NOTIFICATION: start ({message})");
                var payload = JsonSerializer.Serialize(new
                {
                    message
                });
                await webPushClient.SendNotificationAsync(pushSubscription, payload, vapidDetails);
                Console.WriteLine("SEND NOTIFICATION: Done");
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine("SEND NOTIFICATION: Error sending push notification: " + ex.Message);
            }
        }
    }
}
