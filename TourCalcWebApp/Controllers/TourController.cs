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
        [HttpGet("{tourid}")]
        public Tour GetTour(string tourid)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(tourid);
            if (tour == null)
            {
                throw new HttpException(404, $"No tour with id={tourid}");
            }
            return tour;
        }
        [HttpGet("{tourid}/calculated")]
        public Tour GetTourCalculated(string tourid)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(tourid);
            if (tour == null) throw new HttpException(404, $"no tour with id {tourid}");
            var calculator = new TourCalculator(tour);
            var calculated = calculator.Calculate();
            //            var calculated = calculator.SuggestCloseSpendings();
            return calculated;
        }

        [HttpGet("{tourid}/suggested")]
        public Tour GetTourSuggested(string tourid)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(tourid);
            if (tour == null) throw new HttpException(404, $"no tour with id {tourid}");
            var calculator = new TourCalculator(tour);
            var calculated = calculator.SuggestFinalPayments();
            return calculated;
        }

        [HttpGet]
        public IEnumerable<Tour> GetAllTours()
        {
            var tours = TourStorageUtilities_LoadAllTours().OrderBy(t => t.DateCreated);
            return tours.ToArray();
        }


        [HttpPost("add/{accessCode}")]
        public string AddTour([FromBody]Tour t, string accessCode)
        {
            AuthData authData = AuthHelper.GetAuthData(User, Configuration);
            bool allowed = authData.IsMaster;
            if (!allowed)
            {
                allowed = TourStorageUtilities_LoadAllTours().Any();
            }
            if (!allowed)
            {
                throw new HttpException(403, "You are not authorized to create tour");
            }
            t.GUID = IdHelper.NewId();
            t.AccessCodeMD5 = authData.IsMaster ? AuthHelper.CreateMD5(accessCode) : authData.AccessCodeMD5;
            t.DateCreated = DateTime.Now;
            tourStorage.AddTour(t);
            return t.GUID;
        }

        [HttpPatch("{tourid}")]
        public string UpdateTour(string tourid, Tour t)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

            if (tour == null) throw new HttpException(404, $"no tour with id {tourid}");

            t.GUID = tourid;
            tourStorage.StoreTour(t);

            return t.GUID;
        }

        [HttpPatch("{tourid}/changename")]
        public string UpdateTourName(string tourid, Tour t)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

            if (tour == null) throw new HttpException(404, $"no tour with id {tourid}");

            tour.Name = t.Name;
            tourStorage.StoreTour(tour);

            return tour.GUID;
        }

        [HttpDelete("{tourid}")]
        public string DeleteTour(string tourid)
        {
            AuthData authData = AuthHelper.GetAuthData(User, Configuration);
            bool allowed = authData.IsMaster;
            if (!allowed)
            {
                throw new HttpException(403, "Only admin can delete the tour");
            }
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(tourid);
            if (tour == null) throw new HttpException(404, $"no tour with id {tourid}");

            tourStorage.DeleteTour(tourid);
            return tourid;
        }
        #endregion
        #region Persons
        [HttpGet("{tourid}/person")]
        public IEnumerable<Person> GetAllTourPersons(string tourid)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

            if (tour != null) return tour.Persons.OrderBy(p => p.DateCreated);
            else throw new HttpException(404, $"no tour with id {tourid}");

        }

        [HttpGet("{tourid}/person/{personguid}")]
        public Person GetTourPerson(string tourid, string personguid)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

            if (tour != null)
            {
                var p = tour.Persons.Find(x => x.GUID == personguid);
                if (p != null) return p;
                else throw new HttpException(404, $"no person with id {personguid}");
            }
            else throw new HttpException(404, $"no person with id {tourid}");
        }

        [HttpPost("{tourid}/person")]
        public string AddTourPerson(string tourid, Person p)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

            if (tour != null)
            {
                p.GUID = IdHelper.NewId();
                tour.Persons.Add(p);
                tourStorage.StoreTour(tour);
            }
            else throw HttpException.NotFound($"no tour with id {tourid}");
            return p.GUID;
        }

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

                tourStorage.StoreTour(t);
            }
            else throw HttpException.NotFound($"cannot update: no tour with id {tourid}");
            return p.GUID;
        }

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
                    tourStorage.StoreTour(t);
                    return removedPerson.GUID;
                }
                else throw HttpException.NotFound($"cannot delete: no person with id {personguid}");
            }
            else throw HttpException.NotFound($"no tour with id {tourid}");
        }
        #endregion
        #region Spendings
        [HttpGet("{tourid}/spending")]
        public IEnumerable<Spending> GetAllTourSpendings(string tourid)
        {
            var t = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

            if (t != null) return t.Spendings.OrderBy(sp => sp.DateCreated);
            else throw HttpException.NotFound($"no tour with id {tourid}");
        }

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
                tourStorage.StoreTour(t);
            }
            else throw HttpException.NotFound($"no tour with id {tourid}");
            return sp.GUID;
        }

        [HttpGet("{tourid}/spending/{spendingid}")]
        public Spending GetTourSpending(string tourid, string spendingid)
        {
            var t = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

            if (t != null) return t.Spendings.Find(x => x.GUID == spendingid);
            else throw HttpException.NotFound($"no tour with id {tourid}");
        }

        [HttpPost("{tourid}/spending")]
        public string AddTourSpending(string tourid, Spending s)
        {
            var t = TourStorageUtilities_LoadFromLiteDBbyId(tourid);
            if (t != null)
            {
                s.GUID = IdHelper.NewId();
                t.Spendings.Add(s);
                tourStorage.StoreTour(t);
            }
            else throw HttpException.NotFound($"no tour with id {tourid}");
            return s.GUID;
        }

        [HttpDelete("{tourid}/spending/{spendingid}")]
        public string DeleteTourSpending(string tourid, string spendingid)
        {
            var t = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

            if (t != null)
            {
                var removedSpending = t.Spendings.SingleOrDefault(x => x.GUID == spendingid);
                if (removedSpending != null) t.Spendings.Remove(removedSpending);
                tourStorage.StoreTour(t);
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
        private IEnumerable<Tour> TourStorageUtilities_LoadAllTours()
        {
            
            AuthData authData = AuthHelper.GetAuthData(User, Configuration);
            return tourStorage.GetTours(t => authData.IsMaster 
            ? true // get everything
            : (t.AccessCodeMD5 != null && authData.AccessCodeMD5 == t.AccessCodeMD5));
        }

    }
}
