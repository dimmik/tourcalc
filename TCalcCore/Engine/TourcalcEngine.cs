using System;
using System.Threading.Tasks;
using TCalc.Domain;
using TCalc.Logic;
using TCalcCore.Auth;
using TCalcCore.Network;
using TCalcCore.Storage;
using TCalcCore.UI;
using static TCalcCore.Engine.TourcalcDelegates;

namespace TCalcCore.Engine
{
    // TODO sort out synchronousity. Sometimes we await, sometimes run the task and forget. Need more consistency
    public class TourcalcEngine
    {
        #region Props, Constructor, Delegates
        private readonly ITCDataService dataSvc;
        private readonly TCDataSyncService tcDataSyncSvc;
        private readonly AuthSvc authSvc;
        private readonly ITourcalcLocalStorage ts;

        public OnTourLoaded onTourLoaded { get; set; }
        public OnTourListLoaded onTourListLoaded { get; set; }


        public TourcalcEngine(ITCDataService dataSvc, AuthSvc authSvc, ITourcalcLocalStorage ts, TCDataSyncService tcDataSyncSvc)
        {
            this.dataSvc = dataSvc ?? throw new ArgumentNullException(nameof(dataSvc));
            this.authSvc = authSvc ?? throw new ArgumentNullException(nameof(authSvc));
            this.ts = ts ?? throw new ArgumentNullException(nameof(ts));
            this.tcDataSyncSvc = tcDataSyncSvc;

            this.dataSvc.onTourStored += OnTourStored;
        }
        #endregion

        #region Messaging
        public OnMessageReceived onMessageReceived { get; set; }
        public void SendMessage(string type, object payload)
        {
            Task.Run(() =>
            {
                onMessageReceived?.Invoke(type, payload);
            });
        }
        #endregion

        #region Load Tour
        public async Task<Tour> LoadFromServerAndReturnBareTour(string tourId)
{
            var t = await dataSvc.LoadTourBare(tourId, (a, aa, aaa) => { return Task.CompletedTask; }, forceLoadFromServer: true);
            return t;
        }
        public async Task RequestTourLoad(string tourId, bool forceLoadFromServer = false, bool forceLoadFromLocalStorage = false)
        {
            _ = await dataSvc.LoadTour(tourId, async (t, isFromServer, dt) => 
            {
                await (onTourLoaded?.Invoke(t?.Calculated(), isFromServer, dt) ?? Task.CompletedTask);
                dataSvc.TriggerStoreLoop(t.Id);
            }
            , forceLoadFromServer
            , forceLoadFromLocalStorage
            );
        }
        #endregion

        #region Data Services
        public TCDataSyncService DataSync => tcDataSyncSvc;
        public ITCDataService DataSvc => dataSvc;
        #endregion

        #region UI settings
        public async Task<UISettings> GetUISettings()
        {
            return (await ts.GetUISettings()).val;
        }
        public async Task SetUISettings(UISettings s)
        {
            await ts.SetUISettings(s);
        }
        private readonly static string whoamiKey = "__WhoAmI";
        public async Task SetWhoAmI(string whoami)
        {
            await ts.Set(whoamiKey, whoami);
        }
        public async Task<string> GetWhoAmI()
        {
            return (await ts.Get(whoamiKey)).val;
        }
        #endregion

        #region auth
        public async Task LogIn(string scope, string code, bool md5Code = false)
        {
            await authSvc.LogIn(scope, code, md5Code);
        }
        public async Task LogOut()
        {
            await authSvc.LogOut();
        }
        public async Task PickUpAuthInfo()
        {
            await authSvc.PickUpAuthInfo();
        }

        public AuthData Auth => authSvc.Auth;
        #endregion

        #region Tour List
        public async Task RequestTourListLoad(bool forceFromServer = false)
        {
            _ = await dataSvc.GetTourList(async (tours, isFresh, dt) =>
            {
                await (onTourListLoaded?.Invoke(tours, isFresh, dt) ?? Task.CompletedTask);
            }
            ,forceFromServer
            );
        }
        public async Task RequestEditTourProps(Tour t, params (string, object)[] operationAndPayload)
        {
            await dataSvc.EditTourProps(t, operationAndPayload);
            // Commented because actually we need to refresh tour list only if tour is stored on server
            // Logic to be implemented on tour list view
            // This dances are all about abstracting store/retrieve operations from http. OMG
            //_ = RequestTourListLoad(forceFromServer: true);
        }
        public async Task RequestAddTour(Tour t, string code)
        {
            await dataSvc.AddTour(t, code);
            _ = RequestTourListLoad(forceFromServer: true);
        }
        public async Task RequestDeleteTour(Tour t)
        {
            await dataSvc.DeleteTour(t);
            _ = RequestTourListLoad(forceFromServer: true);
        }
        public async Task RequestClearLocalTourList(bool reloadFromServer)
        {
            await dataSvc.ClearLocalCachedTourList();
            if (reloadFromServer)
                _ = RequestTourListLoad(forceFromServer: true);
        }
        public async Task<TourList> GetTourVersions(Tour tour)
        {
            return await dataSvc.GetTourVersions(tour);
        }
        #endregion

        #region on tour stored
        private Task OnTourStored(string tourId, bool storedOnServer)
        {
            if (storedOnServer)
            {
                _ = RequestTourLoad(tourId, forceLoadFromServer: true);
            }
            else
            {
                _ = RequestTourLoad(tourId, forceLoadFromServer: false, forceLoadFromLocalStorage: true);
            }
            return Task.CompletedTask;
        }
        #endregion

        #region Persons
        public OnTourPartSubmitting onPersonAddStart;
        public OnTourPartSubmitting onPersonAddFinish;
        public OnTourPartSubmitting onPersonEditStart;
        public OnTourPartSubmitting onPersonEditFinish;
        public OnTourPartSubmitting onPersonDeleteStart;
        public OnTourPartSubmitting onPersonDeleteFinish;

        public async Task RequestAddPerson(string tourId, Person p)
        {
            await (onPersonAddStart?.Invoke() ?? Task.CompletedTask);
            //await Task.Delay(15000);
            await dataSvc.AddPerson(tourId, p);
            await (onPersonAddFinish?.Invoke() ?? Task.CompletedTask);
        }
        public async Task RequestEditPerson(string tourId, Person p)
        {
            await (onPersonEditStart?.Invoke() ?? Task.CompletedTask);
            //await Task.Delay(15000);
            await dataSvc.EditPerson(tourId, p);
            await (onPersonEditFinish?.Invoke() ?? Task.CompletedTask);
        }
        public async Task RequestDeletePerson(string tourId, Person p)
        {
            await (onPersonDeleteStart?.Invoke() ?? Task.CompletedTask);
            //await Task.Delay(15000);
            await dataSvc.DeletePerson(tourId, p);
            await (onPersonDeleteFinish?.Invoke() ?? Task.CompletedTask);
        }
        #endregion

        #region Spendings
        public OnTourPartSubmitting onSpendingAddStart;
        public OnTourPartSubmitting onSpendingAddFinish;
        public OnTourPartSubmitting onSpendingEditStart;
        public OnTourPartSubmitting onSpendingEditFinish;
        public OnTourPartSubmitting onSpendingDeleteStart;
        public OnTourPartSubmitting onSpendingDeleteFinish;

        public async Task RequestAddSpending(string tourId, Spending s)
        {
            await (onSpendingAddStart?.Invoke() ?? Task.CompletedTask);
            await dataSvc.AddSpending(tourId, s);
            await (onSpendingAddFinish?.Invoke() ?? Task.CompletedTask);
        }
        public async Task RequestEditSpending(string tourId, params Spending[] ss)
        {
            await (onSpendingEditStart?.Invoke() ?? Task.CompletedTask);
            await dataSvc.EditSpending(tourId, ss);
            await (onSpendingEditFinish?.Invoke() ?? Task.CompletedTask);
        }
        public async Task RequestDeleteSpending(string tourId, Spending s)
        {
            await (onSpendingDeleteStart?.Invoke() ?? Task.CompletedTask);
            await dataSvc.DeleteSpending(tourId, s);
            await (onSpendingDeleteFinish?.Invoke() ?? Task.CompletedTask);
        }
        #endregion

    }
    static class TourExt
    {
        public static Tour Calculated(this Tour t)
        {
            var calculator = new TourCalculator(t);
            var calculated = calculator.SuggestFinalPayments();
            return calculated;
        }
    }
}
