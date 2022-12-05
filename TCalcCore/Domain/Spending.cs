using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using TCalcCore.Domain;

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

        public override string ToString()
        {
            return $"{AmountInCents} toAll: {ToAll} planned: {Planned}";
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
            if (p.ToGuid.Count != 1 || pp.ToGuid.Count != 1) return false;
            if (p.FromGuid != pp.FromGuid) return false;
            if (pp.ToGuid.First() != p.ToGuid.First()) return false;
            return true;
        }
        public static long AmountInCurrentCurrency(this Spending sp, Tour tour)
        {
            var rates = tour.Currencies;
            var currentCurr = rates.Where(c => c == tour.CurrentCurrency).FirstOrDefault() ?? new Currency();
            var spendingCurrency = rates.Where(c => c == sp.Currency).FirstOrDefault() ?? currentCurr; // if not found in tour - just same as default

            if (spendingCurrency == currentCurr) return sp.AmountInCents;

            var amount = sp.AmountInCents;
            long coeff = spendingCurrency.CurrencyRate * 1000000 / currentCurr.CurrencyRate;
            long result = amount * coeff / 1000000;//(long)(amount * spendingCurrency.CurrencyRate * 1.0 /  currentCurr.CurrencyRate);
            // TODO for some reason works extremely slow. Return original amount
            return sp.AmountInCents;
//            return result;
        }
    }
}
