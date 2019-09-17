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
            this.CurrentTour = Newtonsoft.Json.JsonConvert.DeserializeObject<Tour>(Newtonsoft.Json.JsonConvert.SerializeObject(tour));
            TotalWeight = 0;
            foreach (Person person in CurrentTour.Persons) TotalWeight += person.Weight;
        }



        private long PersonSpent(Person person)
        {
            long res = 0;
            foreach (Spending spending in CurrentTour.Spendings)
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

        private long PersonReceived(Person person)
        {
            double res = 0;
            foreach (Spending spending in CurrentTour.Spendings)
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
        public Tour Calculate()
        {
            foreach (var p in CurrentTour.Persons)
            {
                p.ReceivedInCents = PersonReceived(p);
                p.SpentInCents = PersonSpent(p);
            }
            return CurrentTour;
        }

    }
}
