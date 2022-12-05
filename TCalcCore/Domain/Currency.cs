using System;
using System.Collections.Generic;
using System.Text;

namespace TCalc.Domain
{
    public class Currency : IEquatable<Currency>
    {
        public string Name { get; set; } = "RUB";
        public int CurrencyRate { get; set; } = 100;
        public static Currency Default => new Currency();

        public override bool Equals(object obj)
        {
            return Equals(obj as Currency);
        }

        public bool Equals(Currency other)
        {
            return !(other is null) &&
                   Name == other.Name;
        }

        public override int GetHashCode()
        {
            return 539060726 + EqualityComparer<string>.Default.GetHashCode(Name);
        }

        public static bool operator ==(Currency left, Currency right)
        {
            return EqualityComparer<Currency>.Default.Equals(left, right);
        }

        public static bool operator !=(Currency left, Currency right)
        {
            return !(left == right);
        }

        public override string ToString()
        {
            return $"{Name} : {CurrencyRate}";
        }
    }
}
