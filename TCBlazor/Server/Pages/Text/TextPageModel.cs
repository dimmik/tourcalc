using System;
using System.Collections.Generic;
using System.Linq;
using System.Linq.Expressions;
using System.Threading.Tasks;
using Microsoft.AspNetCore.Authentication;
using Microsoft.AspNetCore.Mvc;
using Microsoft.AspNetCore.Mvc.RazorPages;
using TCalc.Domain;
using TCalc.Logic;
using TCalc.Storage;
using TCalcCore.Auth;
using TCalcCore.Storage;
using Company.TCBlazor;
using Company.TCBlazor.Auth;

namespace TCBlazor.Server.Pages.Text
{
    /// <summary>
    /// Shared plumbing for the text pages: who is reading, and which tours they may see.
    ///
    /// The visibility rules are the ones TourController applies - master sees everything,
    /// anybody else only tours whose access code matches theirs. They are repeated here
    /// rather than called because the controller keeps them in private methods over its
    /// own <c>User</c>; if they ever move to a service, both should use it.
    /// </summary>
    public abstract class TextPageModel : PageModel
    {
        protected readonly ITcConfiguration Configuration;
        protected readonly ITourStorage TourStorage;

        protected TextPageModel(ITcConfiguration configuration, ITourStorage tourStorage)
        {
            Configuration = configuration;
            TourStorage = tourStorage;
        }

        public AuthData Auth { get; private set; } = new AuthData();

        public bool IsSignedIn => Auth.Type != "None";

        /// <summary>
        /// Who the reader is. UseTextAuth has already signed them in from the cookie, so
        /// this only reads the claims - checking it here rather than with an [Authorize]
        /// attribute is what lets a signed-out reader be sent to the login form instead
        /// of being handed a bare 401, which a text browser shows as an error page.
        /// </summary>
        protected Task ReadAuth()
        {
            Auth = AuthHelper.GetAuthData(HttpContext.User, Configuration);
            return Task.CompletedTask;
        }

        /// <summary>Sends a signed-out reader to the login form, remembering where they were going.</summary>
        protected IActionResult ToLogin()
            => Redirect("/t/login?next=" + Uri.EscapeDataString(HttpContext.Request.Path + HttpContext.Request.QueryString));

        protected Tour LoadTour(string tourId)
        {
            var tour = TourStorage.GetTour(tourId);
            if (tour == null) return null;
            if (!Auth.IsMaster && !Auth.AccessCodeMD5s().Contains(tour.AccessCodeMD5)) return null;
            return tour;
        }

        /// <summary>
        /// The tour with everybody's totals filled in and the settle-up payments worked
        /// out - the very calculation the app runs, from TCalcCore, so the text pages
        /// cannot quote a different number from the one the app shows.
        /// </summary>
        protected Tour LoadTourCalculated(string tourId)
        {
            var tour = LoadTour(tourId);
            if (tour == null) return null;
            return new TourCalculator(tour).SuggestFinalPayments();
        }

        protected IEnumerable<Tour> LoadTours()
        {
            Expression<Func<Tour, bool>> predicate;
            if (Auth.IsMaster)
            {
                predicate = t => true;
            }
            else
            {
                var mine = Auth.AccessCodeMD5s().ToList();
                predicate = t => t.AccessCodeMD5 != null && mine.Contains(t.AccessCodeMD5);
            }
            var tours = TourStorage.GetTours(
                predicate,
                Configuration.GetValue("ReturnVersionsInAllTours", false),
                0, int.MaxValue, out _);
            return tours ?? Enumerable.Empty<Tour>();
        }

        // ---- writing ---------------------------------------------------------------

        /// <summary>
        /// The same processor the API uses; it holds what a change to a tour actually
        /// means - dropping the planned payments a change invalidates, detaching the
        /// people a deleted person paid for, and so on.
        /// </summary>
        protected readonly ITourStorageProcessor Processor = new TourStorageProcessor();

        /// <summary>
        /// Stores a changed tour. The state id has to move: the app guards against two
        /// people saving over each other by comparing it, and a change made here that
        /// left it alone would be silently overwritten by an app tab that had the tour
        /// open from before.
        ///
        /// Load, change and store all happen inside one request, so the window for two
        /// text posts to race is small - but it is not zero, and the loser is the one
        /// whose change is lost rather than rejected. Worth revisiting if the text pages
        /// ever get more than occasional use.
        /// </summary>
        protected void SaveTour(Tour tour)
        {
            tour.StateGUID = IdHelper.NewStateGuid();
            TourStorage.StoreTour(tour);
        }

        // ---- formatting helpers the views use -------------------------------------

        /// <summary>Money the way the rest of the app writes it: grouped, no decimals.</summary>
        public static string Money(long cents)
            => cents.ToString("N0", TCalcCore.UI.GlobConsts.NumGroupSpaceSeparated);

        public static string PersonName(Tour tour, string guid)
            => tour?.Persons?.FirstOrDefault(p => p.GUID == guid)?.Name ?? "n/a";

        /// <summary>
        /// Caps a cell so a table still fits an eighty-column terminal. lynx sizes each
        /// column to its widest cell and, when the total will not fit, abandons the table
        /// and spills the cells across the line - taking the alignment of every other row
        /// with it. One very long name was enough to do that to the expense list, so the
        /// columns that carry names are bounded here.
        /// </summary>
        public static string Short(string text, int max)
        {
            if (string.IsNullOrEmpty(text) || text.Length <= max) return text ?? "";
            return text.Substring(0, Math.Max(1, max - 1)) + "…";
        }
    }
}
