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
                   Url == other.Url &&
                   P256dh == other.P256dh &&
                   Auth == other.Auth;
        }

        public override int GetHashCode()
        {
            int hashCode = -1775057863;
            hashCode = hashCode * -1521134295 + EqualityComparer<string>.Default.GetHashCode(Url);
            hashCode = hashCode * -1521134295 + EqualityComparer<string>.Default.GetHashCode(P256dh);
            hashCode = hashCode * -1521134295 + EqualityComparer<string>.Default.GetHashCode(Auth);
            return hashCode;
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
