using TCalcCore.Engine;
using TCalcCore.Logging;
using TCalcCore.Network;
using TCalcCore.Storage;
using TCalcCore.UI;
using TCBlazor.Client.SharedCode;

namespace TCBlazor.Client
{
    public static class TourcalcExt
    {
        public static void UseCommonTourcalcServices(this IServiceCollection svc)
        {
            svc.AddSingleton<ILocalLogger, LocalLogger>();
            svc.AddAntDesign();
            svc.AddScoped<ISimpleMessageShower, SimpleMessageShower>();
            svc.AddScoped<EnrichedHttpClient>();
            svc.AddScoped<ITourRetriever, HttpBasedTourRetriever>();
            svc.AddScoped<ITCDataService, TCDataService>();
            svc.AddScoped<AuthSvc>();
            svc.AddScoped<TCDataSyncService>();
            svc.AddScoped<TourcalcEngine>();
        }
    }
}
