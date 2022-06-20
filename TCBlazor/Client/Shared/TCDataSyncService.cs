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
        public async Task Sync(string tourId)
        {
            if (await dataSvc.Sync(tourId))
            {
                OnTourSynced?.Invoke();
            }
        }

    }
}
