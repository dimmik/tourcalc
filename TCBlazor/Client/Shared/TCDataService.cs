using Microsoft.AspNetCore.WebUtilities;
using System.Text;
using System.Text.Json;
using TCalc.Domain;
using TCalc.Logic;
using TCalc.Storage;
using TCalcCore.Auth;
using TCBlazor.Client.Storage;

namespace TCBlazor.Client.Shared
{
    public class TCDataService
    {
        private readonly TourcalcLocalStorage ts;
        private readonly EnrichedHttpClient http;
        private readonly TourStorageProcessor tourStorageProcessor = new TourStorageProcessor();

        public TCDataService(TourcalcLocalStorage ts, EnrichedHttpClient http)
        {
            this.ts = ts ?? throw new ArgumentNullException(nameof(ts));
            this.http = http ?? throw new ArgumentNullException(nameof(http));
        }


        public async Task<AuthData?> GetAuthData(bool forceGetFromServer = false)
        {
            var token = await ts.GetToken();
            AuthData? ad;
            if (forceGetFromServer || !token.Contains('.'))
            {
                ad = await http.CallWithAuthToken<AuthData>("/api/Auth/whoami", token);
            } 
            else
            {
                try
                {
                    var parts = token.Split('.');
                    var meaningful = parts[1];
                    var plain = Encoding.UTF8.GetString(WebEncoders.Base64UrlDecode(meaningful));
                    var authDataContainer = JsonSerializer.Deserialize<AuthDataContainer>(plain);
                    string adStr = (authDataContainer?.AuthDataJson ?? "").Trim();
                    ad = JsonSerializer.Deserialize<AuthData>(adStr);
                    if (ad == null) throw new Exception("cannot get auth info from token");
                } catch
                {
                    ad = await http.CallWithAuthToken<AuthData?>("/api/Auth/whoami", token);
                }
            }
            return ad;
        }

        public async Task GetAndStoreToken(string? scope, string? code)
        {
            var url = $"/api/Auth/token/{scope ?? "code"}/{code ?? CodeThatForSureIsNotUsed}";
            var token = await http.GetStringAsync(url);
            await ts.SetToken(token);
        }
        public async Task ClearToken()
        {
            await ts.SetToken("");
        }
        private static readonly string CodeThatForSureIsNotUsed = "__trashNoTours__";
        public async Task GetAndStoreTokenForCodeMd5(string? code)
        {
            var url = $"/api/Auth/token/code/{code ?? CodeThatForSureIsNotUsed}/md5";
            var token = await http.GetStringAsync(url);
            await ts.SetToken(token);
        }

        public async Task<Tour?> LoadTour(string? id)
        {
            if (id == null) return default;

            var tour = await LoadTourBare(id);
            var calculator = new TourCalculator(tour);
            var calculated = calculator.SuggestFinalPayments();
            return calculated;
            //return tour;
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
        public async Task EditTourProps(Tour? tour, string? operation)
        {
            if (tour == null) return;
            if (operation == null) return;
            await http.CallWithAuthToken<string>($"/api/Tour/{tour.Id}/{operation}", await ts.GetToken(), HttpMethod.Patch, tour);
        }
        private async Task UpdateTour(string? tourId, Tour? tour)
        {
            if (tour == null) return;
            if (tourId == null) return;
            await http.CallWithAuthToken<string>($"/api/Tour/{tour.Id}", await ts.GetToken(), HttpMethod.Patch, tour);
        }
        public async Task AddTour(Tour? tour, string? code)
        {
            if (tour == null) return;
            await http.CallWithAuthToken<string>($"/api/Tour/add/{code ?? CodeThatForSureIsNotUsed}", await ts.GetToken(), HttpMethod.Post, tour);
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
        private async Task EditTourData(string tourId, Func<Tour, Tour> process)
        {
            Tour? tour = await LoadTourBare(tourId);
            if (tour == null) return;
            tour = process(tour);
            await UpdateTour(tour.GUID, tour);
        }
        #region Persons
        public async Task DeletePerson(string? tourId, Person? p)
        {
            if (tourId == null) return;
            if (p == null) return;
            await EditTourData(tourId, t => tourStorageProcessor.DeletePerson(t, p.GUID));
        }
        public async Task EditPerson(string? tourId, Person? p)
        {
            if (tourId == null) return;
            if (p == null) return;
            await EditTourData(tourId, (t) => tourStorageProcessor.UpdatePerson(t, p, p.GUID));
        }

        

        public async Task AddPerson(string? tourId, Person? p)
        {
            if (tourId == null) return;
            if (p == null) return;
            await EditTourData(tourId, t => tourStorageProcessor.AddPerson(t, p));
        }
        #endregion
        #region Spendings
        public async Task DeleteSpending(string? tourId, Spending? s)
        {
            if (tourId == null) return;
            if (s == null) return;
            await EditTourData(tourId, t => tourStorageProcessor.DeleteSpending(t, s.GUID));
        }
        public async Task EditSpending(string? tourId, Spending? s)
        {
            if (tourId == null) return;
            if (s == null) return;
            await EditTourData(tourId, t => tourStorageProcessor.UpdateSpending(t, s, s.GUID));
        }
        public async Task AddSpending(string? tourId, Spending? s)
        {
            if (tourId == null) return;
            if (s == null) return;
            await EditTourData(tourId, t => tourStorageProcessor.AddSpending(t, s));
        }

        #endregion

    }
    class AuthDataContainer
    {
        public string AuthDataJson { get; set; } = "";
    }
}
