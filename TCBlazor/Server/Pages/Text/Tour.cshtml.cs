using System.Threading.Tasks;
using Microsoft.AspNetCore.Mvc;
using TCalc.Storage;
using Company.TCBlazor;

namespace TCBlazor.Server.Pages.Text
{
    public class TourModel : TourTextPageModel
    {
        public TourModel(ITcConfiguration configuration, ITourStorage tourStorage)
            : base(configuration, tourStorage) { }

        public async Task<IActionResult> OnGet(string id) => await LoadOr(id) ?? Page();
    }
}
