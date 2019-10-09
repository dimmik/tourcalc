using System;
using System.Collections.Generic;
using System.Text;
using TCalc.Domain;
using System.Linq;

namespace TCalc.Logic
{
    public class TourCalculator
    {
        private readonly Tour CurrentTour;
        private readonly long TotalWeight;
        public TourCalculator(Tour tour)
        {
            // clone
            CurrentTour = Newtonsoft.Json.JsonConvert.DeserializeObject<Tour>(Newtonsoft.Json.JsonConvert.SerializeObject(tour));
            TotalWeight = 0;
            foreach (Person person in CurrentTour.Persons) TotalWeight += person.Weight;
        }



        private long PersonSpent(Person person, bool includePlanned = false)
        {
            long res = 0;
            person.SpentSendingInfo.Clear();
            foreach (Spending spending in CurrentTour.Spendings.Where(s => includePlanned || !s.Planned))
            {
                if (spending.FromGuid == person.GUID)
                {
                    var amount = spending.AmountInCents;
                    person.SpentSendingInfo.Add(new SpendingInfo()
                    {
                        TotalSpendingAmountInCents = amount,
                        ReceivedAmountInCents = -1,
                        From = person.Name,
                        IsSpendingToAll = spending.ToAll,
                        SpendingDescription = string.IsNullOrWhiteSpace(spending.Description) ? "no description" : spending.Description,
                        ToNames = spending.ToAll 
                            ? new string[] { } 
                            : spending.ToGuid
                                .Select(guid => CurrentTour.Persons.FirstOrDefault(p => p.GUID == guid)?.Name ?? "n/a")
                                .ToArray()
                    });
                    res += amount;
                }
            }
            return res;
        }

        private long PersonReceived(Person person, bool includePlanned = false)
        {
            person.ReceivedSendingInfo.Clear();
            double res = 0;
            foreach (Spending spending in CurrentTour.Spendings.Where(s => includePlanned || !s.Planned))
            {
                var amount = 0d;
                bool isApplicable = false;
                if (spending.ToAll)
                {
                    amount += spending.Weighted ? spending.AmountInCents * person.Weight * 1.0 / TotalWeight : spending.AmountInCents * 1.0;
                    isApplicable = true;
                } else
                {
                    if (spending.ToGuid.Contains(person.GUID))
                    {
                        amount += spending.AmountInCents / spending.ToGuid.Count * 1.0;
                        isApplicable = true;
                    }
                }
                if (isApplicable)
                {
                    res += amount;
                    person.ReceivedSendingInfo.Add(new SpendingInfo()
                    {
                        ReceivedAmountInCents = (long)Math.Round(amount),
                        TotalSpendingAmountInCents = spending.AmountInCents,
                        From = CurrentTour.Persons.FirstOrDefault(p => p.GUID == spending.FromGuid)?.Name ?? "n/a",
                        IsSpendingToAll = spending.ToAll,
                        SpendingDescription = string.IsNullOrWhiteSpace(spending.Description) ? "no description" : spending.Description,
                        ToNames = spending.ToAll
                            ? new string[] { }
                            : spending.ToGuid
                                .Select(guid => CurrentTour.Persons.FirstOrDefault(p => p.GUID == guid)?.Name ?? "n/a")
                                .ToArray()
                    });
                }
            }
            return (long)Math.Round(res);
        }
        public Tour Calculate(bool includePlanned = false)
        {
            if (!CurrentTour.Persons.Any()) return CurrentTour;
            foreach (var p in CurrentTour.Persons)
            {
                p.ReceivedInCents = PersonReceived(p, includePlanned);
                p.SpentInCents = PersonSpent(p, includePlanned);
            }
            DealWithRoundErrors();
            return CurrentTour;
        }

        private void DealWithRoundErrors()
        {
            // deal with round
            var creditors = CurrentTour.Persons.Where(p => p.Debt() < 0).OrderBy(p => p.Debt());
            var debtors = CurrentTour.Persons.Where(p => p.Debt() > 0).OrderBy(p => -p.Debt());
            if (!creditors.Any() && !debtors.Any()) return;
            var sumCredit = -creditors.Sum(c => c.Debt());
            var sumDebit = debtors.Sum(c => c.Debt());
            var diff = sumCredit - sumDebit;
            var highestCredit = - (creditors.FirstOrDefault()?.Debt() ?? 0);
            var highestDebit = (debtors.FirstOrDefault()?.Debt() ?? 0);
            if (highestCredit > highestDebit)
            {
                // get creditor and subtract diff
                var p = creditors.First();
                p.ReceivedInCents += diff;
                p.ReceivedSendingInfo.Add(new SpendingInfo()
                {
                    From = p.GUID,
                    SpendingDescription = $"Rounding Error - add {-diff} to received of highest creditor",
                    ReceivedAmountInCents = diff,
                    IsSpendingToAll = false,
                    ToNames = new[] { "System" },
                    TotalSpendingAmountInCents = diff
                });
            } else
            {
                // get debitor and add diff
                var p = debtors.First();
                p.ReceivedInCents += diff;
                p.ReceivedSendingInfo.Add(new SpendingInfo()
                {
                    From = p.GUID,
                    SpendingDescription = $"Rounding Error - add {diff} to received of highest debtor",
                    ReceivedAmountInCents = diff,
                    IsSpendingToAll = false,
                    ToNames = new[] { "System" },
                    TotalSpendingAmountInCents = diff
                });

            }

        }

        public Tour SuggestFinalPayments()
        {
            Calculate(includePlanned: true);
            // find ones who owes min (will receive max)
            var creditors = CurrentTour.Persons.Where(p => p.Debt() < 0).OrderBy(p => p.Debt()).ToArray();
            var debtors   = CurrentTour.Persons.Where(p => p.Debt() > 0).OrderBy(p => -p.Debt()).ToArray();
            int maxIterations = 500;
            int i = 0;
            while (creditors.Any() && debtors.Any())
            {                
                // get first creditor (highest credit)
                var credit = -creditors.First().Debt();
                // find first debtor (highest debt)
                var highestDebt = debtors.First().Debt();
                CurrentTour.Spendings.Add(new Spending()
                {
                    FromGuid = debtors.First().GUID,
                    Planned = true,
                    ToGuid = new[] { creditors.First().GUID }.ToList(),
                    ToAll = false,
                    AmountInCents = credit > highestDebt ? highestDebt : credit,
                    Description = $"'{debtors.First()?.Name ?? "n/a"}' -> '{creditors.First()?.Name ?? "n/a"}'",
                    GUID = Guid.NewGuid().ToString()
                });
                // add spending
                Calculate(includePlanned: true);
                creditors = CurrentTour.Persons.Where(p => p.Debt() < 0).OrderBy(p => p.Debt()).ToArray();
                debtors = CurrentTour.Persons.Where(p => p.Debt() > 0).OrderBy(p => -p.Debt()).ToArray();
                i++;
                if (i > maxIterations) throw new Exception("Cannot calculate tour suggestions");
            }
            Calculate(includePlanned: false);
            return CurrentTour;
        }
    }
   
}
