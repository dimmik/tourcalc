using System;
using System.Collections.Generic;
using System.Text;
using TCalc.Domain;
using System.Linq;
using System.Data.SqlTypes;
using Algorithms.MD5;

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
                        Type = spending.Type,
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
            const long _magnitude_ = 10000000;
            foreach (Spending spending in CurrentTour.Spendings.Where(s => includePlanned || !s.Planned))
            {
                long amount = 0L;
                bool isApplicable = false;
                if (spending.ToAll)
                {
                    amount += spending.AmountInCents * person.Weight * _magnitude_ / TotalWeight;
                    isApplicable = true;
                } else
                {
                    if (spending.ToGuid.Contains(person.GUID))
                    {
                        if (spending.IsPartialWeighted)
                        {
                            int spendingWeight = 0;
                            foreach (var guid in spending.ToGuid)
                            {
                                var pWeight = CurrentTour.Persons.Where(p => p.GUID == guid).FirstOrDefault()?.Weight ?? 0;
                                spendingWeight += pWeight;
                            }
                            if (spendingWeight == 0) spendingWeight = 1;
                            amount += spending.AmountInCents * person.Weight * _magnitude_ / spendingWeight;
                        } else
                        {
                            amount += spending.AmountInCents * _magnitude_ / spending.ToGuid.Count;
                        }
                        isApplicable = true;
                    }
                }
                if (isApplicable)
                {
                    res += amount;
                    person.ReceivedSendingInfo.Add(new SpendingInfo()
                    {
                        ReceivedAmountInCents = (amount + (_magnitude_ / 2)) / _magnitude_,
                        
                        TotalSpendingAmountInCents = spending.AmountInCents,
                        From = CurrentTour.Persons.FirstOrDefault(p => p.GUID == spending.FromGuid)?.Name ?? "n/a",
                        IsSpendingToAll = spending.ToAll,
                        SpendingDescription = string.IsNullOrWhiteSpace(spending.Description) ? "no description" : spending.Description,
                        Type = spending.Type,
                        ToNames = spending.ToAll
                            ? new string[] { }
                            : spending.ToGuid
                                .Select(guid => CurrentTour.Persons.FirstOrDefault(p => p.GUID == guid)?.Name ?? "n/a")
                                .ToArray()
                    });
                }
            }
            var sum = person.ReceivedSendingInfo.Select(rsi => rsi.ReceivedAmountInCents).Sum();
            long ress = (long)((res + (_magnitude_ / 2)) / _magnitude_);
            var diff = ress - sum;
            if (diff != 0)
            {
                person.ReceivedSendingInfo.Add(new SpendingInfo()
                {
                    From = "System",
                    SpendingDescription = $"Distribution Rounding Error: {diff}",
                    ReceivedAmountInCents = diff,
                    IsSpendingToAll = false,
                    ToNames = new[] { person.Name },
                    TotalSpendingAmountInCents = diff
                });
                sum = person.ReceivedSendingInfo.Select(rsi => rsi.ReceivedAmountInCents).Sum();
            }
            return sum;
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
            long diff = sumCredit - sumDebit;
            if (diff == 0) return;
            // find one that will go to 0 with this diff
            var zeroedPerson = CurrentTour.Persons.Where(p => p.Debt() + diff == 0);
            if (zeroedPerson.Any())
            {
                // get zeroed and add diff
                var p = zeroedPerson.First();
                p.ReceivedInCents += diff;
                p.ReceivedSendingInfo.Add(new SpendingInfo()
                {
                    From = "System",
                    SpendingDescription = $"Rounding Error - add {diff} to received of one who get to zero",
                    ReceivedAmountInCents = diff,
                    IsSpendingToAll = false,
                    ToNames = new[] { p.Name },
                    TotalSpendingAmountInCents = diff
                });

            }
            else
            {
                var highestCredit = -(creditors.FirstOrDefault()?.Debt() ?? 0);
                var highestDebit = (debtors.FirstOrDefault()?.Debt() ?? 0);
                if (highestCredit > highestDebit)
                {
                    // get creditor and subtract diff
                    var p = creditors.First();
                    p.ReceivedInCents += diff;
                    p.ReceivedSendingInfo.Add(new SpendingInfo()
                    {
                        From = "System",
                        SpendingDescription = $"Rounding Error - add {diff} to received of highest creditor",
                        ReceivedAmountInCents = diff,
                        IsSpendingToAll = false,
                        ToNames = new[] { p.Name },
                        TotalSpendingAmountInCents = diff
                    });
                }
                else
                {
                    // get debitor and add diff
                    var p = debtors.First();
                    p.ReceivedInCents += diff;
                    p.ReceivedSendingInfo.Add(new SpendingInfo()
                    {
                        From = "System",
                        SpendingDescription = $"Rounding Error - add {diff} to received of highest debtor",
                        ReceivedAmountInCents = diff,
                        IsSpendingToAll = false,
                        ToNames = new[] { p.Name },
                        TotalSpendingAmountInCents = diff
                    });

                }
            }
        }

        public Tour SuggestFinalPayments()
        {
            SuggestFamilies();
            SuggestCrossPayment();
            // calculate without planned to show on UI
            Calculate(includePlanned: false);
            return CurrentTour;
        }
        private static string _PGS_ = "r";
        private void SuggestCrossPayment()
        {
            // find ones who owes min (will receive max)
            Calculate(includePlanned: true);
            var creditors = Creditors();
            var debtors = Debtors();
            int maxIterations = 500;
            int i = 0;
            while (creditors.Any() && debtors.Any())
            {
                // get first creditor (highest credit)
                var creditor = (creditors.Where(c => c.GroupId.StartsWith(_PGS_)).Concat(creditors.Where(c => !c.GroupId.StartsWith(_PGS_)))).First();
                var cGroupId = creditor.GroupId;

                var debtor = (debtors.Where(d => d.GroupId != cGroupId).Concat(debtors.Where(d => d.GroupId == cGroupId))).First();

                var credit = -creditor.Debt();
                // find first debtor (highest debt)
                var highestDebt = debtor.Debt();
                // spending.GUID = $"{spending.FromGuid}{spending.Description}{spending.AmountInCents}{parent.GUID}".CreateMD5();
                var spending = new Spending()
                {
                    FromGuid = debtor.GUID,
                    Planned = true,
                    ToGuid = new[] { creditor.GUID }.ToList(),
                    ToAll = false,
                    AmountInCents = credit > highestDebt ? highestDebt : credit,
                    Description = $"X '{debtor?.Name ?? "n/a"}' -> '{creditor?.Name ?? "n/a"}'",
                    Type = ""
                };
                spending.GUID = $"{spending.FromGuid}{spending.Description}{spending.AmountInCents}{creditor.GUID}".CreateMD5();
                CurrentTour.Spendings.Add(spending);

                Calculate(includePlanned: true);
                creditors = Creditors();
                debtors = Debtors();
                i++;
                if (i > maxIterations) throw new Exception($"Cannot calculate tour suggestions. Max of {maxIterations} iterations excceeded");
            }
            //RemoveRedundant();

        }
        private Person[] Creditors() => CurrentTour.Persons.Where(p => p.Debt() < 0).OrderBy(p => p.Debt()).ToArray();
        private Person[] Debtors() => CurrentTour.Persons.Where(p => p.Debt() > 0).OrderBy(p => -p.Debt()).ToArray();

        private void RemoveRedundant()
        {
            // remove double-sided: a -> B X && B -> a X
            // can appear because of storing the calculations
            var planned = CurrentTour.Spendings.Where(s => s.Planned).ToArray();
            List<Spending> toRemove = new List<Spending>();
            for (int j = 0; j < planned.Length; j++)
            {
                var p = planned[j];
                for (int k = j + 1; k < planned.Length; k++)
                {
                    var pp = planned[k];
                    if (p.IsReturningSameVolumeAs(pp))
                    {
                        CurrentTour.Spendings.Remove(p);
                        CurrentTour.Spendings.Remove(pp);
                    }
                }
            }
        }

        

        private void SuggestFamilies()
        {
            Calculate(includePlanned: true);
            // first, find all guys with parents
            var descedants = CurrentTour.Persons
                .Where(p => !string.IsNullOrWhiteSpace(p.ParentId))
                .Where(p => p.Debt() != 0)
                .Where(
                    p => CurrentTour.Persons.Any(
                        pp => (pp.GUID == p.ParentId)
                     )
                ).ToArray();
            // remove cycles
            descedants = descedants.Where(p => !descedants.Any(d => d.GUID == p.ParentId)).ToArray();
            // nullify all descendants' debt
            foreach (var d in descedants)
            {
                var parent = CurrentTour.Persons.First(p => p.GUID == d.ParentId);
                var debt = d.Debt();
                var spending = new Spending()
                {
                    Planned = true,
                    FromGuid = d.GUID,
                    ToGuid = new[] { parent.GUID }.ToList(),
                    ToAll = false,
                    AmountInCents = debt,
                    Description = $"Family '{d.Name}' -> '{parent.Name}'",
                    Type = ""
                };
                spending.GUID = $"{spending.FromGuid}{spending.Description}{spending.AmountInCents}{parent.GUID}".CreateMD5();
                CurrentTour.Spendings.Add(spending);
            }
        }
    }
    static class SU
    {
        public static string CreateMD5(this string input)
        {
            if (input == null) input = "";
            if (input == null) input = "";
            var md5 = new MD5
            {
                Value = input
            };
            return md5.FingerPrint;
        }
    }
   
}
