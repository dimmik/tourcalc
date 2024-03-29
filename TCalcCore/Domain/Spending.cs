﻿using System;
using System.Collections.Generic;
using System.Linq;

namespace TCalc.Domain
{
    public class Spending : AbstractItem
    {
        public string FromGuid { get; set; }
        public List<string> ToGuid { get; set; } = new List<string>();
        public long AmountInCents { get; set; }
        public bool Weighted { get; set; } = true;
        public bool IsPartialWeighted { get; set; } = false;
        public bool ToAll { get; set; } = false;
        public bool Planned { get; set; } = false;
        public string Description { get; set; } = "";
        public string Type { get; set; } = "Common";
        public Currency Currency { get; set; } = new Currency();
        public IEnumerable<Currency> Currencies { get; set; } = Enumerable.Empty<Currency>();
        public bool IsDryRun { get; set; } = false;
        public bool IncludeDryRunInCalc { get; set; } = false;
        public string Color { get; set; } = "";

        private DateTime? _spendingDate = null;
        public DateTime SpendingDate
        {
            get
            {
                if (_spendingDate == null)
                {
                    return DateCreated;
                }
                return _spendingDate.Value;
            }
            set
            {
                _spendingDate = value;
            }
        }

        public override string ToString()
        {
            return $"{SpendingDate:yyyy-MM-dd} {AmountInCents} toAll: {ToAll} planned: {Planned}";
        }
    }
    public static class SpendingHelpers
    {
        public static bool IsReturningSameVolumeAs(this Spending p, Spending pp)
        {
            if (p.AmountInCents != pp.AmountInCents) return false;
            if (p.ToGuid.Count != 1 || pp.ToGuid.Count != 1) return false;
            if (p.FromGuid != pp.ToGuid.First()) return false;
            if (pp.FromGuid != p.ToGuid.First()) return false;
            return true;
        }
        public static bool IsAlmostTheSame(this Spending p, Spending pp)
        {
            if (p.AmountInCents != pp.AmountInCents) return false;
            if (p.Currency != pp.Currency) return false;
            if (p.ToGuid.Count != 1 || pp.ToGuid.Count != 1) return false;
            if (p.FromGuid != pp.FromGuid) return false;
            if (pp.ToGuid.First() != p.ToGuid.First()) return false;
            return true;
        }
        public static long AmountInCurrentCurrency(this Spending sp, Tour tour)
        {
            if (tour == null) return sp.AmountInCents;
            if (!tour.IsMultiCurrency()) // only one currency or no currencies
            {
                return sp.AmountInCents;
            }

            var tourRates = tour.Currencies;
            var rates = sp.Currencies.Any() ? sp.Currencies : tourRates;//tour.Currencies;
//            var rates = tour.Currencies;
            var currentCurr = rates.FirstOrDefault(c => c == tour.Currency) ?? 
                    tourRates.FirstOrDefault(c => c == tour.Currency) ?? new Currency();
            var spendingCurrency = rates.FirstOrDefault(c => c == sp.Currency) ?? currentCurr; // if not found in tour - just same as default

            if (spendingCurrency == currentCurr) return sp.AmountInCents;

            var amount = sp.AmountInCents;
            long result = (long)Math.Round(amount * spendingCurrency.CurrencyRate * 1.0 / currentCurr.CurrencyRate);

            return result;
        }
    }
}
