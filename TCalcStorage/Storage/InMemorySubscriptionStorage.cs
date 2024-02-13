using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using TCalcCore.Storage;
using TCBlazor.Client.SharedCode;

namespace TCalcStorage.Storage
{
    public class InMemorySubscriptionStorage : ISubscriptionStorage
    {
        private Dictionary<string, List<NotificationSubscription>> _subscriptions = [];

        public void AddSubscription(string tourId, NotificationSubscription sub)
        {
            if (!_subscriptions.ContainsKey(tourId))
            {
                _subscriptions[tourId] = [];
            }
            if (!_subscriptions[tourId].Contains(sub))
            {
                _subscriptions[tourId].Add(sub);
            }
        }

        public bool CheckSubscription(string tourId, NotificationSubscription sub)
        {
            if (!_subscriptions.ContainsKey(tourId)) return false;
            return _subscriptions[tourId].Contains(sub);
        }

        public IEnumerable<NotificationSubscription> GetSubscriptions(string tourId)
        {
            if (!_subscriptions.ContainsKey(tourId)) return Enumerable.Empty<NotificationSubscription>();
            return _subscriptions[tourId];
        }

        public void RemoveSubscription(string tourId, NotificationSubscription sub)
        {
            if (!_subscriptions.ContainsKey(tourId)) return;
            _subscriptions[tourId].Remove(sub);
        }
    }
}
