using System;
using System.Collections.Generic;
using System.Text;

namespace TCalc.Domain
{
    public class Currency : IEquatable<Currency>, IComparable<Currency>
    {
        private string _id = null;
        private readonly object _idLock = new object();
        public string Id { 
            get { 
                if (_id == null) _id = Name; // keep current contract
                return _id;
            }
            set { _id = value; }
        }
        public string Name { get; set; } = "EUR";
        public int CurrencyRate { get; set; } = 100;
        public static Currency Default => new Currency();

        public override bool Equals(object obj)
        {
            return Equals(obj as Currency);
        }

        public bool Equals(Currency other)
        {
            return !(other is null) &&
                   Id == other.Id;
        }

        public override int GetHashCode()
        {
            return 539060726 + EqualityComparer<string>.Default.GetHashCode(Id);
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

        public int CompareTo(Currency other)
        {
            return Name.CompareTo(other.Name);
        }
    }
}
