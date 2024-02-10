using System;
using System.Collections.Generic;

namespace TCBlazor.Client.SharedCode
{
    public class NotificationSubscription : IEquatable<NotificationSubscription>
    { 
        public string Url { get; set; }

        public string P256dh { get; set; }

        public string Auth { get; set; }

        public override bool Equals(object obj)
        {
            return Equals(obj as NotificationSubscription);
        }

        public bool Equals(NotificationSubscription other)
        {
            return !(other is null) &&
                   Url == other.Url;
        }

        public override int GetHashCode()
        {
            return -1915121810 + EqualityComparer<string>.Default.GetHashCode(Url);
        }

        public static bool operator ==(NotificationSubscription left, NotificationSubscription right)
        {
            return EqualityComparer<NotificationSubscription>.Default.Equals(left, right);
        }

        public static bool operator !=(NotificationSubscription left, NotificationSubscription right)
        {
            return !(left == right);
        }
    }
}
