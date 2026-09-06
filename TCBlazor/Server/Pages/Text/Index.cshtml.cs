using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using Microsoft.AspNetCore.Mvc;
using TCalc.Domain;
using TCalc.Storage;
using Company.TCBlazor;

namespace TCBlazor.Server.Pages.Text
{
    public class IndexModel : TextPageModel
    {
        public IndexModel(ITcConfiguration configuration, ITourStorage tourStorage)
            : base(configuration, tourStorage) { }

        public IList<Tour> Tours { get; private set; } = new List<Tour>();

        public async Task<IActionResult> OnGet()
        {
            await ReadAuth();
            if (!IsSignedIn) return ToLogin();

            // archived tours are hidden here exactly as they are in the app's default list
            Tours = LoadTours()
                .Where(t => !t.IsArchived)
                .OrderBy(t => t.Name)
                .ToList();
            return Page();
        }
    }
}
