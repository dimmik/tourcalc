using Microsoft.AspNetCore.Mvc.RazorPages;

namespace TCBlazor.Server.Pages.Tg
{
    /// <summary>
    /// The Mini App's door. It holds no logic of its own - the page asks the server who the
    /// viewer is and is told where to go.
    /// </summary>
    public class TgAppModel : PageModel
    {
        public void OnGet()
        {
        }
    }
}
