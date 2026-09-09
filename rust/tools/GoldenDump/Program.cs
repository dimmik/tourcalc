using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Newtonsoft.Json;
using TCalc.Domain;
using TCalc.Logic;

// Reads the seed tours, runs the real calculator over each one, and writes both the input
// and the answers next to each other. The Rust tests read exactly these files.
//
//   <id>.tour.json        the tour as stored - the input
//   <id>.expected.json    what TourCalculator makes of it
//
// usage: dotnet run --project rust/tools/GoldenDump -- <tours.json> <output dir>

if (args.Length > 0 && args[0] == "verify")
{
    Environment.ExitCode = GoldenDump.Verify.Run(args[1], args.Length > 2 ? args[2] : "rust/fixtures");
    return;
}

if (args.Length > 0 && args[0] == "trace")
{
    GoldenDump.Trace.Run(args[1]);
    return;
}

var source = args.Length > 0 ? args[0] : "TCBlazor/Server/inmemory-tours.json";
var outDir = args.Length > 1 ? args[1] : "rust/fixtures";

Directory.CreateDirectory(outDir);

var json = File.ReadAllText(source);
var tours = JsonConvert.DeserializeObject<List<Tour>>(json) ?? new List<Tour>();

var written = 0;
foreach (var tour in tours)
{
    if (tour == null || string.IsNullOrWhiteSpace(tour.Id)) continue;

    // Each calculator gets its own copy of the tour: SuggestFinalPayments appends planned
    // spendings, and running it first would change what Calculate then sees.
    var calculated = new TourCalculator(Clone(tour)).Calculate(includePlanned: false);
    var suggested = new TourCalculator(Clone(tour)).SuggestFinalPayments();

    var expected = new
    {
        tourId = tour.Id,
        name = tour.Name,
        currency = new { id = tour.Currency.Id, name = tour.Currency.Name, rate = tour.Currency.CurrencyRate },
        isMultiCurrency = tour.IsMultiCurrency(),
        persons = calculated.Persons.Select(p => new
        {
            guid = p.GUID,
            name = p.Name,
            weight = p.Weight,
            spent = p.SpentInCents,
            received = p.ReceivedInCents,
            debt = p.Debt(),
        }).ToArray(),
        // What the app lists as "who pays whom": every planned spending after the suggestion
        // pass, in the order the calculator produced them.
        transfers = suggested.Spendings.Where(s => s.Planned).Select(s => new
        {
            from = s.FromGuid,
            fromName = NameOf(suggested, s.FromGuid),
            to = s.ToGuid.FirstOrDefault(),
            toName = NameOf(suggested, s.ToGuid.FirstOrDefault()),
            amount = s.AmountInCents,
            description = s.Description,
        }).ToArray(),
    };

    var name = Sanitise(tour.Id);
    File.WriteAllText(Path.Combine(outDir, $"{name}.tour.json"),
        JsonConvert.SerializeObject(tour, Formatting.Indented));
    File.WriteAllText(Path.Combine(outDir, $"{name}.expected.json"),
        JsonConvert.SerializeObject(expected, Formatting.Indented));

    Console.WriteLine($"{name,-16} {calculated.Persons.Count,3} people {tour.Spendings.Count,4} spendings " +
                      $"{expected.transfers.Length,3} transfers  currency {tour.Currency.Name}");
    written++;
}

Console.WriteLine($"\n{written} tours written to {outDir}");

static Tour Clone(Tour t) =>
    JsonConvert.DeserializeObject<Tour>(JsonConvert.SerializeObject(t));

static string NameOf(Tour t, string guid) =>
    t.Persons.FirstOrDefault(p => p.GUID == guid)?.Name ?? "n/a";

static string Sanitise(string id) =>
    string.Concat(id.Select(c => char.IsLetterOrDigit(c) || c == '-' ? c : '_'));
