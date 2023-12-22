using System;
using System.Collections.Generic;
using System.Text;

namespace TCalc.Domain
{
    public class Person : AbstractItem
    {
        public string Name { get; set; } = $"New Person";
        public int Weight { get; set; } = 100;
        public long SpentInCents { get; set; } = 0;
        public long ReceivedInCents { get; set; } = 0;

        public List<SpendingInfo> SpentSendingInfo { get; set; } = new List<SpendingInfo>();
        public List<SpendingInfo> ReceivedSendingInfo { get; set; } = new List<SpendingInfo>();

        public string ParentId { get; set; }

        public string GroupId { get; set; } = Guid.NewGuid().ToString("N");

        public string FamilyId { get; set; } = string.Empty;

        public override string ToString()
        {
            return $"{Name} - {GUID} -- {Debt()}";
        }
        public long Debt()
        {
            return ReceivedInCents - SpentInCents;
        }
    }
    public class SpendingInfo
    {
        public SpendingInfo()
        {
        }

        public string From { get; set; }
        public long ReceivedAmountInCents { get; set; }
        public Currency Currency { get; set; }
        public long OriginalReceivedAmountInCents { get; set; }
        public long TotalSpendingAmountInCents { get; set; }
        public long OriginalTotalSpendingAmountInCents { get; set; }
        public string SpendingDescription { get; set; }
        public bool IsSpendingToAll { get; set; }
        public string[] ToNames { get; set; }
        public string Type { get; set; } = "";
    }
}
