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
    public class PeopleModel : TourTextPageModel
    {
        public PeopleModel(ITcConfiguration configuration, ITourStorage tourStorage)
            : base(configuration, tourStorage) { }

        public async Task<IActionResult> OnGet(string id) => await LoadOr(id) ?? Page();

        /// <summary>
        /// Everybody, with the people someone pays for listed directly under them - the
        /// same grouping the app draws, so the two lists read in the same order.
        /// </summary>
        public IEnumerable<(Person person, bool isChild, long settle)> Rows()
        {
            var all = Tour.Persons ?? new List<Person>();
            var byGuid = all.GroupBy(p => p.GUID).ToDictionary(g => g.Key, g => g.First());
            bool HasParent(Person p) => !string.IsNullOrWhiteSpace(p.ParentId) && byGuid.ContainsKey(p.ParentId);

            long Settle(Person p)
            {
                // someone who is paid for hands nothing over themselves, so their own
                // figure is the honest one to show
                var v = HasParent(p) ? p.Debt() : Tour.AmountAPersonWillPay(p);
                return Math.Abs(v) > MinMeaningful ? v : 0;
            }

            foreach (var head in all.Where(p => !HasParent(p)).OrderBy(p => p.Name))
            {
                yield return (head, false, Settle(head));
                foreach (var kid in all.Where(p => HasParent(p) && p.ParentId == head.GUID).OrderBy(p => p.Name))
                {
                    yield return (kid, true, Settle(kid));
                }
            }
        }
    }
}
