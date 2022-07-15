using Microsoft.AspNetCore.Components.Web;
using Microsoft.AspNetCore.Components.WebAssembly.Hosting;
using TCBlazor.Client;
using TCalcCore.Storage;
using TCalcCore.Network;
using TCalcCore.Logging;
using TCalcCore.UI;
using TCalcCore.Engine;
using TCBlazor.Client.SharedCode;
using TCBlazor.Shared;
using TCalcCore.Auth;
using TCBlazor.Client.Shared;

namespace TCBlazor.Client
{
    public class TCBlazorClientMain
    {
        public static async Task Main(string[] args)
        {
            var builder = WebAssemblyHostBuilder.CreateDefault(args);
            // server-rendered component now
            //builder.RootComponents.Add<App>("#app");
            builder.RootComponents.Add<HeadOutlet>("head::after");

            /*builder.Services
                .AddSingleton<ILocalLogger, LocalLogger>()
                .AddScoped(sp => new HttpClient { BaseAddress = new Uri(builder.HostEnvironment.BaseAddress) })
                .AddAntDesign()
                .AddScoped<ISimpleMessageShower, SimpleMessageShower>()
                .AddScoped<ITokenStorage, CookieTokenStorage>()
                .AddSingleton<ITourcalcLocalStorage, TourcalcLocalStorage>()
                .AddScoped<EnrichedHttpClient>()
                .AddScoped<ITourRetriever, HttpBasedTourRetriever>()
                //.AddSingleton<TCGlobal>()
                .AddScoped<ITCDataService, TCDataService>()
                .AddScoped<AuthSvc>()
                .AddScoped<TCDataSyncService>()
                .AddScoped<TourcalcEngine>()
                ;*/
            AddTCServices(builder.Services, null, new PrerenderingContext(), builder.HostEnvironment.BaseAddress);
            builder.Services.AddScoped<ITokenStorage, CookieTokenStorage>();


            await builder.Build().RunAsync();
        }
        public static void AddTCServices(IServiceCollection svc, ITourcalcLocalStorage? tlsImpl, IPrerenderingContext ctx, string? url)
        {
            svc.AddSingleton<ILocalLogger, LocalLogger>()
                .AddSingleton(ctx);
            if (url == null)
            {
                svc.AddScoped(sp => new HttpClient());
            } else
            {
                svc.AddScoped(sp => new HttpClient { BaseAddress = new Uri(url) });
            }


            svc.AddAntDesign()
                .AddScoped<ISimpleMessageShower, SimpleMessageShower>();
                if (tlsImpl != null) {
                    svc.AddScoped<ITourcalcLocalStorage>((s) => tlsImpl);
                } else
                {
                    svc.AddScoped<ITourcalcLocalStorage, TourcalcLocalStorage>();
                }
                svc
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