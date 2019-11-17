using Newtonsoft.Json;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Net.Http;
using System.Threading.Tasks;
using TCalc.Domain;

namespace TourCalcBlazor.Data
{
    public class TourCalcDataService
    {
        private readonly string BackendUrl;
        private string Token = "eyJhbGciOiJFUzI1NiIsInR5cCI6IkpXVCJ9" +
            ".eyJodHRwOi8vc2NoZW1hcy54bWxzb2FwLm9yZy93cy8yMDA1LzA1L2lkZW50aXR5L2NsYWltcy9uYW1laWRlbnRpZmllciI6ImFkbWluIiwiQXV0aERhdGFKc29uIjoie1wiVHlwZ" +
            "VwiOlwiTWFzdGVyXCIsXCJJc01hc3RlclwiOnRydWUsXCJBY2Nlc3NDb2RlTUQ1XCI6XCJcIixcIlRvdXJJZHNcIjpbXX0iLCJleHAiOjE1ODgxNTYxMjksImlzcyI6IlRvdXJDYWx" +
            "jIiwiYXVkIjoiVXNlcnMifQ.HPKvlwBFDW6fx3egBOimbB7lnzUthoCAi2ebtoe2G_B7So6H_6BT2tePD98Kueasv68WaG9empSEUaJ6e1XAJA";//= null;
        public TourCalcDataService(string url)
        {
            BackendUrl = url;
        }
        public bool IsAuthorized => (Token != null);
        public async Task Login(string scope, string code)
        {
            using (var client = GetHttpClient())
            {
                var url = $"{BackendUrl}/api/auth/token/{scope}/{code}";
                var response = await client.GetAsync(url);
                Token = await response.Content.ReadAsStringAsync();
            }
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
