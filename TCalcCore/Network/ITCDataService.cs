using System;
using System.Collections.Generic;
using System.Threading.Tasks;
using TCalc.Domain;
using TCalc.Storage;
using TCalcCore.Auth;

namespace TCalcCore.Network
{
    public interface ITCDataService
    {
        IDataServiceDelegates.OnTourStoredDelegate onTourStored { get; set; }
        IDataServiceDelegates.onserverqstored OnServerQueueStored { get; set; }

        Task AddPerson(string tourId, Person p);
        Task AddSpending(string tourId, Spending s);
        Task AddTour(Tour tour, string code);
        Task ClearLocalCachedTourList();
        Task ClearToken();
        Task DeletePerson(string tourId, Person p);
        Task DeleteSpending(string tourId, Spending s);
        Task DeleteTour(Tour tour);
        Task EditPerson(string tourId, Person p);
        Task EditSpending(string tourId, params Spending[] ss);
        Task EditTourProps(Tour tour, params (string, object)[] operationAndPayload);
        Task GetAndStoreToken(string scope, string code);
        Task GetAndStoreTokenForCodeMd5(string code);
        Task<AuthData> GetAuthData(bool forceGetFromServer = false);
        Task<Queue<SerializableTourOperation>> GetServerQueue(string tourId);
        Task<TourList> GetTourList(Func<TourList, bool, DateTimeOffset, Task> onTourListAvailable, bool forceFromServer);
        Task<TourList> GetTourListFromServer();
        Task<TourList> GetTourVersions(Tour tour);
        Task<Tour> LoadTour(string id, Func<Tour, bool, DateTimeOffset, Task> onTourAvailable, bool forceLoadFromServer = false, bool forceLoadFromLocalStorage = false);
        Task<Tour> LoadTourBare(string id, Func<Tour, bool, DateTimeOffset, Task> onTourAvailable, bool forceLoadFromServer = false, bool forceLoadFromLocalStorage = false);
        Task Sync(string tourId);
        void TriggerStoreLoop(string tourId);
    }
    public class IDataServiceDelegates
    {
        public delegate Task OnTourStoredDelegate(string tourId, bool storedOnServer);
        public delegate Task onserverqstored();
    }
}