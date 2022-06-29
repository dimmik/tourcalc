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
        public OnTourListLoaded onTourListLoaded;


        public TourcalcEngine(TCDataService dataSvc)
        {
            this.dataSvc = dataSvc ?? throw new ArgumentNullException(nameof(dataSvc));
        }
        public async Task RequestTourLoad(string tourId, bool forceLoadFromServer = false, bool forceLoadFromLocalStorage = false)
        {
            _ = await dataSvc.LoadTour(tourId, async (t, isFromServer, dt) => 
            {
                await (onTourLoaded?.Invoke(t, isFromServer, dt) ?? Task.CompletedTask);
            }
            , forceLoadFromServer
            , forceLoadFromLocalStorage
            );
        }


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
        public async Task RequestEditTourProps(Tour t, string action)
        {
            await dataSvc.EditTourProps(t, action);
            _ = RequestTourListLoad(forceFromServer: true);
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
        #endregion
        #region on tour stored
        private void OnTourStored(string tourId, bool storedOnServer)
        {
            if (storedOnServer)
            {
                _ = RequestTourLoad(tourId, forceLoadFromServer: true);
            }
            else
            {
                _ = RequestTourLoad(tourId, forceLoadFromServer: false, forceLoadFromLocalStorage: true);
            }
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
            await dataSvc.AddPerson(tourId, p, (srv) => {
                    OnTourStored(tourId, srv);
                    return Task.CompletedTask;
                });
            await (onPersonAddFinish?.Invoke() ?? Task.CompletedTask);
        }
        public async Task RequestEditPerson(string tourId, Person p)
        {
            await (onPersonEditStart?.Invoke() ?? Task.CompletedTask);
            //await Task.Delay(15000);
            await dataSvc.EditPerson(tourId, p, (srv) => {
                OnTourStored(tourId, srv);
                return Task.CompletedTask;
            });
            await (onPersonEditFinish?.Invoke() ?? Task.CompletedTask);
        }
        public async Task RequestDeletePerson(string tourId, Person p)
        {
            await (onPersonDeleteStart?.Invoke() ?? Task.CompletedTask);
            //await Task.Delay(15000);
            await dataSvc.DeletePerson(tourId, p, (srv) => {
                OnTourStored(tourId, srv);
                return Task.CompletedTask;
            });
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
            await dataSvc.AddSpending(tourId, s, (srv) => {
                OnTourStored(tourId, srv);
                return Task.CompletedTask;
            });
            await (onSpendingAddFinish?.Invoke() ?? Task.CompletedTask);
        }
        public async Task RequestEditSpending(string tourId, Spending s)
        {
            await (onSpendingEditStart?.Invoke() ?? Task.CompletedTask);
            await dataSvc.EditSpending(tourId, s, (srv) => {
                OnTourStored(tourId, srv);
                return Task.CompletedTask;
            });
            await (onSpendingEditFinish?.Invoke() ?? Task.CompletedTask);
        }
        public async Task RequestDeleteSpending(string tourId, Spending s)
        {
            await (onSpendingDeleteStart?.Invoke() ?? Task.CompletedTask);
            await dataSvc.DeleteSpending(tourId, s, (srv) => {
                OnTourStored(tourId, srv);
                return Task.CompletedTask;
            });
            await (onSpendingDeleteFinish?.Invoke() ?? Task.CompletedTask);
        }
        #endregion

    }
}
