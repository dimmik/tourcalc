using Microsoft.AspNetCore.Components.Web;
using Microsoft.AspNetCore.Components.WebAssembly.Hosting;
using TCBlazor.Client;
using TCalcCore.Storage;
using TCalcCore.Network;
using TCalcCore.Logging;
using TCalcCore.UI;
using TCalcCore.Engine;
using TCBlazor.Client.SharedCode;

namespace TCBlazor.Client
{
    public class TCBlazorClientMain
    {
        public static async Task Main(string[] args)
        {
            var builder = WebAssemblyHostBuilder.CreateDefault(args);
            builder.RootComponents.Add<App>("#app");
            builder.RootComponents.Add<HeadOutlet>("head::after");

            /*builder.Services
                .AddSingleton<ILocalLogger, LocalLogger>()
                .AddScoped(sp => new HttpClient { BaseAddress = new Uri(builder.HostEnvironment.BaseAddress) })
                .AddAntDesign()
                .AddScoped<ISimpleMessageShower, SimpleMessageShower>()
                .AddSingleton<ITourcalcLocalStorage, TourcalcLocalStorage>()
                .AddScoped<EnrichedHttpClient>()
                .AddScoped<ITourRetriever, HttpBasedTourRetriever>()
                //.AddSingleton<TCGlobal>()
                .AddScoped<ITCDataService, TCDataService>()
                .AddScoped<AuthSvc>()
                .AddScoped<TCDataSyncService>()
                .AddScoped<TourcalcEngine>()
                ;*/
            AddTCServices(builder.Services, builder.HostEnvironment.BaseAddress);


            await builder.Build().RunAsync();
        }
        public static void AddTCServices(IServiceCollection svc, string baseAddress)
        {
            svc.AddSingleton<ILocalLogger, LocalLogger>()
                .AddScoped(sp => new HttpClient())
                .AddAntDesign()
                .AddScoped<ISimpleMessageShower, SimpleMessageShower>()
                .AddScoped<ITourcalcLocalStorage, TourcalcLocalStorage>()
                .AddScoped<EnrichedHttpClient>()
                .AddScoped<ITourRetriever, HttpBasedTourRetriever>()
                .AddScoped<ITCDataService, TCDataService>()
                .AddScoped<AuthSvc>()
                .AddScoped<TCDataSyncService>()
                .AddScoped<TourcalcEngine>()
                ;
        }
    }
}