using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using Microsoft.AspNetCore.Mvc;
using TCalc.Domain;
using TCalc.Logic;
using TCalc.Storage;
using Company.TCBlazor;

namespace TCBlazor.Server.Pages.Text
{
    public class StatsModel : TourTextPageModel
    {
        public StatsModel(ITcConfiguration configuration, ITourStorage tourStorage)
            : base(configuration, tourStorage) { }

        public record Group(string Key, int Count, long Sum);

        public List<Group> ByCategory { get; private set; } = new List<Group>();
        public List<Group> ByPayer { get; private set; } = new List<Group>();
        public int Days { get; private set; } = 1;
        public long PerPerson { get; private set; }
        public long PerPersonPerDay => PerPerson / (Days == 0 ? 1 : Days);

        public async Task<IActionResult> OnGet(string id)
        {
            var bad = await LoadOr(id);
            if (bad != null) return bad;

            var counted = Tour.Spendings.Where(CountsAsSpending).ToList();
            Days = Tour.Duration > 0 ? Tour.Duration : 1;

            var totalWeight = Tour.Persons.Sum(p => p.Weight);
            if (totalWeight == 0) totalWeight = 100;
            // a "full share" person: a parent if anybody is paid for, otherwise the lightest
            var oneShare = Tour.Persons.Any(p => !string.IsNullOrWhiteSpace(p.ParentId))
                ? (Tour.Persons.FirstOrDefault(p => p.GUID == Tour.Persons.First(x => !string.IsNullOrWhiteSpace(x.ParentId)).ParentId)?.Weight ?? 100)
                : (Tour.Persons.Any(p => p.Weight > 0) ? Tour.Persons.Where(p => p.Weight > 0).Min(p => p.Weight) : 100);
            PerPerson = TotalSpent * oneShare / totalWeight;

            ByCategory = counted
                .GroupBy(s => string.IsNullOrWhiteSpace(s.Type) ? "—" : s.Type)
                .Select(g => new Group(g.Key, g.Count(), g.Sum(x => x.AmountInCurrentCurrency(Tour))))
                .OrderByDescending(g => g.Sum).ToList();

            ByPayer = counted
                .GroupBy(s => PersonName(Tour, s.FromGuid))
                .Select(g => new Group(g.Key, g.Count(), g.Sum(x => x.AmountInCurrentCurrency(Tour))))
                .OrderByDescending(g => g.Sum).ToList();

            return Page();
        }

        /// <summary>Rounding a small slice to "0%" reads as "nothing", which it is not.</summary>
        public string Percent(long part)
        {
            if (TotalSpent == 0) return "0%";
            var p = part * 100.0 / TotalSpent;
            if (p > 0 && p < 1) return "<1%";
            return $"{Math.Round(p)}%";
        }
    }
}
