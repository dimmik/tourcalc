using System;
using System.Collections.Generic;
using System.Text;
using TCBlazor.Client.SharedCode;

namespace TCalcCore.Storage
{
    public interface ISubscriptionStorage
    {
        void AddSubscription(string tourId, NotificationSubscription sub);
        void RemoveSubscription(string tourId, NotificationSubscription sub);
        bool CheckSubscription(string tourId, NotificationSubscription sub);
        IEnumerable<NotificationSubscription> GetSubscriptions(string tourId);
    }
}
