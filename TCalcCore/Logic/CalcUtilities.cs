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
        public static (Person p, long debt) GetDebtor(Tour tr)
        {
            var debtor = tr?.Persons
                                    .Select(p => (p, CalcUtilities.GetPayOrReceiveSpendings(tr, p, 0)))
                                    .Where(pwps => pwps.Item2.WillPay)
                                    .Select(pwps => (pwps.p, pwps.Item2.WillPay, pwps.Item2.sp?.Where(s => CalcUtilities.DebtStatusOfSpending(tr, s, pwps.p) != "JustOk")?.Select(s => s.AmountInCurrentCurrency(tr))?.Sum() ?? 0))
                                    .OrderByDescending(hmm => hmm.Item3)
                                    .Select(hmm => (hmm.p, hmm.Item3))
                                    .FirstOrDefault()
                                    ;
            return debtor ?? (new Person(), 0);
        }
    }
}
