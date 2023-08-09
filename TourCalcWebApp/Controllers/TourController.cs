using System;
using System.Collections.Generic;
using System.Linq;
using TCalc.Domain;
using TCalc.Storage;
using Microsoft.AspNetCore.Mvc;
using Microsoft.AspNetCore.Authorization;
using TCalc.Logic;
using TourCalcWebApp.Auth;
using TourCalcWebApp.Exceptions;
using System.Linq.Expressions;
using TCalcStorage.Storage.LiteDB;
using TCalcCore.Auth;
using System.Threading.Tasks;

namespace TourCalcWebApp.Controllers
{
    [Route("api/[controller]")]
    [Authorize]
    [AllowAnonymous]
    [ApiController]
    public class TourController : ControllerBase
    {

        private readonly ITcConfiguration Configuration;
        private readonly ITourStorage tourStorage;

        private readonly ITourStorageProcessor tourStorageProcessor = new TourStorageProcessor();

        public TourController(ITcConfiguration config, ITourStorage storage)
        {
            Configuration = config;
            tourStorage = storage;
        }
        #region Tours
        /// <summary>
        /// Tour with given id
        /// </summary>
        /// <param name="tourid">id of the tour</param>
        /// <returns>Tour with given ID</returns>
        [HttpGet("{tourid}")]
        public Tour GetTour(string tourid)
        {
            var tour = TourStorageUtilities_LoadFromStoragebyId(tourid);
            if (tour == null)
            {
                throw HttpException.NotFound($"No tour with id={tourid}");
            }
            // TODO remove, for debugging purposes only
            // Task.Delay(20000).Wait();
            return tour;
        }
        /// <summary>
        /// Calculated (i.e. with persons' spent and persons' received fields filled) tour
        /// </summary>
        /// <param name="tourid">id of the tour</param>
        /// <returns>calculated tour</returns>
        //[HttpGet("{tourid}/calculated")]
        private Tour GetTourCalculated(string tourid)
        {
            var tour = TourStorageUtilities_LoadFromStoragebyId(tourid);
            if (tour == null) throw HttpException.NotFound($"no tour with id {tourid}");
            var calculator = new TourCalculator(tour);
            var calculated = calculator.Calculate();
            return calculated;
        }

        /// <summary>
        /// Versions of the tour
        /// </summary>
        /// <param name="tourid">id of the tour</param>
        /// <param name="from">Default 0</param>
        /// <param name="count">Number of tours to return, default 50</param>
        /// <returns>calculated tour</returns>
        [HttpGet("{tourid}/versions")]
        public TourList GetTourVersions(string tourid, [FromQuery] int from = 0, [FromQuery] int count = 50)
        {
            AuthData authData = AuthHelper.GetAuthData(User, Configuration);
            Expression<Func<Tour, bool>> predicate;
            if (authData.IsMaster)
            {
                predicate = t => true;
            } else
            {
                predicate = t => (t.AccessCodeMD5 != null && authData.AccessCodeMD5s().Contains(t.AccessCodeMD5));
            }
            var tours = tourStorage.GetTourVersions(
                predicate
                , tourid
                , from
                , count
                , out var totalCount
                ).ToArray();
            foreach (var t in tours){
               t.Persons.Clear();
               t.Spendings.Clear();
            }
            var versions = new TourList()
            {
                Tours = tours,
                From = from,
                RequestedCount = count,
                Count = tours.Length,
                TotalCount = totalCount
            };
            return versions;
        }

        /// <summary>
        /// Tour calculated AND with suggested spendings to close the tour (i.e. so that each persons' debt equal to zero)
        /// *This one is used in the SPA*
        /// </summary>
        /// <param name="tourid">id of the tour</param>
        /// <returns>calculated tour with suggestions</returns>
        //[HttpGet("{tourid}/suggested")]
        private Tour GetTourSuggested(string tourid)
        {
            var tour = TourStorageUtilities_LoadFromStoragebyId(tourid);
            if (tour == null) throw HttpException.NotFound($"no tour with id {tourid}");
            var calculator = new TourCalculator(tour);
            var calculated = calculator.SuggestFinalPayments();
            return calculated;
        }
        /// <summary>
        /// All tours available for a user
        /// </summary>
        /// <param name="from">Default 0</param>
        /// <param name="count">Number of tours to return, default 50</param>
        /// <param name="code">(valid for admin only) code to filter on</param>
        /// <returns>List of tours</returns>
        //[HttpGet]
        private TourList GetAllTours([FromQuery] int from = 0, [FromQuery] int count = 50, [FromQuery] string code = "")
        {
            var tours = TourStorageUtilities_LoadAllTours(from, count, code);
            return tours;
        }

        /// <summary>
        /// All tours available for a user, with suggested payments calculated
        /// </summary>
        /// <param name="from">Default 0</param>
        /// <param name="count">Number of tours to return, default 50</param>
        /// <param name="code">(valid for admin only) code to filter on</param>
        /// <returns>List of tours, all with calculated suggestions</returns>
        [HttpGet("all/suggested")]
        public TourList GetAllToursSuggested([FromQuery] int from = 0, [FromQuery] int count = 50, [FromQuery] string code = "")
        {
            var tours = GetAllTours(from, count, code);
            var ts = tours.Tours.Select(t => new TourCalculator(t).SuggestFinalPayments()).ToArray();
            tours.Tours = ts;
            foreach (var t in tours.Tours){
                t.Spendings.Clear();
                foreach (var p in t.Persons){
                    p.SpentSendingInfo.Clear();
                    p.ReceivedSendingInfo.Clear();                    
                }
            }
            return tours;
        }

        /// <summary>
        /// Add tour with given access code (for admin) or with current access code (for non-admin user)
        /// If user is not admin, new tour can be added only if there are already tours with given code. 
        /// </summary>
        /// <param name="accessCode">Access code for the tour. Is not taken into account for non-admin user.</param>
        /// <param name="tourJson">Tour json. There are defaults, so /namefilled would be ok.</param>
        /// <returns>id of newly created tour</returns>
        [HttpPost("add/{accessCode}")]
        public string AddTour([FromBody]Tour tourJson, string accessCode)
        {
            AuthData authData = AuthHelper.GetAuthData(User, Configuration);
            bool allowed = authData.IsMaster;
            var forbidMessage = "Only admin can create first tour for a code";
            if (!allowed) // not master
            {
                var tours = TourStorageUtilities_LoadAllTours();
                var maxCountOfToursPerCode = Configuration.GetValue<int>("MaxCountOfToursPerCode", -1);
                bool noTours = !tours.Tours.Any();
                bool maxToursReached = !(maxCountOfToursPerCode == -1 || tours.Tours.Count() < maxCountOfToursPerCode);
                allowed = !noTours && !maxToursReached;
                if (maxToursReached) forbidMessage = $"You can create up to {maxCountOfToursPerCode} tours per code. To add more please ask administrator";
            }
            if (!allowed)
            {
                throw HttpException.Forbid(forbidMessage);
            }
            tourJson.GUID = IdHelper.NewId();
            tourJson.AccessCodeMD5 = authData.IsMaster ? AuthHelper.CreateMD5(accessCode) : authData.AccessCodeMD5s().First();
            tourJson.DateCreated = DateTime.Now;
            tourStorage.AddTour(tourJson);
            return tourJson.GUID;
        }
        private static readonly object getTourMonitorLock = new();
        private static readonly Dictionary<string, object> updateTourMonitor = new();
        /// <summary>
        /// Update the tour
        /// </summary>
        /// <param name="tourid">id of the tour</param>
        /// <param name="tourJson">updated tour. Full json.</param>
        /// <returns>id of updated tour</returns>
        [HttpPatch("{tourid}")]
        public string UpdateTour(string tourid, Tour tourJson)
        {
            var tour = TourStorageUtilities_LoadFromStoragebyId(tourid);

            if (tour == null) throw HttpException.NotFound($"no tour with id {tourid}");

            lock (getTourMonitorLock)
            {
                if (!updateTourMonitor.ContainsKey(tour.Id)) 
                    updateTourMonitor[tour.Id] = new object();
            }

            lock (updateTourMonitor[tour.Id]){ 
                if (tourJson.IsVersion && tourJson.GUID != tour.GUID && !tour.IsVersion) // restoring from version
                {
                    tourJson.IsVersion = false;
                    tourJson.InternalVersionComment = $"Tour Restored to {tourJson.DateVersioned:yyyy-MM-dd HH:mm:ss}";
                }
                else // changing the tour
                {
                    // check soft lock
                    var storedState = tour.StateGUID;
                    var updatingState = tourJson.StateGUID;
                    if (storedState != updatingState)
                    { // we are saving tour of different state
                        throw new HttpException(409, $"You are trying to override newer version of tour ({storedState})");
                    }
                    // all ok, storing. Provide new state id
                    tourJson.StateGUID = IdHelper.NewStateGuid();
                }
                tourJson.GUID = tourid;
                TourStorage_StoreTour(tourJson);
                return tourJson.GUID;
            }
        }
        /// <summary>
        /// Update tour's name
        /// </summary>
        /// <param name="tourid">id of tour to update</param>
        /// <param name="tourJson">tour's json. Only /name and /AccessCodeMD5 (if provided) is used</param>
        /// <returns>id of updated tour</returns>
        //[HttpPatch("{tourid}/changename")]
        private string UpdateTourName(string tourid, Tour tourJson)
        {
            var tour = TourStorageUtilities_LoadFromStoragebyId(tourid);

            if (tour == null) throw HttpException.NotFound($"no tour with id {tourid}");

            tour.Name = tourJson.Name;
            if (!string.IsNullOrWhiteSpace(tourJson.AccessCodeMD5))
            {
                AuthData authData = AuthHelper.GetAuthData(User, Configuration);
                if (authData.IsMaster)
                {
                    tour.AccessCodeMD5 = AuthHelper.CreateMD5(tourJson.AccessCodeMD5);
                }
            }
            TourStorage_StoreTour(tour);

            return tour.GUID;
        }
        /// <summary>
        /// Update tour's archived status
        /// </summary>
        /// <param name="tourid">id of tour to update</param>
        /// <param name="tourJson">tour's json. Only /isArchived and /AccessCodeMD5 (if provided) is used</param>
        /// <returns>id of updated tour</returns>
        //[HttpPatch("{tourid}/archive")]
        private string UpdateTourArchived(string tourid, Tour tourJson)
        {
            var tour = TourStorageUtilities_LoadFromStoragebyId(tourid);

            if (tour == null) throw HttpException.NotFound($"no tour with id {tourid}");

            tour.IsArchived = tourJson.IsArchived;
            if (!string.IsNullOrWhiteSpace(tourJson.AccessCodeMD5))
            {
                AuthData authData = AuthHelper.GetAuthData(User, Configuration);
                if (authData.IsMaster)
                {
                    tour.AccessCodeMD5 = AuthHelper.CreateMD5(tourJson.AccessCodeMD5);
                }
            }
            TourStorage_StoreTour(tour);

            return tour.GUID;
        }
        /// <summary>
        /// Update tour's archived status
        /// </summary>
        /// <param name="tourid">id of tour to update</param>
        /// <param name="tourJson">tour's json. Only /isArchived and /AccessCodeMD5 (if provided) is used</param>
        /// <returns>id of updated tour</returns>
        //[HttpPatch("{tourid}/finalizing")]
        private string UpdateTourFinalizing(string tourid, Tour tourJson)
        {
            var tour = TourStorageUtilities_LoadFromStoragebyId(tourid);

            if (tour == null) throw HttpException.NotFound($"no tour with id {tourid}");

            tour.IsFinalizing = tourJson.IsFinalizing;
            if (!string.IsNullOrWhiteSpace(tourJson.AccessCodeMD5))
            {
                AuthData authData = AuthHelper.GetAuthData(User, Configuration);
                if (authData.IsMaster)
                {
                    tour.AccessCodeMD5 = AuthHelper.CreateMD5(tourJson.AccessCodeMD5);
                }
            }
            TourStorage_StoreTour(tour);

            return tour.GUID;
        }
        /// <summary>
        /// Delete tour
        /// </summary>
        /// <param name="tourid">id of tour</param>
        /// <returns>id of deleted tour</returns>
        [HttpDelete("{tourid}")]
        public string DeleteTour(string tourid)
        {
            AuthData authData = AuthHelper.GetAuthData(User, Configuration);
            bool allowed = authData.IsMaster;
            var forbidMsg = "Only admin can delete last tour for a code";
            if (!allowed)
            {
                var tours = TourStorageUtilities_LoadAllTours();
                var cnt = tours.Tours.Count();
                if (cnt > 1) // cannot delete last tour, otherwise can
                {
                    allowed = true;
                }
            }
            if (!allowed)
            {
                throw HttpException.Forbid(forbidMsg);
            }
            var tour = TourStorageUtilities_LoadFromStoragebyId(tourid);
            if (tour == null) throw HttpException.NotFound($"no tour with id {tourid}");

            tourStorage.DeleteTour(tourid);
            return tourid;
        }
        #endregion
        #region Persons
        /// <summary>
        /// Add a person to tour
        /// </summary>
        /// <param name="tourid">tour id</param>
        /// <param name="p">person json. GUID ignored</param>
        /// <returns>newly created person id</returns>
        //[HttpPost("{tourid}/person")]
        private string AddTourPerson(string tourid, Person p)
        {
            var tour = TourStorageUtilities_LoadFromStoragebyId(tourid);

            if (tour != null)
            {
                tour = tourStorageProcessor.AddPerson(tour, p);
                TourStorage_StoreTour(tour);
            }
            else throw HttpException.NotFound($"no tour with id {tourid}");
            return p.GUID;
        }
        /// <summary>
        /// Update given person in given tour
        /// </summary>
        /// <param name="tourid">tour id</param>
        /// <param name="personguid">person id</param>
        /// <param name="p">full person json. GUID ignored</param>
        /// <returns>updated person id</returns>
        //[HttpPatch("{tourid}/person/{personguid}")]
        private string UpdateTourPerson(string tourid, string personguid, Person p)
        {
            var t = TourStorageUtilities_LoadFromStoragebyId(tourid);

            if (t != null)
            {
                t = tourStorageProcessor.UpdatePerson(t, p, personguid);
                if (t == null) throw HttpException.NotFound($"No person with id {personguid} in tour {tourid}");
                TourStorage_StoreTour(t);
            }
            else throw HttpException.NotFound($"cannot update: no tour with id {tourid}");
            return p.GUID;
        }
        /// <summary>
        /// Delete given person from given tour
        /// </summary>
        /// <param name="tourid">tour id</param>
        /// <param name="personguid">person id</param>
        /// <returns>id of deleted person</returns>
        //[HttpDelete("{tourid}/person/{personguid}")]
        private string DeleteTourPerson(string tourid, string personguid)
        {
            var t = TourStorageUtilities_LoadFromStoragebyId(tourid);

            if (t != null)
            {
                t = tourStorageProcessor.DeletePerson(t, personguid);
                if (t == null) throw HttpException.NotFound($"cannot delete: no person with id {personguid}");
                TourStorage_StoreTour(t);
                return personguid;
            }
            else throw HttpException.NotFound($"no tour with id {tourid}");
        }
        #endregion
        #region Spendings
        /// <summary>
        /// Update given spending in given tour
        /// </summary>
        /// <param name="tourid">id of the tour</param>
        /// <param name="spendingid">id of the spending</param>
        /// <param name="sp">full spending JSON. GUID ignored</param>
        /// <returns>id of updated spending</returns>
        //[HttpPatch("{tourid}/spending/{spendingid}")]
        private string UpdateTourSpending(string tourid, string spendingid, Spending sp)
        {
            var t = TourStorageUtilities_LoadFromStoragebyId(tourid);


            if (t != null)
            {
                t = tourStorageProcessor.UpdateSpending(t, sp, spendingid);
                if (t == null) throw HttpException.NotFound($"No spending with id {spendingid} in tour {tourid}");
                TourStorage_StoreTour(t);
            }
            else throw HttpException.NotFound($"no tour with id {tourid}");
            return sp.GUID;
        }
        /// <summary>
        /// Add spending to the tour
        /// </summary>
        /// <param name="tourid">tour id</param>
        /// <param name="s">Spending JSON</param>
        /// <returns>id of newly created spending</returns>
        //[HttpPost("{tourid}/spending")]
        private string AddTourSpending(string tourid, Spending s)
        {
            var t = TourStorageUtilities_LoadFromStoragebyId(tourid);
            if (t != null)
            {
                t = tourStorageProcessor.AddSpending(t, s);
                TourStorage_StoreTour(t);
            }
            else throw HttpException.NotFound($"no tour with id {tourid}");
            return s.GUID;
        }

        private void TourStorage_StoreTour(Tour tour)
        {
             tourStorage.StoreTour(tour);
        }

        /// <summary>
        /// Delete given spending from given tour
        /// </summary>
        /// <param name="tourid">tour id</param>
        /// <param name="spendingid">spending id</param>
        /// <returns>id of deleted spending</returns>
        //[HttpDelete("{tourid}/spending/{spendingid}")]
        private string DeleteTourSpending(string tourid, string spendingid)
        {
            var t = TourStorageUtilities_LoadFromStoragebyId(tourid);

            if (t != null)
            {
                t = tourStorageProcessor.DeleteSpending(t, spendingid);
                if (t == null) throw HttpException.NotFound($"Cannot delete: no spending with id {spendingid} in tour {tourid}");
                TourStorage_StoreTour(t);
                return spendingid;
            }
            else throw HttpException.NotFound($"no tour with id {tourid}");
        }
        #endregion
        private Tour TourStorageUtilities_LoadFromStoragebyId(string tourid)
        {
            AuthData authData = AuthHelper.GetAuthData(User, Configuration);
            var tour = tourStorage.GetTour(tourid);
            if (tour == null) return null;
            if (!authData.IsMaster)
            {
                if (!(authData.AccessCodeMD5s().Contains(tour.AccessCodeMD5)))
                {
                    return null;
                }
            }
            return tour;
        }
        private TourList TourStorageUtilities_LoadAllTours(int from = 0, int count = 50, string code = "")
        {
            
            AuthData authData = AuthHelper.GetAuthData(User, Configuration);
            Expression<Func<Tour, bool>> predicate;
            var cmd5 = AuthHelper.CreateMD5(code);
            if (authData.IsMaster)
            {
                if (string.IsNullOrWhiteSpace(code))
                {
                    predicate = t => true;
                } else
                {
                    predicate = (t) => cmd5 == t.AccessCodeMD5;
                }
            } else
            {
                predicate = t => (t.AccessCodeMD5 != null && authData.AccessCodeMD5s().Contains(t.AccessCodeMD5));
            }
            var tours = tourStorage.GetTours(
                predicate
                ,Configuration.GetValue("ReturnVersionsInAllTours", false)
                ,from
                ,count
                , out var totalCount
                );
            return new TourList()
            {
                Tours = tours,
                From = from,
                RequestedCount = count,
                Count = tours.Count(),
                TotalCount = totalCount
            };
        }

    }
}
