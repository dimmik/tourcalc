
using System.Linq;
using System.Threading.Tasks;

namespace TCalcCore.Network
{
    public class TCDataSyncService
    {
        public delegate Task OnTourSyncedDelegate();
        private readonly ITCDataService dataSvc;

        public TCDataSyncService(ITCDataService dataSvc)
        {
            this.dataSvc = dataSvc;
        }

        public async Task<int> CountOfUnsyncedOperations(string tourId)
        {
            var q = await dataSvc.GetServerQueue(tourId);
            return q.Where(op => op.Failed).Count();
        }
        public volatile bool IsSyncing = false;
        public async Task Sync(string tourId)
        {
            await dataSvc.Sync(tourId);
        }

    }
}
