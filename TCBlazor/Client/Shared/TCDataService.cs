using TCalc.Domain;
using TCBlazor.Client.Storage;

namespace TCBlazor.Client.Shared
{
    public class TCDataService
    {
        private readonly TourcalcLocalStorage ts;
        private readonly EnrichedHttpClient http;

        public TCDataService(TourcalcLocalStorage ts, EnrichedHttpClient http)
        {
            this.ts = ts ?? throw new ArgumentNullException(nameof(ts));
            this.http = http ?? throw new ArgumentNullException(nameof(http));
        }

        public async Task<Tour?> LoadTour(string id)
        {
            var token = await ts.GetToken();
            var t = await http.CallWithAuthToken<Tour>($"/api/Tour/{id}/suggested", token);
            return t;
        }
        #region Persons
        public async Task DeletePerson(string tourId, Person p)
        {
            await http.CallWithAuthToken<string>($"/api/Tour/{tourId}/person/{p.GUID}", await ts.GetToken(), HttpMethod.Delete, null);
        }
        public async Task EditPerson(string tourId, Person p)
        {
            await http.CallWithAuthToken<string>($"/api/Tour/{tourId}/person/{p.GUID}", await ts.GetToken(), HttpMethod.Patch, p);
        }
        public async Task AddPerson(string tourId, Person p)
        {
            await http.CallWithAuthToken<string>($"/api/Tour/{tourId}/person", await ts.GetToken(), HttpMethod.Post, p);
        }
        #endregion
        #region Spendings
        public async Task DeleteSpending(string tourId, Spending s)
        {
            await http.CallWithAuthToken<string>($"/api/Tour/{tourId}/spending/{s.GUID}", await ts.GetToken(), HttpMethod.Delete, null);
        }
        public async Task EditSpending(string tourId, Spending s)
        {
            await http.CallWithAuthToken<string>($"/api/Tour/{tourId}/spending/{s.GUID}", await ts.GetToken(), HttpMethod.Patch, s);
        }
        public async Task AddSpending(string tourId, Spending s)
        {
            await http.CallWithAuthToken<string>($"/api/Tour/{tourId}/spending", await ts.GetToken(), HttpMethod.Post, s);
        }

        #endregion

    }
}
