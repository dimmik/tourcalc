using System;
using System.Collections.Generic;
using System.Linq;
using Newtonsoft.Json;
using TCalc.Domain;
using TCalc.Logic;

namespace GoldenDump
{
    /// <summary>
    /// Replays the C# settlement one payment at a time and prints what the calculator sees
    /// at each round, so a disagreeing port can be compared against it rather than guessed at.
    /// </summary>
    public static class Trace
    {
        public static void Run(string tourPath)
        {
            var tour = JsonConvert.DeserializeObject<Tour>(System.IO.File.ReadAllText(tourPath));

            // Same two steps SuggestFinalPayments takes, but stopping to look after each one.
            var calc = new TourCalculator(tour);
            var t = calc.SuggestFinalPayments();

            var planned = t.Spendings.Where(s => s.Planned).ToList();
            Console.WriteLine($"{planned.Count} planned payments\n");

            // Rebuild the state round by round: start from the tour without planned spendings,
            // then add them back one at a time and print the balances the loop would see.
            var work = JsonConvert.DeserializeObject<Tour>(JsonConvert.SerializeObject(tour));
            work.Spendings = work.Spendings.Where(s => !s.Planned).ToList();

            for (int i = 0; i <= planned.Count; i++)
            {
                var probe = JsonConvert.DeserializeObject<Tour>(JsonConvert.SerializeObject(work));
                var state = new TourCalculator(probe).Calculate(includePlanned: true);

                var creditors = state.Persons.Where(p => p.Debt() < 0).OrderBy(p => p.Debt()).ToArray();
                var debtors = state.Persons.Where(p => p.Debt() > 0).OrderBy(p => -p.Debt()).ToArray();

                Console.WriteLine($"--- round {i} ---");
                Console.WriteLine("  creditors: " + string.Join(", ",
                    creditors.Select(p => $"{p.Name}({p.Debt()})")));
                Console.WriteLine("  debtors:   " + string.Join(", ",
                    debtors.Select(p => $"{p.Name}({p.Debt()})")));

                if (i < planned.Count)
                {
                    var next = planned[i];
                    Console.WriteLine($"  => adds {next.Description} {next.AmountInCents}");
                    work.Spendings.Add(next);
                }
            }
        }
    }
}
