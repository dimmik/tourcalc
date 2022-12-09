using Microsoft.AspNetCore.Components.Web;
using Microsoft.AspNetCore.Components.WebAssembly.Hosting;
using TCBlazor.Client;
using TCalcCore.Storage;
using TCalcCore.Network;
using TCalcCore.Logging;
using TCalcCore.UI;
using TCalcCore.Engine;
using TCBlazor.Client.SharedCode;
using TCalcCore.Auth;

namespace Company.WebApplication1
{
    public class TCBlazorClientMain
    {
        public static async Task Main(string[] args)
        {
            var builder = WebAssemblyHostBuilder.CreateDefault(args);
            builder.RootComponents.Add<App>("#app");
            builder.RootComponents.Add<HeadOutlet>("head::after");

            builder.Services
                .AddSingleton<ILocalLogger, LocalLogger>()
                .AddScoped(sp => new HttpClient { BaseAddress = new Uri(builder.HostEnvironment.BaseAddress) })
                .AddAntDesign()
                .AddScoped<ISimpleMessageShower, SimpleMessageShower>()
                .AddScoped<ITokenStorage, CookieTokenStorage>()
                .AddSingleton<ITourcalcLocalStorage, TourcalcLocalStorage>()
                .AddScoped<EnrichedHttpClient>()
                .AddScoped<ITourRetriever, HttpBasedTourRetriever>()
                .AddScoped<ITCDataService, TCDataService>()
                .AddScoped<AuthSvc>()
                .AddScoped<TCDataSyncService>()
                .AddScoped<TourcalcEngine>()
                ;



            await builder.Build().RunAsync();
        }
    }
}