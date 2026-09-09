using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Newtonsoft.Json;
using TCalc.Domain;
using TCalc.Logic;

namespace GoldenDump
{
    /// <summary>
    /// Reads tours written by the Rust port and checks that the C# calculator still makes
    /// the same numbers of them. This is the only check that proves the two really share a
    /// storage format: everything else only proves each side is consistent with itself.
    /// </summary>
    public static class Verify
    {
        public static int Run(string rewrittenDir, string expectedDir)
        {
            var failures = 0;
            foreach (var path in Directory.GetFiles(rewrittenDir, "*.tour.json").OrderBy(p => p))
            {
                var name = Path.GetFileName(path).Replace(".tour.json", "");
                var expectedPath = Path.Combine(expectedDir, $"{name}.expected.json");
                if (!File.Exists(expectedPath)) continue;

                var tour = JsonConvert.DeserializeObject<Tour>(File.ReadAllText(path));
                var expected = JsonConvert.DeserializeObject<dynamic>(File.ReadAllText(expectedPath));

                var calculated = new TourCalculator(Clone(tour)).Calculate(includePlanned: false);
                var suggested = new TourCalculator(Clone(tour)).SuggestFinalPayments();

                var problems = new List<string>();

                foreach (var want in expected.persons)
                {
                    string guid = (string)want.guid;
                    var got = calculated.Persons.FirstOrDefault(p => p.GUID == guid);
                    if (got == null) { problems.Add($"person {want.name} missing"); continue; }
                    if (got.SpentInCents != (long)want.spent)
                        problems.Add($"{want.name} spent {got.SpentInCents} want {want.spent}");
                    if (got.ReceivedInCents != (long)want.received)
                        problems.Add($"{want.name} received {got.ReceivedInCents} want {want.received}");
                }

                var gotTransfers = suggested.Spendings.Where(s => s.Planned)
                    .Select(s => $"{s.Description}|{s.AmountInCents}").ToArray();
                var wantTransfers = ((IEnumerable<dynamic>)expected.transfers)
                    .Select(t => $"{(string)t.description}|{(long)t.amount}").ToArray();
                if (!gotTransfers.SequenceEqual(wantTransfers))
                {
                    problems.Add("transfers differ:");
                    problems.Add("  got:  " + string.Join(", ", gotTransfers));
                    problems.Add("  want: " + string.Join(", ", wantTransfers));
                }

                if (problems.Count == 0)
                {
                    Console.WriteLine($"  ok    {name}");
                }
                else
                {
                    failures++;
                    Console.WriteLine($"  FAIL  {name}");
                    foreach (var p in problems) Console.WriteLine($"        {p}");
                }
            }
            Console.WriteLine(failures == 0
                ? "\nC# reads what Rust wrote, and gets the same answers."
                : $"\n{failures} tour(s) disagree.");
            return failures;
        }

        static Tour Clone(Tour t) => JsonConvert.DeserializeObject<Tour>(JsonConvert.SerializeObject(t));
    }
}
