using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Text;
using TCalc.Logic;

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
        public string StateGUID { get; set; } = "";
        public IEnumerable<Currency> Currencies { get; set; } = new Currency[] { Currency.Default };
        public Currency CurrentCurrency { get; set; } = Currency.Default;
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
    static class TourHelper
    {
        public static long TotalAbsDebt(this Tour tour)
        {
            if (tour.Persons.Count <= 0) return 0;
            return tour.Persons.Aggregate(0L, (prev, p) => prev + Math.Abs(p.ReceivedInCents - p.SpentInCents));
        }
    }
}
