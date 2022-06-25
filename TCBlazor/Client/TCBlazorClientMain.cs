using Microsoft.AspNetCore.Components.Web;
using Microsoft.AspNetCore.Components.WebAssembly.Hosting;
using TCBlazor.Client;
using TCBlazor.Client.Storage;
using TCBlazor.Client.Shared;
using TCalcCore.Storage;

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
                .AddSingleton<ITourcalcLocalStorage, TourcalcLocalStorage>()
                .AddScoped<EnrichedHttpClient>()
                .AddSingleton<TCGlobal>()
                .AddScoped<TCDataService>()
                .AddScoped<AuthSvc>()
                .AddScoped<TCDataSyncService>()
                ;



            await builder.Build().RunAsync();
        }
    }
}