using Microsoft.AspNetCore.WebUtilities;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Net.Http;
using System.Text;
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
        }


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
                } catch
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

        public async Task<Tour> LoadTour(string id, Func<Tour, bool, DateTimeOffset, Task> onTourAvailable, bool forceLoadFromServer = false, bool forceLoadFromLocalStorage = false)
        {
            if (id == null) return default;

            var tour = await LoadTourBare(id, onTourAvailable, forceLoadFromServer, forceLoadFromLocalStorage);
            if (tour == null) return null;
            var calculator = new TourCalculator(tour);
            var calculated = calculator.SuggestFinalPayments();
            return calculated;
            //return tour;
        }
        private static string GetTourStorageKey(string tourId)
        {
            return $"__tour_{tourId}";
        }
        private static string GetUpdateQueueStorageKey(string tourId)
        {
            return $"__update_q_{tourId}";
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
            } else
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
        private async Task UpdateTour(string tourId, Tour tour)
        {
            if (tour == null) return;
            if (tourId == null) return;
            var tid = await http.CallWithAuthToken<string>($"/api/Tour/{tour.Id}", (await ts.GetToken()).val, new HttpMethod("PATCH"), tour);
            if (string.IsNullOrWhiteSpace(tid))
            {
                throw new Exception("wrong tour id returned");
            } else
            {
                // it is updated fine. store locally as well
                await ts.SetObject(GetTourStorageKey(tour.Id), tour);
            }
        }
        public async Task AddTour(Tour tour, string code)
        {
            if (tour == null) return;
            await http.CallWithAuthToken<string>($"/api/Tour/add/{code ?? CodeThatForSureIsNotUsed}", (await ts.GetToken()).val, HttpMethod.Post, tour);
        }
        public async Task ClearTourList()
        {
            await ts.SetObject<TourList>(GetTourListStorageKey(), null);
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
            _ = Task.Run(async () => {
                var tli = await GetTourListFromServer();
                if (tli != null)
                {
                    await ts.SetObject(GetTourListStorageKey(), tli);
                    await onTourListAvailable(tli, true, DateTimeOffset.Now);
                }
            });
            return tl;
        }

        private string GetTourListStorageKey()
        {
            return "__tour_list";
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

        private readonly Dictionary<string, Queue<SerializableTourOperation>> tourLocalUpdateQueues = new Dictionary<string, Queue<SerializableTourOperation>>();
        private async Task EditTourData(string tourId, SerializableTourOperation op, Func<bool, Task> onTourStored)
        {
            Queue<SerializableTourOperation> serverQueue = await GetServerQueue(tourId);
            serverQueue.Enqueue(op);
            Queue<SerializableTourOperation> localQueue = GetLocalQueue(tourId);
            localQueue.Enqueue(op);

            // update locally
            await UpdateLocally(tourId, localQueue); 
            await onTourStored(false); // locally

            // on server in background task
            _ = UpdateOnServer(tourId, serverQueue, onTourStored);
        }

        private Queue<SerializableTourOperation> GetLocalQueue(string tourId)
        {
            if (!tourLocalUpdateQueues.ContainsKey(tourId)) tourLocalUpdateQueues[tourId] = new Queue<SerializableTourOperation>();
            var localQueue = tourLocalUpdateQueues[tourId];
            return localQueue;
        }

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
        public delegate Task onserverqstored();
        public onserverqstored OnServerQueueStored;
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

        public async Task<bool> Sync(string tourId)
        {
            var q = await GetServerQueue(tourId);
            bool ok = await TryApplyOnServer(tourId, q);
            return ok;
        }

        private async Task UpdateOnServer(string tourId, Queue<SerializableTourOperation> q, Func<bool, Task> onTourStored)
        {
            // try update on server
            bool updatedOnServer = await TryApplyOnServer(tourId, q);
            if (!updatedOnServer)
            {
                messageShower.ShowError($"Failed to sync: {q.Count} ops. Will sync once connection is restored.");
            }
            else
            {
                await onTourStored(true);
            }
        }
        private async Task<bool> TryApplyOnServer(string tourId, Queue<SerializableTourOperation> q)
        {
            if (q == null || q.Count == 0) return false;
            Tour tour = await LoadTourBare(tourId, (a, aa, aaa) => { return Task.CompletedTask; }, forceLoadFromServer: true);
            if (tour == null)
            {
                await StoreServerQueue(tourId, q);
                //http.ShowError("stored queue after tour is not loaded");
                logger.Log("stored queue after tour is not loaded");
                return false;
            }
            Queue<SerializableTourOperation> updateQueue = q;
            // debugging. comment out
            //http.ShowError($"update queue of size {updateQueue.Count}");
            Queue<SerializableTourOperation> backupQueue = new Queue<SerializableTourOperation>();
            
            try
            {
                while (updateQueue.TryDequeue(out var op))
                {
                    backupQueue.Enqueue(op);
                    var proc = op.ApplyOperationFunc(tourStorageProcessor);
                    try
                    {
                        if (tour != null) tour = proc(tour);
                    } catch (Exception e)
                    {
                        messageShower.ShowError($"(tour is null: {tour == null}) Failed to apply {op.OperationName} (id: '{op.ItemId ?? "n/a"}'): {e.Message}. Skipping");
                    }
                }
                await UpdateTour(tour?.GUID, tour);
                //http.ShowError($"NOW update queue of size {updateQueue.Count}");
                await StoreServerQueue(tourId, updateQueue); // should be empty here, so keep it empty
                return true;
            } catch (Exception)
            {
                // 
                //http.ShowError($"Exception: NOW update queue of size {updateQueue.Count} ({e.Message})");
                await StoreServerQueue(tourId, backupQueue);
                // debugging. comment out
                //http.ShowError($"keeping queue of size {backupQueue.Count} ({e.Message})");
                return false;
            }
        }
        #region Persons
        public async Task DeletePerson(string tourId, Person p, Func<bool, Task> onTourStored)
        {
            if (tourId == null) return;
            if (p == null) return;
            await EditTourData(tourId, new SerializableTourOperation("DeletePerson", p.GUID, (string)null), onTourStored);
        }
        public async Task EditPerson(string tourId, Person p, Func<bool, Task> onTourStored)
        {
            if (tourId == null) return;
            if (p == null) return;
            await EditTourData(tourId, new SerializableTourOperation("UpdatePerson", p.GUID, p), onTourStored);
        }
        public async Task AddPerson(string tourId, Person p, Func<bool, Task> onTourStored)
        {
            if (tourId == null) return;
            if (p == null) return;
            await EditTourData(tourId, new SerializableTourOperation("AddPerson", null, p), onTourStored);
        }
        #endregion
        #region Spendings
        public async Task DeleteSpending(string tourId, Spending s, Func<bool, Task> onTourStored)
        {
            if (tourId == null) return;
            if (s == null) return;
            await EditTourData(tourId, new SerializableTourOperation("DeleteSpending", s.GUID, (string)null), onTourStored);
        }
        public async Task EditSpending(string tourId, Spending s, Func<bool, Task> onTourStored)
        {
            if (tourId == null) return;
            if (s == null) return;
            await EditTourData(tourId, new SerializableTourOperation("UpdateSpending", s.GUID, s), onTourStored);
        }
        public async Task AddSpending(string tourId, Spending s, Func<bool, Task> onTourStored)
        {
            if (tourId == null) return;
            if (s == null) return;
            await EditTourData(tourId, new SerializableTourOperation("AddSpending", null, s), onTourStored);
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
