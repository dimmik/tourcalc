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
    /// One form for both adding and editing, the way the app uses one dialog for both:
    /// the fields are the same, and keeping them in one place is what stops the two
    /// drifting apart.
    /// </summary>
    public class SpendEditModel : TourTextPageModel
    {
        public SpendEditModel(ITcConfiguration configuration, ITourStorage tourStorage)
            : base(configuration, tourStorage) { }

        public string SpendingId { get; private set; }
        public bool IsNew => string.IsNullOrEmpty(SpendingId);
        public string Error { get; private set; }

        [BindProperty] public long Amount { get; set; }
        [BindProperty] public string Description { get; set; } = "";
        [BindProperty] public string FromGuid { get; set; } = "";
        [BindProperty] public bool ToAll { get; set; }
        [BindProperty] public List<string> ToGuid { get; set; } = new List<string>();
        [BindProperty] public string Type { get; set; } = "";
        [BindProperty] public string Date { get; set; } = "";
        [BindProperty] public bool IsDryRun { get; set; }

        public List<string> Categories { get; private set; } = new List<string>();

        public async Task<IActionResult> OnGet(string id, string spendingId)
        {
            var bad = await LoadOr(id);
            if (bad != null) return bad;

            SpendingId = spendingId;
            Categories = Tour.Spendings
                .Where(s => !s.Planned && !string.IsNullOrWhiteSpace(s.Type))
                .Select(s => s.Type).Distinct().OrderBy(t => t).ToList();

            if (IsNew)
            {
                ToAll = true;
                Date = DateTime.Now.ToString("yyyy-MM-dd");
                FromGuid = Tour.Persons.FirstOrDefault()?.GUID ?? "";
                return Page();
            }

            var sp = Tour.Spendings.FirstOrDefault(s => s.GUID == spendingId && !s.Planned);
            if (sp == null) return NotFound();

            Amount = sp.AmountInCents;
            Description = sp.Description;
            FromGuid = sp.FromGuid;
            ToAll = sp.ToAll;
            ToGuid = sp.ToGuid?.ToList() ?? new List<string>();
            Type = sp.Type;
            Date = sp.SpendingDate.ToString("yyyy-MM-dd");
            IsDryRun = sp.IsDryRun;
            return Page();
        }

        public async Task<IActionResult> OnPost(string id, string spendingId)
        {
            var bad = await LoadOr(id);
            if (bad != null) return bad;

            SpendingId = spendingId;
            Categories = Tour.Spendings
                .Where(s => !s.Planned && !string.IsNullOrWhiteSpace(s.Type))
                .Select(s => s.Type).Distinct().OrderBy(t => t).ToList();

            if (string.IsNullOrWhiteSpace(Description)) { Error = "Give the expense a description."; return Page(); }
            if (Amount <= 0) { Error = "The amount has to be more than zero."; return Page(); }
            if (string.IsNullOrWhiteSpace(FromGuid)) { Error = "Say who paid."; return Page(); }
            if (!ToAll && !ToGuid.Any()) { Error = "Pick who the expense is for, or tick \"for everyone\"."; return Page(); }

            var raw = LoadTour(id);
            if (raw == null) return NotFound();

            var sp = IsNew ? new Spending() : raw.Spendings.FirstOrDefault(s => s.GUID == spendingId);
            if (sp == null) return NotFound();

            sp.AmountInCents = Amount;
            sp.Description = Description.Trim();
            sp.FromGuid = FromGuid;
            sp.ToAll = ToAll;
            sp.ToGuid = ToAll ? new List<string>() : ToGuid.ToList();
            sp.Type = (Type ?? "").Trim();
            sp.IsDryRun = IsDryRun;
            if (DateTime.TryParse(Date, out var d)) sp.SpendingDate = d;
            // a tour with several currencies is recorded in its own: the text form has
            // no currency picker, and guessing one would be worse than not offering it
            if (sp.Currency == null) sp.Currency = raw.Currency;

            raw = IsNew ? Processor.AddSpending(raw, sp) : Processor.UpdateSpending(raw, sp, spendingId);
            if (raw == null) return NotFound();
            SaveTour(raw);

            return Redirect($"/t/{id}/spend");
        }

        public async Task<IActionResult> OnPostDelete(string id, string spendingId)
        {
            await ReadAuth();
            if (!IsSignedIn) return ToLogin();

            var raw = LoadTour(id);
            if (raw == null) return NotFound();
            raw = Processor.DeleteSpending(raw, spendingId);
            if (raw == null) return NotFound();
            SaveTour(raw);
            return Redirect($"/t/{id}/spend");
        }
    }
}
