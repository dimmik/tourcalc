using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using Microsoft.AspNetCore.Mvc;
using TCalc.Domain;
using TCalc.Storage;
using Company.TCBlazor;

namespace TCBlazor.Server.Pages.Text
{
    public class PersonEditModel : TourTextPageModel
    {
        public PersonEditModel(ITcConfiguration configuration, ITourStorage tourStorage)
            : base(configuration, tourStorage) { }

        public string PersonId { get; private set; }
        public bool IsNew => string.IsNullOrEmpty(PersonId);
        public string Error { get; private set; }

        [BindProperty] public string Name { get; set; } = "";
        [BindProperty] public int Weight { get; set; } = 100;
        [BindProperty] public string ParentId { get; set; } = "";

        /// <summary>Anyone but this person - nobody can be paid for by themselves.</summary>
        public IEnumerable<Person> PossibleParents()
            => Tour.Persons.Where(p => p.GUID != PersonId);

        public async Task<IActionResult> OnGet(string id, string personId)
        {
            var bad = await LoadOr(id);
            if (bad != null) return bad;

            PersonId = personId;
            if (IsNew) return Page();

            var person = Tour.Persons.FirstOrDefault(p => p.GUID == personId);
            if (person == null) return NotFound();
            Name = person.Name;
            Weight = person.Weight;
            ParentId = person.ParentId ?? "";
            return Page();
        }

        public async Task<IActionResult> OnPost(string id, string personId)
        {
            var bad = await LoadOr(id);
            if (bad != null) return bad;

            PersonId = personId;
            if (string.IsNullOrWhiteSpace(Name)) { Error = "A person needs a name."; return Page(); }

            var raw = LoadTour(id);
            if (raw == null) return NotFound();

            if (IsNew)
            {
                var person = new Person
                {
                    GUID = IdHelper.NewId(),
                    Name = Name.Trim(),
                    Weight = Weight,
                    ParentId = string.IsNullOrEmpty(ParentId) ? null : ParentId,
                };
                raw = Processor.AddPerson(raw, person);
            }
            else
            {
                var person = raw.Persons.FirstOrDefault(p => p.GUID == personId);
                if (person == null) return NotFound();
                person.Name = Name.Trim();
                person.Weight = Weight;
                person.ParentId = string.IsNullOrEmpty(ParentId) ? null : ParentId;
                raw = Processor.UpdatePerson(raw, person, personId);
                if (raw == null) return NotFound();
            }
            SaveTour(raw);
            return Redirect($"/t/{id}/people");
        }

        public async Task<IActionResult> OnPostDelete(string id, string personId)
        {
            await ReadAuth();
            if (!IsSignedIn) return ToLogin();

            var raw = LoadTour(id);
            if (raw == null) return NotFound();
            raw = Processor.DeletePerson(raw, personId);
            if (raw == null) return NotFound();
            SaveTour(raw);
            return Redirect($"/t/{id}/people");
        }
    }
}
