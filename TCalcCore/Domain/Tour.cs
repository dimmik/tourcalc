using System;
using System.Collections.Generic;
using System.Linq;
using TCalc.Logic;
using TCalcCore.Helpers;

namespace TCalc.Domain
{
    public class Tour : AbstractItem
    {
        public List<Person> Persons { get; set; } = new List<Person>();
        public List<Spending> Spendings { get; set; } = new List<Spending>();
        public string Name { get; set; } = $"Tour of {DateTime.Now.ToLongDateString()}";
        public string Id { get { return GUID; } set { GUID = value; } }
        public string AccessCodeMD5 { get; set; } = "";
        public bool IsVersion { get; set; } = false;
        public DateTime DateVersioned { get; set; } = DateTime.Now;
        public string VersionFor_Id { get; set; } = "";
        public string VersionComment { get; set; } = "";
        public string InternalVersionComment = null;
        public bool IsArchived { get; set; } = false;
        public bool IsFinalizing { get; set; } = false;

        public int Duration { get; set; } = 5;

        public string StateGUID { get; set; } = "";
        public IEnumerable<Currency> Currencies { get; set; } = new Currency[] { Currency.Default };
        public string TourCurrencyId { get; set; } = Currency.Default.Id;
        public Currency Currency { 
            get {
                if (!Currencies.Any())
                {
                    Currencies = new Currency[] { Currency.Default };
                }
                if (!Currencies.Any(c => c.Id == TourCurrencyId))
                {
                    TourCurrencyId = Currencies.First().Id;
                }
                return CheapCopy(Currencies.Where(c => c.Id == TourCurrencyId).First());
            }
            set
            {
                if (TourCurrencyId != value.Id)
                {
                    TourCurrencyId = value.Id;
                    Spendings = Spendings.Where(s => !s.Planned).ToList();
                }
            }
        }

        // Callers may keep and even mutate what they get, so this still hands out a copy -
        // but building it by hand instead of a JSON round-trip. The property is read once per
        // amount conversion, i.e. thousands of times per render on a large tour.
        private static Currency CheapCopy(Currency c)
        {
            return new Currency { Id = c.Id, Name = c.Name, CurrencyRate = c.CurrencyRate };
        }
        public void PrepareForStoring()
        {
            // delete spending lists that might be rather large
            Persons.ForEach(p => { p.ReceivedSendingInfo = new List<SpendingInfo>(); p.SpentSendingInfo = new List<SpendingInfo>(); });
            // remove planned calculations
            //Spendings = Spendings.Where(s => !s.Planned).ToList();
            // check if current calculation is ok
            var calculator = new TourCalculator(this);
            var calculated = calculator.Calculate(includePlanned: true);
            bool isCalculationAlreadyOk = calculated.TotalAbsDebt() == 0;
            if (!isCalculationAlreadyOk)
            {
                Spendings = this.Spendings.Where(s => !s.Planned).ToList();
                calculator = new TourCalculator(this);
                var suggested = calculator.SuggestFinalPayments();
                Spendings = Spendings.Where(s => !s.Planned).Concat(suggested.Spendings.Where(s => s.Planned)).ToList();
            }
        }
        public Tour Clone()
        {
            return Newtonsoft.Json.JsonConvert.DeserializeObject<Tour>(Newtonsoft.Json.JsonConvert.SerializeObject(this));
        }
    }
    public static class TourHelper
    {
        public static long TotalAbsDebt(this Tour tour)
        {
            if (tour.Persons.Count <= 0) return 0;
            return tour.Persons.Aggregate(0L, (prev, p) => prev + Math.Abs(p.ReceivedInCents - p.SpentInCents));
        }
        public static bool IsMultiCurrency(this Tour tour)
        {
            if (tour == null) return false;
            return !(tour.Currencies == null || !tour.Currencies.Any() || tour.Currencies.Count() <= 1);
        }
        public static string CurrencyNameEmptyIfSingleCurrency(this Tour t, string def = "")
        {
            if (!t.IsMultiCurrency()) return def;
            return t?.Currency?.Name ?? def;
        }
    }
}
