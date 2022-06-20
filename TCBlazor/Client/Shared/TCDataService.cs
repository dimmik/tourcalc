using Microsoft.AspNetCore.WebUtilities;
using Newtonsoft.Json;
using System.Text;
using TCalc.Domain;
using TCalc.Logic;
using TCalc.Storage;
using TCalcCore.Auth;
using TCBlazor.Client.Storage;

namespace TCBlazor.Client.Shared
{
    public class TCDataService
    {
        private readonly TourcalcLocalStorage ts;
        private readonly EnrichedHttpClient http;
        private readonly TourStorageProcessor tourStorageProcessor = new TourStorageProcessor();

        public TCDataService(TourcalcLocalStorage ts, EnrichedHttpClient http)
        {
            this.ts = ts ?? throw new ArgumentNullException(nameof(ts));
            this.http = http ?? throw new ArgumentNullException(nameof(http));
        }


        public async Task<AuthData?> GetAuthData(bool forceGetFromServer = false)
        {
            var token = await ts.GetToken();
            AuthData? ad;
            if (forceGetFromServer || !token.Contains('.'))
            {
                ad = await http.CallWithAuthToken<AuthData>("/api/Auth/whoami", token);
            } 
            else
            {
                try
                {
                    var parts = token.Split('.');
                    var meaningful = parts[1];
                    var plain = Encoding.UTF8.GetString(WebEncoders.Base64UrlDecode(meaningful));
                    var authDataContainer = Newtonsoft.Json.JsonConvert.DeserializeObject<AuthDataContainer>(plain);
                    string adStr = (authDataContainer?.AuthDataJson ?? "").Trim();
                    ad = Newtonsoft.Json.JsonConvert.DeserializeObject<AuthData>(adStr);
                    if (ad == null) throw new Exception("cannot get auth info from token");
                } catch
                {
                    ad = await http.CallWithAuthToken<AuthData?>("/api/Auth/whoami", token);
                }
            }
            return ad;
        }

        public async Task GetAndStoreToken(string? scope, string? code)
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
        public async Task GetAndStoreTokenForCodeMd5(string? code)
        {
            var url = $"/api/Auth/token/code/{code ?? CodeThatForSureIsNotUsed}/md5";
            var token = await http.GetStringAsync(url);
            await ts.SetToken(token);
        }

        public async Task<Tour?> LoadTour(string? id, Func<Task> onTourRefreshedFromServer)
        {
            if (id == null) return default;

            var tour = await LoadTourBare(id, onTourRefreshedFromServer);
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
        public async Task<Tour?> LoadTourBare(string? id, Func<Task> onTourRefreshedFromServer, bool forceLoadFromServer = false)
        {
            if (id == null) return default;
            // First get from local storage
            Tour? t = await ts.GetObject<Tour?>(GetTourStorageKey(id));
            if (t != null && !forceLoadFromServer) // found locally and we do not enforce loading from server
            {
                // Then in background - load the tour from server
                Task<Tour?> tourLoadTask = LoadTourFromServerInBackground(id, t, onTourRefreshedFromServer);
                return t;
            } else
            {
                // if it is null, no tour in local storage = well, just load from server
                t = await LoadTourFromServerInBackground(id, t, onTourRefreshedFromServer);
                return t;
            }
        }

        private async Task<Tour?> LoadTourFromServerInBackground(string id, Tour? localTour, Func<Task> onTourRefreshedFromServer)
        {
            var token = await ts.GetToken();
            var t = await http.CallWithAuthToken<Tour>($"/api/Tour/{id}", token);
            if (t != null && t.StateGUID != (localTour?.StateGUID ?? Guid.NewGuid().ToString()))
            {
                // On success, AND if state differs - store the tour and execute onTourRefreshedFromServer
                await ts.SetObject(GetTourStorageKey(id), t);
                await onTourRefreshedFromServer();
            }
            return t;
        }

        public async Task DeleteTour(Tour? tour)
        {
            if (tour == null) return;
            await http.CallWithAuthToken<string>($"/api/Tour/{tour.Id}", await ts.GetToken(), HttpMethod.Delete, null);
        }
        public async Task EditTourProps(Tour? tour, string? operation)
        {
            if (tour == null) return;
            if (operation == null) return;
            await http.CallWithAuthToken<string>($"/api/Tour/{tour.Id}/{operation}", await ts.GetToken(), HttpMethod.Patch, tour);
        }
        private async Task UpdateTour(string? tourId, Tour? tour)
        {
            if (tour == null) return;
            if (tourId == null) return;
            var tid = await http.CallWithAuthToken<string>($"/api/Tour/{tour.Id}", await ts.GetToken(), HttpMethod.Patch, tour);
            if (string.IsNullOrWhiteSpace(tid))
            {
                throw new Exception("wrong tour id returned");
            } else
            {
                // it is updated fine. store locally as well
                await ts.SetObject(GetTourStorageKey(tour.Id), tour);
            }
        }
        public async Task AddTour(Tour? tour, string? code)
        {
            if (tour == null) return;
            await http.CallWithAuthToken<string>($"/api/Tour/add/{code ?? CodeThatForSureIsNotUsed}", await ts.GetToken(), HttpMethod.Post, tour);
        }
        public async Task<TourList?> GetTourList()
        {
            var token = await ts.GetToken();
            // TODO pagination, links, all the stuff
            var from = 0;
            var count = 1000;
            var code = "";
            var tours = await http.CallWithAuthToken<TourList>($"/api/Tour/all/suggested?from={from}&count={count}&code={code}", token);
            return tours;
        }
        // TODO make serializable. Store method name and params instead of action, and proces respectively
        private readonly Dictionary<string, Queue<SerializableTourOperation>> tourServerUpdateQueues = new();
        private readonly Dictionary<string, Queue<SerializableTourOperation>> tourLocalUpdateQueues = new();
        private async Task EditTourData(string tourId, SerializableTourOperation op, Func<Task> onFreshTourLoaded)
        {
            Queue<SerializableTourOperation> serverQueue = await GetServerQueue(tourId);
            serverQueue.Enqueue(op);
            Queue<SerializableTourOperation> localQueue = GetLocalQueue(tourId);
            localQueue.Enqueue(op);

            // update locally
            await UpdateLocally(tourId, localQueue);

            // on server in background task
            _ = UpdateOnServer(tourId, serverQueue, onFreshTourLoaded);
        }

        private Queue<SerializableTourOperation> GetLocalQueue(string tourId)
        {
            if (!tourLocalUpdateQueues.ContainsKey(tourId)) tourLocalUpdateQueues[tourId] = new();
            var localQueue = tourLocalUpdateQueues[tourId];
            return localQueue;
        }

        private async Task<Queue<SerializableTourOperation>> GetServerQueue(string tourId)
        {
            if (!tourServerUpdateQueues.ContainsKey(tourId)) tourServerUpdateQueues[tourId] = new();
            var serverQueue = tourServerUpdateQueues[tourId];
            return serverQueue;
        }
        private async Task StoreServerQueue(string tourId, Queue<SerializableTourOperation> q)
        {
            tourServerUpdateQueues[tourId] = q;
        }

        private async Task UpdateLocally(string tourId, Queue<SerializableTourOperation> q)
        {
            Tour? tour = await ts.GetObject<Tour>(GetTourStorageKey(tourId));
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

        private async Task UpdateOnServer(string tourId, Queue<SerializableTourOperation> q, Func<Task> onFreshTourLoaded)
        {
            // try update on server
            bool updatedOnServer = await TryApplyOnServer(tourId, q);
            if (!updatedOnServer)
            {
                http.ShowError($"Failed to sync: {q.Count}");
            }
            else
            {
                await onFreshTourLoaded();
            }
        }
        private async Task<bool> TryApplyOnServer(string tourId, Queue<SerializableTourOperation> q)
        {
            Tour? tour = await LoadTourBare(tourId, () => { return Task.CompletedTask; }, forceLoadFromServer: true);
            if (tour == null) return false;
            Queue<SerializableTourOperation> updateQueue = q;
            Queue<SerializableTourOperation> backupQueue = new();
            while (updateQueue.TryDequeue(out var op))
            {
                backupQueue.Enqueue(op);
                var proc = op.ApplyOperationFunc(tourStorageProcessor);
                tour = proc(tour);
            }
            try
            {
                await UpdateTour(tour.GUID, tour);
                return true;
            } catch (Exception e)
            {
                // 
                await StoreServerQueue(tourId, backupQueue);
                // debugging. comment out
                http.ShowError($"keeping queue of size {backupQueue.Count} ({e.Message})");
                return false;
            }
        }
        #region Persons
        public async Task DeletePerson(string? tourId, Person? p, Func<Task> onFreshTourLoaded)
        {
            if (tourId == null) return;
            if (p == null) return;
            await EditTourData(tourId, new SerializableTourOperation("DeletePerson", p.GUID, (string?)null), onFreshTourLoaded);
        }
        public async Task EditPerson(string? tourId, Person? p, Func<Task> onFreshTourLoaded)
        {
            if (tourId == null) return;
            if (p == null) return;
            await EditTourData(tourId, new SerializableTourOperation("UpdatePerson", p.GUID, p), onFreshTourLoaded);
        }

        

        public async Task AddPerson(string? tourId, Person? p, Func<Task> onFreshTourLoaded)
        {
            if (tourId == null) return;
            if (p == null) return;
            await EditTourData(tourId, new SerializableTourOperation("AddPerson", null, p), onFreshTourLoaded);
        }
        #endregion
        #region Spendings
        public async Task DeleteSpending(string? tourId, Spending? s, Func<Task> onFreshTourLoaded)
        {
            if (tourId == null) return;
            if (s == null) return;
            await EditTourData(tourId, new SerializableTourOperation("DeleteSpending", s.GUID, (string?)null), onFreshTourLoaded);
        }
        public async Task EditSpending(string? tourId, Spending? s, Func<Task> onFreshTourLoaded)
        {
            if (tourId == null) return;
            if (s == null) return;
            await EditTourData(tourId, new SerializableTourOperation("UpdateSpending", s.GUID, s), onFreshTourLoaded);
        }
        public async Task AddSpending(string? tourId, Spending? s, Func<Task> onFreshTourLoaded)
        {
            if (tourId == null) return;
            if (s == null) return;
            await EditTourData(tourId, new SerializableTourOperation("AddSpending", null, s), onFreshTourLoaded);
        }

        #endregion

    }
    class AuthDataContainer
    {
        public string AuthDataJson { get; set; } = "";
    }
    public class SerializableTourOperation
    {
        public string OperationName { get; set; }
        public string? ItemId { get; set; }
        public string? ItemJson { get; set; }

        public SerializableTourOperation(string operationName, string? itemId, string? itemJson)
        {
            OperationName = operationName;
            ItemId = itemId;
            ItemJson = itemJson;
        }
        public SerializableTourOperation(string operationName, string? itemId, AbstractItem item)
        {
            OperationName = operationName;
            ItemId = itemId;
            ItemJson = JsonConvert.SerializeObject(item);
        }

        public Func<Tour, Tour> ApplyOperationFunc(TourStorageProcessor tourStorageProcessor)
        {
            switch (OperationName)
            {
                case "AddSpending":
                    Spending? sa = JsonConvert.DeserializeObject<Spending>(ItemJson);
                    return t => tourStorageProcessor.AddSpending(t, sa);
                case "EditSpending":
                case "UpdateSpending":
                    Spending? se = JsonConvert.DeserializeObject<Spending>(ItemJson);
                    return t => tourStorageProcessor.UpdateSpending(t, se, ItemId);
                case "DeleteSpending":
                    return t => tourStorageProcessor.DeleteSpending(t, ItemId);
                case "AddPerson":
                    Person? pa = JsonConvert.DeserializeObject<Person>(ItemJson);
                    return t => tourStorageProcessor.AddPerson(t, pa);
                case "EditPerson":
                case "UpdatePerson":
                    Person? pe = JsonConvert.DeserializeObject<Person>(ItemJson);
                    return t => tourStorageProcessor.UpdatePerson(t, pe, ItemId);
                case "DeletePerson":
                    return t => tourStorageProcessor.DeletePerson(t, ItemId);
                default:
                    return t => t;
            }
        }
    }
}
