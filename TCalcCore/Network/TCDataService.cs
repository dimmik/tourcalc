using Microsoft.AspNetCore.WebUtilities;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Net.Http;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using TCalc.Domain;
using TCalc.Logic;
using TCalc.Storage;
using TCalcCore.Auth;
using TCalcCore.Logging;
using TCalcCore.Storage;
using TCalcCore.UI;

namespace TCalcCore.Network
{
    public class TCDataService
    {
        #region Constructor and properties
        private readonly ITourcalcLocalStorage ts;
        private readonly EnrichedHttpClient http;
        private readonly ISimpleMessageShower messageShower;
        private readonly ITourStorageProcessor tourStorageProcessor = new TourStorageProcessor();
        private readonly ILocalLogger logger;

        public TCDataService(ITourcalcLocalStorage ts, EnrichedHttpClient http, ISimpleMessageShower messageShower, ILocalLogger logger)
        {
            this.ts = ts ?? throw new ArgumentNullException(nameof(ts));
            this.http = http ?? throw new ArgumentNullException(nameof(http));
            this.logger = logger;
            this.messageShower = messageShower;

            // ensure not null
            onTourStored += (a, aa) => Task.CompletedTask;
        }
        #endregion

        #region Misc keys, delegates, etc...
        public delegate Task OnTourStoredDelegate(string tourId, bool storedOnServer);
        public OnTourStoredDelegate onTourStored;
        public delegate Task onserverqstored();
        public onserverqstored OnServerQueueStored;

        private static string GetTourStorageKey(string tourId)
        {
            return $"__tour_{tourId}";
        }
        private static string GetUpdateQueueStorageKey(string tourId)
        {
            return $"__update_q_{tourId}";
        }
        public async Task ClearLocalCachedTourList()
        {
            await ts.SetObject<TourList>(GetTourListStorageKey(), null);
        }
        private string GetTourListStorageKey()
        {
            return "__tour_list";
        }
        #endregion

        #region Authentication and Authorization
        public async Task<AuthData> GetAuthData(bool forceGetFromServer = false)
        {
            var token = await ts.GetToken();
            AuthData ad;
            if (forceGetFromServer || !token.val.Contains("."))
            {
                ad = await http.CallWithAuthToken<AuthData>("/api/Auth/whoami", token.val);
            }
            else
            {
                try
                {
                    var parts = token.val.Split('.');
                    var meaningful = parts[1];
                    var plain = Encoding.UTF8.GetString(WebEncoders.Base64UrlDecode(meaningful));
                    var authDataContainer = Newtonsoft.Json.JsonConvert.DeserializeObject<AuthDataContainer>(plain);
                    string adStr = (authDataContainer?.AuthDataJson ?? "").Trim();
                    ad = Newtonsoft.Json.JsonConvert.DeserializeObject<AuthData>(adStr);
                    if (ad == null) throw new Exception("cannot get auth info from token");
                }
                catch
                {
                    ad = await http.CallWithAuthToken<AuthData>("/api/Auth/whoami", token.val);
                }
            }
            return ad;
        }

        public async Task GetAndStoreToken(string scope, string code)
        {
            var url = $"/api/Auth/token/{scope ?? "code"}/{code ?? CodeThatForSureIsNotUsed}";
            var token = await http.GetStringAsync(url);
            await ts.SetToken(token);
        }
        public async Task ClearToken()
        {
            await ts.SetToken("");
        }
        private static readonly string CodeThatForSureIsNotUsed = "__trashNoTours__";
        public async Task GetAndStoreTokenForCodeMd5(string code)
        {
            var url = $"/api/Auth/token/code/{code ?? CodeThatForSureIsNotUsed}/md5";
            var token = await http.GetStringAsync(url);
            await ts.SetToken(token);
        }
        #endregion

        #region Tour Loading
        public async Task<Tour> LoadTour(string id, Func<Tour, bool, DateTimeOffset, Task> onTourAvailable, bool forceLoadFromServer = false, bool forceLoadFromLocalStorage = false)
        {
            if (id == null) return default;

            var tour = await LoadTourBare(id, onTourAvailable, forceLoadFromServer, forceLoadFromLocalStorage);
            if (tour == null) return null;
            var calculator = new TourCalculator(tour);
            var calculated = calculator.SuggestFinalPayments();
            return calculated;
        }
        /// <summary>
        /// 
        /// </summary>
        /// <param name="id"></param>
        /// <param name="onTourAvailable"></param>
        /// <param name="forceLoadFromServer"> ignored if forceLoadFromLocalStorage == true</param>
        /// <param name="forceLoadFromLocalStorage"></param>
        /// <returns></returns>
        public async Task<Tour> LoadTourBare(string id, Func<Tour, bool, DateTimeOffset, Task> onTourAvailable, bool forceLoadFromServer = false, bool forceLoadFromLocalStorage = false)
        {
            if (id == null) return default;
            // First get from local storage
            var (t, dt) = await ts.GetObject<Tour>(GetTourStorageKey(id));
            if (forceLoadFromLocalStorage)
            {
                await onTourAvailable(t, false, dt);
                return t;
            }
            if (t != null && !forceLoadFromServer) // found locally and we do not enforce loading from server
            {
                await onTourAvailable(t, false, dt);
                // Then in background - load the tour from server
                _ = LoadTourFromServerInBackground(id, t, onTourAvailable);
                return t;
            }
            else
            {
                // if it is null, no tour in local storage = well, just load from server
                t = await LoadTourFromServerInBackground(id, t, onTourAvailable);
                return t;
            }
        }

        private async Task<Tour> LoadTourFromServerInBackground(string id, Tour localTour, Func<Tour, bool, DateTimeOffset, Task> onTourAvailable)
        {
            var token = await ts.GetToken();
            var t = await http.CallWithAuthToken<Tour>($"/api/Tour/{id}", token.val, showErrorMessages: false);
            if (t != null
                // && t.StateGUID != (localTour?.StateGUID ?? Guid.NewGuid().ToString()) // should we actually compare state ids? older ones all with empty state ids; on change in legacy - state id will become empty.
                )
            {
                // On success (??? AND if state differs) - store the tour and execute onTourAvailable
                await ts.SetObject(GetTourStorageKey(id), t);
                await onTourAvailable(t, true, DateTimeOffset.Now);
            }
            return t;
        }
        #endregion

        #region Tour List and Tour Property operations (including add/delete)
        public async Task DeleteTour(Tour tour)
        {
            if (tour == null) return;
            await http.CallWithAuthToken<string>($"/api/Tour/{tour.Id}", (await ts.GetToken()).val, HttpMethod.Delete, null);
        }
        public async Task EditTourProps(Tour tour, string operation)
        {
            if (tour == null) return;
            if (operation == null) return;
            await http.CallWithAuthToken<string>($"/api/Tour/{tour.Id}/{operation}", (await ts.GetToken()).val, new HttpMethod("PATCH"), tour);
        }
        public async Task AddTour(Tour tour, string code)
        {
            if (tour == null) return;
            await http.CallWithAuthToken<string>($"/api/Tour/add/{code ?? CodeThatForSureIsNotUsed}", (await ts.GetToken()).val, HttpMethod.Post, tour);
        }
        public async Task<TourList> GetTourList(Func<TourList, bool, DateTimeOffset, Task> onTourListAvailable, bool forceFromServer)
        {
            if (forceFromServer)
            {
                var tli = await GetTourListFromServer();
                if (tli != null)
                {
                    await ts.SetObject(GetTourListStorageKey(), tli);
                    await onTourListAvailable(tli, true, DateTimeOffset.Now);
                }
            }
            var (tl, dt) = await ts.GetObject<TourList>(GetTourListStorageKey());
            if (tl == null)
            {
                tl = await GetTourListFromServer();
                await ts.SetObject(GetTourListStorageKey(), tl);
                await onTourListAvailable(tl, true, DateTimeOffset.Now);
                return tl;
            }
            await onTourListAvailable(tl, false, dt);
            _ = Task.Run(async () =>
            {
                var tli = await GetTourListFromServer();
                if (tli != null)
                {
                    await ts.SetObject(GetTourListStorageKey(), tli);
                    await onTourListAvailable(tli, true, DateTimeOffset.Now);
                }
            });
            return tl;
        }
        public async Task<TourList> GetTourListFromServer()
        {
            var token = await ts.GetToken();
            // TODO pagination, links, all the stuff
            var from = 0;
            var count = 1000;
            var code = "";
            var tours = await http.CallWithAuthToken<TourList>($"/api/Tour/all/suggested?from={from}&count={count}&code={code}", token.val, showErrorMessages: true);
            return tours;
        }
        #endregion

        #region Store Tour Loop
        private readonly Dictionary<string, Task> storeLoopTasks = new Dictionary<string, Task>();
        private volatile CancellationTokenSource storeWaitCts = new CancellationTokenSource();
        public volatile CancellationTokenSource storeLoopCts = new CancellationTokenSource();
        private TimeSpan storeInterval = TimeSpan.FromSeconds(25);
        public void TriggerStoreLoop(string tourId)
        {
            Task storeLoopTask = storeLoopTasks.ContainsKey(tourId) ? storeLoopTasks[tourId] : Task.CompletedTask;
            // if loop is not running - run it
            if (storeLoopTask == null || storeLoopTask.IsCompleted || storeLoopTask.IsFaulted)
            {
                storeLoopTask = StartStoreLoop(tourId);
                storeLoopTasks[tourId] = storeLoopTask;
            }
            storeWaitCts.Cancel();
        }

        private async Task StartStoreLoop(string tourId)
        {
            bool doLoop = true;
            while (!storeLoopCts.IsCancellationRequested && doLoop)
            {
                _ = await WaitSomeTime(storeInterval);
                //logger.Log($"({tourId}) Wait completed ({storeInterval}). Interrupted: {interrupted}");
                var storeQ = await GetServerQueue(tourId);
                if (storeQ.Count == 0)
                {
                    doLoop = false;
                    logger.Log($"({tourId}) stop the store loop due to queue is empty");
                }
                else
                {
                    //logger.Log($"({tourId}) stored queue {storeQ.Count}");
                    bool updated = await TryApplyOnServer(tourId, storeQ);
                    //logger.Log($"({tourId}) updated. Success? {updated}");
                    if (updated)
                    {
                        await onTourStored(tourId, storedOnServer: true);
                        doLoop = false;
                        logger.Log($"({tourId}) stop the store loop after successful store");
                    }
                }
            }
        }
        private static int settingCtsIsInProgress = 0;
        private async Task<bool> WaitSomeTime(TimeSpan interval)
        {
            try
            {
                await Task.Delay(interval, storeWaitCts.Token);
                return false;
            }
            catch // requested a store event
            {
                if (0 == Interlocked.Exchange(ref settingCtsIsInProgress, 1)) // is not used by other thread
                {
                    //logger.Log("Lock acquired");
                    if (storeWaitCts.IsCancellationRequested)
                    {
                        var newCts = new CancellationTokenSource();
                        Interlocked.Exchange(ref storeWaitCts, newCts);
                    }
                    // release using
                    Interlocked.Exchange(ref settingCtsIsInProgress, 0);
                }
                else
                {
                    logger.Log("Lock NOT acquired. Do nothing");
                }
            }
            return true;
        }
        #endregion

        #region Tour editing : add/remove/edit spendings and persons
        #region Local Queue
        private readonly Dictionary<string, Queue<SerializableTourOperation>> tourLocalUpdateQueues = new Dictionary<string, Queue<SerializableTourOperation>>();
        private Queue<SerializableTourOperation> GetLocalQueue(string tourId)
        {
            if (!tourLocalUpdateQueues.ContainsKey(tourId)) tourLocalUpdateQueues[tourId] = new Queue<SerializableTourOperation>();
            var localQueue = tourLocalUpdateQueues[tourId];
            return localQueue;
        }
        #endregion

        #region Server Queue
        public async Task<Queue<SerializableTourOperation>> GetServerQueue(string tourId)
        {
            SerializableTourOperationContainer qc = (await ts.GetObject<SerializableTourOperationContainer>(GetUpdateQueueStorageKey(tourId))).val;
            if (qc == null)
            {
                //http.ShowError("q is null");
                qc = new SerializableTourOperationContainer();
            }
            Queue<SerializableTourOperation> q = new Queue<SerializableTourOperation>(qc.operations);
            return q;
        }
        private async Task StoreServerQueue(string tourId, Queue<SerializableTourOperation> q)
        {
            //http.ShowError($"storing queue of size {q.Count} -- {new StackTrace(true)}");
            SerializableTourOperationContainer qc = new SerializableTourOperationContainer()
            {
                operations = q.ToList()
            };
            await ts.SetObject(GetUpdateQueueStorageKey(tourId), qc);
            OnServerQueueStored?.Invoke();
        }
        #endregion
        private async Task UpdateTour(string tourId, Tour tour)
        {
            if (tour == null) return;
            if (tourId == null) return;
            var tid = await http.CallWithAuthToken<string>($"/api/Tour/{tour.Id}", (await ts.GetToken()).val, new HttpMethod("PATCH"), tour);
            if (string.IsNullOrWhiteSpace(tid))
            {
                throw new Exception("wrong tour id returned");
            }
            else
            {
                // it is updated fine. store locally as well
                await ts.SetObject(GetTourStorageKey(tour.Id), tour);
            }
        }

        private async Task EditTourData(string tourId, SerializableTourOperation op)
        {
            Queue<SerializableTourOperation> serverQueue = await GetServerQueue(tourId);
            serverQueue.Enqueue(op);
            await StoreServerQueue(tourId, serverQueue);

            Queue<SerializableTourOperation> localQueue = GetLocalQueue(tourId);
            localQueue.Enqueue(op);

            // update locally
            await UpdateLocally(tourId, localQueue);
            await onTourStored(tourId, storedOnServer: false);

            TriggerStoreLoop(tourId);
        }



        private async Task UpdateLocally(string tourId, Queue<SerializableTourOperation> q)
        {
            Tour tour = (await ts.GetObject<Tour>(GetTourStorageKey(tourId))).val;
            if (tour != null)
            {
                Queue<SerializableTourOperation> localUpdateQueue = q;
                while (localUpdateQueue.TryDequeue(out var op))
                {
                    var proc = op.ApplyOperationFunc(tourStorageProcessor);
                    tour = proc(tour);
                }
                await ts.SetObject(GetTourStorageKey(tourId), tour);
            }
        }

        public Task Sync(string tourId)
        {
            TriggerStoreLoop(tourId);
            return Task.CompletedTask;
        }

        private async Task<bool> TryApplyOnServer(string tourId, Queue<SerializableTourOperation> q)
        {
            if (q == null || q.Count == 0) return false;
            Tour tour = await LoadTourBare(tourId, (a, aa, aaa) => { return Task.CompletedTask; }, forceLoadFromServer: true);
            if (tour == null)
            {
                foreach (var op in q)
                {
                    op.Failed = true;
                }
                await StoreServerQueue(tourId, q);
                logger.Log("stored queue after tour is not loaded");
                return false;
            }
            Queue<SerializableTourOperation> updateQueue = q;
            Queue<SerializableTourOperation> backupQueue = new Queue<SerializableTourOperation>();
            
            try
            {
                while (updateQueue.TryDequeue(out var op))
                {
                    op.Failed = true;
                    backupQueue.Enqueue(op);
                    var proc = op.ApplyOperationFunc(tourStorageProcessor);
                    try
                    {
                        if (tour != null)
                        {
                            tour = proc(tour);
                        }
                    } catch (Exception e)
                    {
                        messageShower.ShowError($"(tour is null: {tour == null}) Failed to apply {op.OperationName} (id: '{op.ItemId ?? "n/a"}'): {e.Message}. Skipping");
                    }
                }
                await UpdateTour(tour?.GUID, tour);
                await StoreServerQueue(tourId, updateQueue); // should be empty here, so keep it empty
                return true;
            } catch (Exception)
            {
                await StoreServerQueue(tourId, backupQueue);
                return false;
            }
        }
        #endregion

        #region Persons
        public async Task DeletePerson(string tourId, Person p)
        {
            if (tourId == null) return;
            if (p == null) return;
            await EditTourData(tourId, new SerializableTourOperation("DeletePerson", p.GUID, (string)null));
        }
        public async Task EditPerson(string tourId, Person p)
        {
            if (tourId == null) return;
            if (p == null) return;
            await EditTourData(tourId, new SerializableTourOperation("UpdatePerson", p.GUID, p));
        }
        public async Task AddPerson(string tourId, Person p)
        {
            if (tourId == null) return;
            if (p == null) return;
            await EditTourData(tourId, new SerializableTourOperation("AddPerson", null, p));
        }
        #endregion

        #region Spendings
        public async Task DeleteSpending(string tourId, Spending s)
        {
            if (tourId == null) return;
            if (s == null) return;
            await EditTourData(tourId, new SerializableTourOperation("DeleteSpending", s.GUID, (string)null));
        }
        public async Task EditSpending(string tourId, Spending s)
        {
            if (tourId == null) return;
            if (s == null) return;
            await EditTourData(tourId, new SerializableTourOperation("UpdateSpending", s.GUID, s));
        }
        public async Task AddSpending(string tourId, Spending s)
        {
            if (tourId == null) return;
            if (s == null) return;
            await EditTourData(tourId, new SerializableTourOperation("AddSpending", null, s));
        }

        #endregion

    }
    class AuthDataContainer
    {
        public string AuthDataJson { get; set; } = "";
    }
    static class QueueExt
    {
        public static bool TryDequeue<T>(this Queue<T> q, out T elem)
        {
            if (q == null) // ???? or maybe throw an exception?..
            {
                elem = default;
                return false;
            }
            if (q.Count == 0)
            {
                elem = default;
                return false;
            }
            elem = q.Dequeue();
            return true;
        }
    }
}
