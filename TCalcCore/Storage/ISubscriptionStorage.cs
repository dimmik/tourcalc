using System;
using System.Collections.Generic;
using System.Text;
using TCBlazor.Client.SharedCode;

namespace TCalcCore.Storage
{
    public interface ISubscriptionStorage
    {
        void AddSubscription(string tourId, NotificationSubscription sub);
        IEnumerable<NotificationSubscription> GetSubscriptions(string tourId);
    }
}
