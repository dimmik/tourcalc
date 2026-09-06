using Microsoft.AspNetCore.Mvc.RazorPages;
using Company.TCBlazor.Auth;

namespace TCBlazor.Server.Pages.Text
{
    public class LogoutModel : PageModel
    {
        public void OnGet() => TextAuth.ClearCookie(Response);
    }
}
