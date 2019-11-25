using Blazor.Extensions.Storage;
using Microsoft.AspNetCore.Components;
using Newtonsoft.Json;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Net.Http;
using System.Threading.Tasks;
using TCalc.Domain;

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
        private string Token = null;
        private readonly object tokengetlock = new object();
        public async Task Init()
        {
            if (Token == null)
            {
                Token = await LocalStorage.GetItem<string>("__tc_token");
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
            await LocalStorage.SetItem<string>("__tc_token", Token);
        }
        private HttpClient GetHttpClient()
        {
            if (Token == null) return new HttpClient();
            var c = new HttpClient();
            c.DefaultRequestHeaders.Add("Authorization", $"Bearer {Token}");
            return c;
        }
        public async Task<TourList> GetTourList()
        {
            using (var client = GetHttpClient())
            {
                var url = $"{BackendUrl}/api/tour";
                var response = await client.GetAsync(url);
                var json = await response.Content.ReadAsStringAsync();
                var tours = JsonConvert.DeserializeObject<TourList>(json);
                return tours;
            }
        }
    }
}
