using TCalcCore.Network;

namespace TCBlazor.Client.Shared
{
    public class TCDataSyncService
    {
        public delegate Task OnTourSyncedDelegate();
        public event OnTourSyncedDelegate? OnTourSynced;
        private readonly TCDataService dataSvc;

        public TCDataSyncService(TCDataService dataSvc)
        {
            this.dataSvc = dataSvc;
        }

        public async Task<int> CountOfUnsyncedOperations(string tourId)
        {
            var q = await dataSvc.GetServerQueue(tourId);
            return q.Count;
        }
        public bool isSyncing { get; set; } = false;
        public async Task Sync(string tourId)
        {
            if (!isSyncing) // well, naive locking...
            {
                isSyncing = true;
                try
                {
                    if (await dataSvc.Sync(tourId))
                    {
                        OnTourSynced?.Invoke();
                    }
                } finally
                {
                    isSyncing = false;
                }
            }
        }

    }
}
