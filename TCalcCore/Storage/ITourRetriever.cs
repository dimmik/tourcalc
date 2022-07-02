using Newtonsoft.Json.Linq;
using System;
using System.Collections.Generic;
using System.Drawing;
using System.Text;
using System.Threading.Tasks;
using TCalc.Domain;
using TCalcCore.Auth;
using static System.Net.WebRequestMethods;

namespace TCalcCore.Storage
{
    public interface ITourRetriever
    {
        // await http.CallWithAuthToken<AuthData>("/api/Auth/whoami", token);
        Task<AuthData> GetAuthData(string token, Action<string> errorHandler);
        
        // await http.GetStringAsync($"/api/Auth/token/{scope ?? "code"}/{code ?? CodeThatForSureIsNotUsed}");
        Task<string> GetToken(string scope, string code, bool isMd5, Action<string> errorHandler);
        
        // await http.CallWithAuthToken<Tour>($"/api/Tour/{id}", token, showErrorMessages: false);
        Task<Tour> GetTour(string tourId, string token, Action<string> errorHandler);
        
        // tid = await http.CallWithAuthToken<string>($"/api/Tour/add/{code ?? CodeThatForSureIsNotUsed}", (await ts.GetToken()).val, HttpMethod.Post, tour);
        Task<string> AddTour(Tour tour, string code, string token, Action<string> errorHandler);
        
        //var tid = await http.CallWithAuthToken<string>($"/api/Tour/{tour.Id}", (await ts.GetToken()).val, new HttpMethod("PATCH"), tour);
        Task<string> UpdateTour(string tourId, Tour tour, string token, Action<string> errorHandler);
        
        // await http.CallWithAuthToken<string>($"/api/Tour/{tour.Id}", (await ts.GetToken()).val, HttpMethod.Delete, null);
        Task<string> DeleteTour(string tourId, string token, Action<string> errorHandler);

        //var tours = await http.CallWithAuthToken<TourList>($"/api/Tour/all/suggested?from={from}&count={count}&code={code}", token, showErrorMessages: true);
        Task<TourList> GetTourList(string token, Action<string> errorHandler);
    }
}
