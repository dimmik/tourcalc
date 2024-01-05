using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using TCalc.Domain;

namespace TCalc.Logic
{
    public static class CalcUtilities
    {
        public static (bool WillPay, IEnumerable<Spending> sp) GetPayOrReceiveSpendings(Tour tour, Person person, long minMeaningfulAmount)
        {
            IEnumerable<Spending> sp;
            bool wp;
            var spToPay = tour.Spendings.Where(s => s.Planned && s.FromGuid == person.GUID && s.AmountInCurrentCurrency(tour) > minMeaningfulAmount);
            if (spToPay.Any())
            {
                wp = true;
                sp = spToPay;
            }
            else
            {
                sp = tour.Spendings.Where(s => s.Planned && s.ToGuid.Count == 1 && s.ToGuid[0] == person.GUID && s.AmountInCurrentCurrency(tour) > minMeaningfulAmount);
                wp = false;
            }
            return (wp, sp);

        }
        public static string DebtStatusOfSpending(Tour tour, Spending spending, Person person)
        {
            var pTo = tour?.Persons?.FirstOrDefault(pp => pp.GUID == spending.ToGuid[0]) ?? new Person();
            var pFrom = tour?.Persons?.FirstOrDefault(pp => pp.GUID == spending.FromGuid) ?? new Person();
            if (pTo.GUID == person.ParentId || pFrom.ParentId == person.GUID) return "JustOk"; // payment to parent
            if (pFrom.GUID == person.GUID) return "Bankrupt"; // the person will pay
            return "Pleasure"; // someone, not child, will pay me
        }
    }
}
