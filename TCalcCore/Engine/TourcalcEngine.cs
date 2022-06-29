using System;
using System.Collections.Generic;
using System.Text;
using System.Threading.Tasks;
using TCalc.Domain;
using TCalcCore.Network;
using static TCalcCore.Engine.TourcalcDelegates;

namespace TCalcCore.Engine
{
    public class TourcalcEngine
    {
        private readonly TCDataService dataSvc;
        public OnTourLoaded onTourLoaded;

        public TourcalcEngine(TCDataService dataSvc)
        {
            this.dataSvc = dataSvc ?? throw new ArgumentNullException(nameof(dataSvc));
        }

        public async Task RequestTourLoad(string tourId, bool forceLoadFromServer = false)
        {
            _ = await dataSvc.LoadTour(tourId, async (t, isFromServer) => 
            {
                await onTourLoaded?.Invoke(t, isFromServer);
            }
            , forceLoadFromServer
            );
        }
    }
}
