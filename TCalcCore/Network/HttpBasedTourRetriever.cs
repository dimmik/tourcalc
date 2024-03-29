﻿using System;
using System.Collections.Generic;
using System.Drawing;
using System.Net.Http;
using System.Text;
using System.Threading.Tasks;
using TCalc.Domain;
using TCalcCore.Auth;
using TCalcCore.Storage;

namespace TCalcCore.Network
{
    public class HttpBasedTourRetriever : ITourRetriever
    {
        private readonly EnrichedHttpClient http;

        public HttpBasedTourRetriever(EnrichedHttpClient http)
        {
            this.http = http ?? throw new ArgumentNullException(nameof(http));
        }

        public async Task<string> AddTour(Tour tour, string code, string token, Action<string> errorHandler)
        {
            var tid = await http.CallWithAuthToken<string>($"/api/Tour/add/{code ?? CodeThatForSureIsNotUsed}", token, HttpMethod.Post, tour, errorHandler);
            return tid;
        }

        public async Task<string> DeleteTour(string tourId, string token, Action<string> errorHandler)
        {
            var tid = await http.CallWithAuthToken<string>($"/api/Tour/{tourId}", token, HttpMethod.Delete, null, errorHandler);
            return tid;
        }

        public async Task<AuthData> GetAuthData(string token, Action<string> errorHandler)
        {
            var ad = await http.CallWithAuthToken<AuthData>("/api/Auth/whoami", token, errorHandler);
            return ad;
        }
        private static readonly string CodeThatForSureIsNotUsed = "__trashNoTours__";
        public async Task<string> GetToken(string scope, string code, bool isMd5, Action<string> errorHandler)
        {
            var token = await http.GetStringAsync($"/api/Auth/token/{scope ?? "code"}/{code ?? CodeThatForSureIsNotUsed}{(isMd5 ? "/md5" : "")}", errorHandler);
            return token;
        }

        public async Task<Tour> GetTour(string tourId, string token, Action<string> errorHandler)
        {
            var t = await http.CallWithAuthToken<Tour>($"/api/Tour/{tourId}", token, errorHandler);
            return t;
        }

        public async Task<TourList> GetTourList(string token, Action<string> errorHandler)
        {
            var from = 0;
            var count = 1000;
            var code = "";
            var tours = await http.CallWithAuthToken<TourList>($"/api/Tour/all/suggested?from={from}&count={count}&code={code}", token, errorHandler);
            return tours;
        }

        public async Task<TourList> GetTourVersions(string token, string tourId, Action<string> errorHandler)
        {
            var from = 0;
            var count = 1000;
            var tours = await http.CallWithAuthToken<TourList>($"/api/Tour/{tourId}/versions?from={from}&count={count}", token, errorHandler);
            return tours;
        }

        public async Task<string> UpdateTour(string tourId, Tour tour, string token, Action<string> errorHandler)
        {
            var tid = await http.CallWithAuthToken<string>($"/api/Tour/{tourId}", token, new HttpMethod("PATCH"), tour, errorHandler);
            return tid;
        }
    }
}
