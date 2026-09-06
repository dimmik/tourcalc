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
    /// <summary>
    /// Base for the four pages that show one tour. It loads the tour already calculated
    /// and works out the handful of figures the header carries, so each page is left
    /// with only its own table.
    /// </summary>
    public abstract class TourTextPageModel : TextPageModel
    {
        protected TourTextPageModel(ITcConfiguration configuration, ITourStorage tourStorage)
            : base(configuration, tourStorage) { }

        /// <summary>
        /// What counts as too small to bother with. The app takes this from the reader's
        /// own settings, which live in their browser and are out of reach here, so the
        /// text pages use the same default the app ships with.
        /// </summary>
        private const int DefaultMinimumMeaningfulDebt = 49;

        public Tour Tour { get; private set; }
        public string TourId { get; private set; }

        public long MinMeaningful { get; private set; }
        public string CurrencyName { get; private set; } = "";
        public long TotalSpent { get; private set; }
        public int ExpenseCount { get; private set; }
        public List<Spending> Transfers { get; private set; } = new List<Spending>();
        public long LeftToSettle { get; private set; }

        /// <summary>Loads the tour and the shared figures, or returns the result to send instead.</summary>
        protected async Task<IActionResult> LoadOr(string id)
        {
            await ReadAuth();
            if (!IsSignedIn) return ToLogin();

            TourId = id;
            Tour = LoadTourCalculated(id);
            if (Tour == null) return NotFound();

            CurrencyName = Tour.CurrencyNameEmptyIfSingleCurrency();
            MinMeaningful = Tour.GetAmountInCurrentCurrencyFromMinValued(DefaultMinimumMeaningfulDebt);

            var real = Tour.Spendings.Where(s => !s.Planned).ToList();
            ExpenseCount = real.Count;
            TotalSpent = real.Where(CountsAsSpending).Sum(s => s.AmountInCurrentCurrency(Tour));

            Transfers = Tour.Spendings
                .Where(s => s.Planned && !s.Description.StartsWith("Family"))
                .Where(s => Math.Abs(s.AmountInCurrentCurrency(Tour)) > MinMeaningful)
                .OrderByDescending(s => s.AmountInCurrentCurrency(Tour))
                .ToList();
            LeftToSettle = Transfers.Sum(s => s.AmountInCurrentCurrency(Tour));

            return null;
        }

        /// <summary>What the tour counts as actual spending - paybacks and uncounted drafts are not.</summary>
        public static bool CountsAsSpending(Spending s)
            => !s.Planned && !string.IsNullOrWhiteSpace(s.Type) && (!s.IsDryRun || s.IncludeDryRunInCalc);

        public IEnumerable<Spending> RealSpendings()
            => Tour.Spendings.Where(s => !s.Planned);

        public IEnumerable<(Person person, long amount)> Balances()
            => (Tour.Persons ?? new List<Person>())
                .Where(p => string.IsNullOrWhiteSpace(p.ParentId))
                .Select(p => (person: p, amount: Tour.AmountAPersonWillPay(p)))
                .Where(pa => Math.Abs(pa.amount) > MinMeaningful)
                .OrderByDescending(pa => pa.amount);
    }
}
