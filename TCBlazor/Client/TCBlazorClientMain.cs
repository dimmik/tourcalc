using Microsoft.AspNetCore.Components.Web;
using Microsoft.AspNetCore.Components.WebAssembly.Hosting;
using TCBlazor.Client;
using TCBlazor.Client.Storage;
using TCBlazor.Client.Utils;
using TCBlazor.Client.Shared;

namespace Company.WebApplication1
{
    public class TCBlazorClientMain
    {
        public static async Task Main(string[] args)
        {
            var builder = WebAssemblyHostBuilder.CreateDefault(args);
            builder.RootComponents.Add<App>("#app");
            builder.RootComponents.Add<HeadOutlet>("head::after");

            builder.Services.AddScoped(sp => new HttpClient { BaseAddress = new Uri(builder.HostEnvironment.BaseAddress) });
            builder.Services.AddAntDesign();
            builder.Services.AddSingleton<TourcalcLocalStorage>();
            builder.Services.AddScoped<EnrichedHttpClient>();
            builder.Services.AddSingleton<TCGlobal>();

            await builder.Build().RunAsync();
        }
    }
}