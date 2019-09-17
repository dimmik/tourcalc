using System;
using System.Collections.Generic;
using System.Text;

namespace TCalc.Domain
{
    public class Spending : AbstractItem
    {
        public string FromGuid { get; set; }
        public List<string> ToGuid { get; set; } = new List<string>();
        public long AmountInCents { get; set; }
        public bool Weighted { get; set; } = true;
        public bool ToAll { get; set; } = false;
        public string Description { get; set; } = "";

        public override string ToString()
        {
            return $"{AmountInCents} toAll: {ToAll}";
        }
    }
}
