using System;
using System.Linq;
using System.Threading.Tasks;
using Microsoft.AspNetCore.Mvc;
using TCalc.Domain;
using TCalc.Logic;
using TCalc.Storage;
using Company.TCBlazor;

namespace TCBlazor.Server.Pages.Text
{
    public class TourModel : TourTextPageModel
    {
        public TourModel(ITcConfiguration configuration, ITourStorage tourStorage)
            : base(configuration, tourStorage) { }

        /// <summary>Set after a payment was recorded, so the page can say so.</summary>
        [TempData]
        public string Recorded { get; set; }

        public async Task<IActionResult> OnGet(string id) => await LoadOr(id) ?? Page();

        /// <summary>
        /// Records that one of the suggested payments actually happened, exactly as the
        /// app's "Mark paid" does: the planned transfer is copied into a real spending
        /// with no category, which is what keeps it out of the spending totals while
        /// still moving both balances.
        /// </summary>
        public async Task<IActionResult> OnPostMarkPaid(string id, string transferId)
        {
            var bad = await LoadOr(id);
            if (bad != null) return bad;

            var planned = Tour.Spendings.FirstOrDefault(s => s.Planned && s.GUID == transferId);
            if (planned == null)
            {
                // the tour changed under the reader and this payment is no longer suggested
                Recorded = null;
                return Redirect($"/t/{id}");
            }

            var copy = planned.SafeClone<Spending>();
            copy.Type = "";
            copy.Color = copy.Description.StartsWith("X") ? "lightgreen" : "lightgray";

            // the calculated tour must never be stored - it carries the generated
            // payments as if they were real spendings
            var raw = LoadTour(id);
            if (raw == null) return NotFound();
            raw = Processor.AddSpending(raw, copy);
            SaveTour(raw);

            Recorded = $"{PersonName(Tour, planned.FromGuid)} → " +
                       $"{PersonName(Tour, planned.ToGuid.FirstOrDefault())}, " +
                       $"{Money(planned.AmountInCurrentCurrency(Tour))}";
            return Redirect($"/t/{id}");
        }
    }
}
