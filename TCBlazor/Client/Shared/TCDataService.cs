using TCalc.Domain;
using TCalcCore.Auth;
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


        public async Task<AuthData?> GetAuthData()
        {
            var token = await ts.GetToken();
            var ad = await http.CallWithAuthToken<AuthData?>("/api/Auth/whoami", token);
            return ad;
        }

        public async Task GetAndStoreToken(string? scope, string? code)
        {
            var url = $"/api/Auth/token/{scope ?? "code"}/{code ?? "trashNoTours"}";
            var token = await http.GetStringAsync(url);
            await ts.SetToken(token);
        }
        public async Task ClearToken()
        {
            await ts.SetToken("");
        }
        public async Task GetAndStoreTokenForCodeMd5(string? code)
        {
            var url = $"/api/Auth/token/code/{code ?? "trashNoTours"}/md5";
            var token = await http.GetStringAsync(url);
            await ts.SetToken(token);
        }

        public async Task<Tour?> LoadTour(string? id)
        {
            if (id == null) return default;
            var token = await ts.GetToken();
            var t = await http.CallWithAuthToken<Tour>($"/api/Tour/{id}/suggested", token);
            return t;
        }
        public async Task<Tour?> LoadTourBare(string? id)
        {
            if (id == null) return default;
            var token = await ts.GetToken();
            var t = await http.CallWithAuthToken<Tour>($"/api/Tour/{id}", token);
            return t;
        }
        public async Task DeleteTour(Tour? tour)
        {
            if (tour == null) return;
            await http.CallWithAuthToken<string>($"/api/Tour/{tour.Id}", await ts.GetToken(), HttpMethod.Delete, null);
        }
        public async Task EditTour(Tour? tour, string? operation)
        {
            if (tour == null) return;
            if (operation == null) return;
            await http.CallWithAuthToken<string>($"/api/Tour/{tour.Id}/{operation}", await ts.GetToken(), HttpMethod.Patch, tour);
        }
        public async Task AddTour(Tour? tour, string? code)
        {
            if (tour == null) return;
            await http.CallWithAuthToken<string>($"/api/Tour/add/{code ?? "trashNoTours"}", await ts.GetToken(), HttpMethod.Post, tour);
        }
        public async Task<TourList?> GetTourList()
        {
            var token = await ts.GetToken();
            // TODO pagination, links, all the stuff
            var from = 0;
            var count = 1000;
            var code = "";
            var tours = await http.CallWithAuthToken<TourList>($"/api/Tour/all/suggested?from={from}&count={count}&code={code}", token);
            return tours;
        }

        #region Persons
        public async Task DeletePerson(string? tourId, Person? p)
        {
            if (tourId == null) return;
            if (p == null) return;
            await http.CallWithAuthToken<string>($"/api/Tour/{tourId}/person/{p.GUID}", await ts.GetToken(), HttpMethod.Delete, null);
        }
        public async Task EditPerson(string? tourId, Person? p)
        {
            if (tourId == null) return;
            if (p == null) return;
            await http.CallWithAuthToken<string>($"/api/Tour/{tourId}/person/{p.GUID}", await ts.GetToken(), HttpMethod.Patch, p);
        }
        public async Task AddPerson(string? tourId, Person? p)
        {
            if (tourId == null) return;
            if (p == null) return;
            await http.CallWithAuthToken<string>($"/api/Tour/{tourId}/person", await ts.GetToken(), HttpMethod.Post, p);
        }
        #endregion
        #region Spendings
        public async Task DeleteSpending(string? tourId, Spending? s)
        {
            if (tourId == null) return;
            if (s == null) return;
            await http.CallWithAuthToken<string>($"/api/Tour/{tourId}/spending/{s.GUID}", await ts.GetToken(), HttpMethod.Delete, null);
        }
        public async Task EditSpending(string? tourId, Spending? s)
        {
            if (tourId == null) return;
            if (s == null) return;
            await http.CallWithAuthToken<string>($"/api/Tour/{tourId}/spending/{s.GUID}", await ts.GetToken(), HttpMethod.Patch, s);
        }
        public async Task AddSpending(string? tourId, Spending? s)
        {
            if (tourId == null) return;
            if (s == null) return;
            await http.CallWithAuthToken<string>($"/api/Tour/{tourId}/spending", await ts.GetToken(), HttpMethod.Post, s);
        }

        #endregion

    }
}
