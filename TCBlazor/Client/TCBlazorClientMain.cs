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

            var svc = builder.Services;
            svc.UseCommonTourcalcServices();

            // specific to client
            svc.AddScoped(sp => new HttpClient { BaseAddress = new Uri(builder.HostEnvironment.BaseAddress) });
            svc.AddScoped<ITokenStorage, ClientSideCookieTokenStorage>();
            svc.AddScoped<ITourcalcLocalStorage, ClientSideTourcalcLocalStorage>();
            svc.AddSingleton<IPrerenderingContext, ClientSidePrerenderingContext>();

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
                    svc.AddScoped<ITourcalcLocalStorage, ClientSideTourcalcLocalStorage>();
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