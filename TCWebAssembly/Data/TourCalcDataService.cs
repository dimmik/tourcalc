using Blazor.Extensions.Storage;
using Microsoft.AspNetCore.Components;
using Newtonsoft.Json;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Net.Http;
using System.Threading.Tasks;
using TCalc.Domain;
using TCalc.Logic;

namespace TCWebAssembly.Data
{
    public class BackendUrl
    {
        public string url { get; set; }
    }
    public class TourCalcDataService
    {
        private readonly string BackendUrl;
        public bool Initialized = false;
        const string TokenStorageKey = "__tc_token";
        private string Token = null;
        private readonly object TokenGetlock = new object();

        private TourList TourList;
        private Tour CurrentTour;


        public async Task Init()
        {
            if (Token == null)
            {
                Token = await LocalStorage.GetItem<string>(TokenStorageKey);
            }
            Initialized = true;
        }
        public TourCalcDataService(BackendUrl bu, LocalStorage st)
        {
            BackendUrl = bu.url;
            LocalStorage = st;
        }
        
        protected LocalStorage LocalStorage { get; set; }

        public bool IsAuthorized => Token != null;
        public async Task Login(string scope, string code)
        {
            using (var client = GetHttpClient())
            {
                var url = $"{BackendUrl}/api/auth/token/{scope}/{code}";
                var response = await client.GetAsync(url);
                Token = await response.Content.ReadAsStringAsync();
            }
            await LocalStorage.SetItem<string>(TokenStorageKey, Token);
        }
        private HttpClient GetHttpClient()
        {
            if (Token == null) return new HttpClient();
            var c = new HttpClient();
            c.DefaultRequestHeaders.Add("Authorization", $"Bearer {Token}");
            return c;
        }
        private async Task RefreshTours()
        {
            using (var client = GetHttpClient())
            {
                var url = $"{BackendUrl}/api/tour";
                var response = await client.GetAsync(url);
                var json = await response.Content.ReadAsStringAsync();
                var tours = JsonConvert.DeserializeObject<TourList>(json);
                TourList = tours;
            }

        }
        public async Task<TourList> GetTourList(bool refresh = false)
        {
            if (TourList == null || refresh)
            {
                await RefreshTours();
            }
            var ts = TourList.Tours.Select(t => new TourCalculator(t).SuggestFinalPayments());
            TourList.Tours = ts;
            return TourList;
        }
        
        private async Task RefreshTour(string id)
        {
            using (var client = GetHttpClient())
            {
                var url = $"{BackendUrl}/api/tour/{id}";
                var response = await client.GetAsync(url);
                var json = await response.Content.ReadAsStringAsync();
                var tour = JsonConvert.DeserializeObject<Tour>(json);
                CurrentTour = tour;
            }

        }
        public async Task<Tour> GetTour(string id, bool refresh = false)
        {
            if (CurrentTour == null || CurrentTour.Id != id || refresh)
            {
                await RefreshTour(id);
            }
            return CurrentTour;
        }
    }
}
