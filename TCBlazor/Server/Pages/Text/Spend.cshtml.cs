using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using Microsoft.AspNetCore.Mvc;
using TCalc.Domain;
using TCalc.Storage;
using Company.TCBlazor;

namespace TCBlazor.Server.Pages.Text
{
    public class SpendModel : TourTextPageModel
    {
        public SpendModel(ITcConfiguration configuration, ITourStorage tourStorage)
            : base(configuration, tourStorage) { }

        public string Query { get; private set; } = "";
        public List<Spending> Shown { get; private set; } = new List<Spending>();
        public int TotalCount { get; private set; }
        public long ShownTotal { get; private set; }

        public async Task<IActionResult> OnGet(string id, [FromQuery] string q)
        {
            var bad = await LoadOr(id);
            if (bad != null) return bad;

            Query = q ?? "";
            var all = RealSpendings().ToList();
            TotalCount = all.Count;

            IEnumerable<Spending> res = all;
            var needle = Query.Trim();
            if (needle.Length > 0)
            {
                res = res.Where(s =>
                    (s.Description ?? "").Contains(needle, StringComparison.OrdinalIgnoreCase)
                    || (s.Type ?? "").Contains(needle, StringComparison.OrdinalIgnoreCase)
                    || PersonName(Tour, s.FromGuid).Contains(needle, StringComparison.OrdinalIgnoreCase));
            }

            Shown = res.OrderByDescending(s => s.SpendingDate).ToList();
            ShownTotal = Shown.Where(CountsAsSpending).Sum(s => s.AmountInCurrentCurrency(Tour));
            return Page();
        }

        /// <summary>
        /// Who an expense was for, in whatever room a table cell has. Spelling out eight
        /// names broke the table: at eighty columns lynx gave up on laying the row out and
        /// spilled it across two lines, taking the alignment of the whole list with it.
        /// A short list still gets named; a long one becomes a count.
        /// </summary>
        public string ForWhomShort(Spending s)
        {
            if (s.ToAll) return "everyone";
            var names = (s.ToGuid ?? new List<string>())
                .Select(g => PersonName(Tour, g))
                .OrderBy(n => n)
                .ToList();
            if (names.Count == 0) return "nobody";
            var joined = string.Join(", ", names);
            return joined.Length <= 12 ? joined : $"{names.Count} people";
        }
    }
}
