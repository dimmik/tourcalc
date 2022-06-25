using Microsoft.AspNetCore.Components.Web;
using Microsoft.AspNetCore.Components.WebAssembly.Hosting;
using TCBlazor.Client;
using TCBlazor.Client.Storage;
using TCBlazor.Client.Shared;
using TCalcCore.Storage;
using TCalcCore.Network;

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
                .AddSingleton<LocalLogger>()
                .AddScoped(sp => new HttpClient { BaseAddress = new Uri(builder.HostEnvironment.BaseAddress) })
                .AddAntDesign()
                .AddScoped<SimpleMessageShower>()
                .AddSingleton<ITourcalcLocalStorage, TourcalcLocalStorage>()
                .AddScoped<IEnrichedHttpClient, EnrichedHttpClient>()
                .AddSingleton<TCGlobal>()
                .AddScoped<TCDataService>()
                .AddScoped<AuthSvc>()
                .AddScoped<TCDataSyncService>()
                ;



            await builder.Build().RunAsync();
        }
    }
}