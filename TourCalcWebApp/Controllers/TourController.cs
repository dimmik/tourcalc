using System;
using System.Collections.Generic;
using System.Linq;
using TCalc.Domain;
using TCalc.Storage;
using LiteDB;
using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Configuration;
using Microsoft.AspNetCore.Authorization;
using System.Security.Claims;
using TCalc.Logic;
using TourCalcWebApp.Auth;
using TourCalcWebApp.Exceptions;

namespace TourCalcWebApp.Controllers
{
    [Route("api/[controller]")]
    [Authorize]
    [AllowAnonymous]
    [ApiController]
    public class TourController : ControllerBase
    {

        private readonly IConfiguration Configuration;
        private readonly ITourStorage tourStorage;

        public TourController(IConfiguration config, ITourStorage storage)
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
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(tourid);
            if (tour == null)
            {
                throw HttpException.NotFound($"No tour with id={tourid}");
            }
            return tour;
        }
        /// <summary>
        /// Calculated (i.e. with persons' spent and persons' received fields filled) tour
        /// </summary>
        /// <param name="tourid">id of the tour</param>
        /// <returns>calculated tour</returns>
        [HttpGet("{tourid}/calculated")]
        public Tour GetTourCalculated(string tourid)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(tourid);
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
            var tours = tourStorage.GetTourVersions(
                t => authData.IsMaster
                        ? true // get everything
                        : (t.AccessCodeMD5 != null && authData.AccessCodeMD5 == t.AccessCodeMD5)
                , tourid
                , from
                , count
                , out var totalCount
                ).OrderBy(t => t.DateVersioned).Reverse();
            return new TourList()
            {
                Tours = tours,
                From = from,
                RequestedCount = count,
                Count = tours.Count(),
                TotalCount = totalCount
            };
        }

        /// <summary>
        /// Tour calculated AND with suggested spendings to close the tour (i.e. so that each persons' debt equal to zero)
        /// *This one is used in the SPA*
        /// </summary>
        /// <param name="tourid">id of the tour</param>
        /// <returns>calculated tour with suggestions</returns>
        [HttpGet("{tourid}/suggested")]
        public Tour GetTourSuggested(string tourid)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(tourid);
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
        /// <returns>List of tours</returns>
        [HttpGet]
        public TourList GetAllTours([FromQuery] int from = 0, [FromQuery] int count = 50)
        {
            var tours = TourStorageUtilities_LoadAllTours(from, count);
            return tours;
        }

        /// <summary>
        /// All tours available for a user, with suggested payments calculated
        /// </summary>
        /// <param name="from">Default 0</param>
        /// <param name="count">Number of tours to return, default 50</param>
        /// <returns>List of tours, all with calculated suggestions</returns>
        [HttpGet("all/suggested")]
        public TourList GetAllToursSuggested([FromQuery] int from = 0, [FromQuery] int count = 50)
        {
            var tours = GetAllTours(from, count);
            var ts = tours.Tours.Select(t => new TourCalculator(t).SuggestFinalPayments());
            tours.Tours = ts;
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
            tourJson.AccessCodeMD5 = authData.IsMaster ? AuthHelper.CreateMD5(accessCode) : authData.AccessCodeMD5;
            tourJson.DateCreated = DateTime.Now;
            tourStorage.AddTour(tourJson);
            return tourJson.GUID;
        }
        /// <summary>
        /// Update the tour
        /// </summary>
        /// <param name="tourid">id of the tour</param>
        /// <param name="tourJson">updated tour. Full json.</param>
        /// <returns>id of updated tour</returns>
        [HttpPatch("{tourid}")]
        public string UpdateTour(string tourid, Tour tourJson)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

            if (tour == null) throw HttpException.NotFound($"no tour with id {tourid}");

            tourJson.GUID = tourid;
            TourStorage_StoreTour(tourJson);

            return tourJson.GUID;
        }
        /// <summary>
        /// Update tour's name
        /// </summary>
        /// <param name="tourid">id of tour to update</param>
        /// <param name="tourJson">tour's json. Only /name and /AccessCodeMD5 (if provided) is used</param>
        /// <returns>id of updated tour</returns>
        [HttpPatch("{tourid}/changename")]
        public string UpdateTourName(string tourid, Tour tourJson)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

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
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(tourid);
            if (tour == null) throw HttpException.NotFound($"no tour with id {tourid}");

            tourStorage.DeleteTour(tourid);
            return tourid;
        }
        #endregion
        #region Persons
        /// <summary>
        /// All persons in given tour
        /// </summary>
        /// <param name="tourid">tour id</param>
        /// <returns>list of persons</returns>
        [HttpGet("{tourid}/person")]
        public IEnumerable<Person> GetAllTourPersons(string tourid)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

            if (tour != null) return tour.Persons.OrderBy(p => p.DateCreated);
            else throw HttpException.NotFound($"no tour with id {tourid}");

        }
        /// <summary>
        /// Given person in given tour
        /// </summary>
        /// <param name="tourid">tour id</param>
        /// <param name="personguid">person GUID</param>
        /// <returns>the person</returns>
        [HttpGet("{tourid}/person/{personguid}")]
        public Person GetTourPerson(string tourid, string personguid)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

            if (tour != null)
            {
                var p = tour.Persons.Find(x => x.GUID == personguid);
                if (p != null) return p;
                else throw HttpException.NotFound($"no person with id {personguid}");
            }
            else throw HttpException.NotFound($"no person with id {tourid}");
        }
        /// <summary>
        /// Add a person to tour
        /// </summary>
        /// <param name="tourid">tour id</param>
        /// <param name="p">person json. GUID ignored</param>
        /// <returns>newly created person id</returns>
        [HttpPost("{tourid}/person")]
        public string AddTourPerson(string tourid, Person p)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

            if (tour != null)
            {
                p.GUID = IdHelper.NewId();
                tour.Persons.Add(p);
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
        [HttpPatch("{tourid}/person/{personguid}")]
        public string UpdateTourPerson(string tourid, string personguid, Person p)
        {
            var t = TourStorageUtilities_LoadFromLiteDBbyId(tourid);
            p.GUID = personguid;

            if (t != null)
            {
                var idx = t.Persons.FindIndex(x => x.GUID == personguid);
                if (idx < 0) throw HttpException.NotFound($"No person with id {personguid} in tour {tourid}");
                p.DateCreated = t.Persons[idx].DateCreated; // preserve

                t.Persons[idx] = p;

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
        [HttpDelete("{tourid}/person/{personguid}")]
        public string DeleteTourPerson(string tourid, string personguid)
        {
            var t = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

            if (t != null)
            {
                var removedPerson = t.Persons.SingleOrDefault(x => x.GUID == personguid);
                if (removedPerson != null)
                {
                    t.Persons.Remove(removedPerson);
                    t.Spendings.RemoveAll(s => s.FromGuid == removedPerson.GUID);
                    t.Spendings.ForEach(s => s.ToGuid.RemoveAll(g => g == removedPerson.GUID));
                    TourStorage_StoreTour(t);
                    return removedPerson.GUID;
                }
                else throw HttpException.NotFound($"cannot delete: no person with id {personguid}");
            }
            else throw HttpException.NotFound($"no tour with id {tourid}");
        }
        #endregion
        #region Spendings
        /// <summary>
        /// All tour spendings
        /// </summary>
        /// <param name="tourid">id of tour</param>
        /// <returns>spendings</returns>
        [HttpGet("{tourid}/spending")]
        public IEnumerable<Spending> GetAllTourSpendings(string tourid)
        {
            var t = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

            if (t != null) return t.Spendings.OrderBy(sp => sp.DateCreated);
            else throw HttpException.NotFound($"no tour with id {tourid}");
        }
        /// <summary>
        /// Update given spending in given tour
        /// </summary>
        /// <param name="tourid">id of the tour</param>
        /// <param name="spendingid">id of the spending</param>
        /// <param name="sp">full spending JSON. GUID ignored</param>
        /// <returns>id of updated spending</returns>
        [HttpPatch("{tourid}/spending/{spendingid}")]
        public string UpdateTourSpending(string tourid, string spendingid, Spending sp)
        {
            var t = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

            sp.GUID = spendingid;

            if (t != null)
            {
                var idx = t.Spendings.FindIndex(x => x.GUID == spendingid);
                if (idx < 0) throw HttpException.NotFound($"No spending with id {spendingid} in tour {tourid}");
                sp.DateCreated = t.Spendings[idx].DateCreated; // preserve
                t.Spendings[idx] = sp;
                TourStorage_StoreTour(t);
            }
            else throw HttpException.NotFound($"no tour with id {tourid}");
            return sp.GUID;
        }
        /// <summary>
        /// Given spending in given tour
        /// </summary>
        /// <param name="tourid">tour id</param>
        /// <param name="spendingid">spending id</param>
        /// <returns>the spending</returns>
        [HttpGet("{tourid}/spending/{spendingid}")]
        public Spending GetTourSpending(string tourid, string spendingid)
        {
            var t = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

            if (t != null) return t.Spendings.Find(x => x.GUID == spendingid);
            else throw HttpException.NotFound($"no tour with id {tourid}");
        }
        /// <summary>
        /// Add spending to the tour
        /// </summary>
        /// <param name="tourid">tour id</param>
        /// <param name="s">Spending JSON</param>
        /// <returns>id of newly created spending</returns>
        [HttpPost("{tourid}/spending")]
        public string AddTourSpending(string tourid, Spending s)
        {
            var t = TourStorageUtilities_LoadFromLiteDBbyId(tourid);
            if (t != null)
            {
                s.GUID = IdHelper.NewId();
                t.Spendings.Add(s);
                TourStorage_StoreTour(t);
            }
            else throw HttpException.NotFound($"no tour with id {tourid}");
            return s.GUID;
        }

        private void TourStorage_StoreTour(Tour tour)
        {
            try
            {
                tourStorage.StoreTour(tour);
            } catch (TourStorageException e)
            {
                throw HttpException.Forbid(e.Message);
            }
        }

        /// <summary>
        /// Delete given spending from given tour
        /// </summary>
        /// <param name="tourid">tour id</param>
        /// <param name="spendingid">spending id</param>
        /// <returns>id of deleted spending</returns>
        [HttpDelete("{tourid}/spending/{spendingid}")]
        public string DeleteTourSpending(string tourid, string spendingid)
        {
            var t = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

            if (t != null)
            {
                var removedSpending = t.Spendings.SingleOrDefault(x => x.GUID == spendingid);
                if (removedSpending != null) t.Spendings.Remove(removedSpending);
                TourStorage_StoreTour(t);
                return spendingid;
            }
            else throw HttpException.NotFound($"no tour with id {tourid}");
        }
        #endregion
        private Tour TourStorageUtilities_LoadFromLiteDBbyId(string tourid)
        {
            AuthData authData = AuthHelper.GetAuthData(User, Configuration);
            var tour = tourStorage.GetTour(tourid);
            if (tour == null) return null;
            if (!authData.IsMaster)
            {
                if (!(authData.AccessCodeMD5 == tour.AccessCodeMD5))
                {
                    return null;
                }
            }
            return tour;
        }
        private TourList TourStorageUtilities_LoadAllTours(int from = 0, int count = 50)
        {
            
            AuthData authData = AuthHelper.GetAuthData(User, Configuration);
            var tours = tourStorage.GetTours(
                t => authData.IsMaster
                        ? true // get everything
                        : (t.AccessCodeMD5 != null && authData.AccessCodeMD5 == t.AccessCodeMD5)
                ,Configuration.GetValue("ReturnVersionsInAllTours", false)
                ,from
                ,count
                , out var totalCount
                ).OrderBy(t => t.DateCreated);
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
    public class TourList
    {
        public IEnumerable<Tour> Tours { get; set; }
        public int TotalCount { get; set; }
        public int From { get; set; }
        public int Count { get; set; }
        public int RequestedCount { get; set; }
    }
}
