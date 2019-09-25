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
        public IActionResult GetTour(string tourid)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(tourid);
            return tour == null ? (IActionResult)NotFound($"No tour with id {tourid}") : (IActionResult)Ok(tour);
        }
        [HttpGet("{tourid}/calculated")]
        public IActionResult GetTourCalculated(string tourid)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(tourid);
            if (tour == null) return NotFound($"no tour with id {tourid}");
            var calculator = new TourCalculator(tour);
            var calculated = calculator.Calculate();
            return Ok(calculated);
        }

        [HttpGet]
        public IActionResult GetAllTours()
        {
            var tours = TourStorageUtilities_LoadAllTours();
            return Ok(tours.ToArray());
        }


        [HttpPost("add/{accessCode}")]
        public IActionResult AddTour([FromBody]Tour t, string accessCode)
        {
            // TODO - authentication!!!
            AuthData authData = AuthHelper.GetAuthData(User, Configuration);
            if (!authData.IsMaster)
            {
                return Forbid();
            }
            t.GUID = IdHelper.NewId();
            t.AccessCodeMD5 = AuthHelper.CreateMD5(accessCode);
            tourStorage.AddTour(t);
            return Ok(t.GUID);
        }

        [HttpPatch("{tourid}")]
        public IActionResult UpdateTour(string tourid, Tour t)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

            if (tour == null) return NotFound($"no tour with id {tourid}");

            t.GUID = tourid;
            tourStorage.StoreTour(t);

            return Ok(t.GUID);
        }

        [HttpPatch("{tourid}/changename")]
        public IActionResult UpdateTourName(string tourid, Tour t)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

            if (tour == null) return NotFound($"no tour with id {tourid}");

            tour.Name = t.Name;
            tourStorage.StoreTour(tour);

            return Ok(tour.GUID);
        }

        [HttpDelete("{tourid}")]
        public IActionResult DeleteTour(string tourid)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(tourid);
            if (tour == null) return NotFound($"no tour with id {tourid}");

            tourStorage.DeleteTour(tourid);
            return Ok(tourid);
        }
        #endregion
        #region Persons
        [HttpGet("{tourid}/person")]
        public IActionResult GetAllTourPersons(string tourid)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

            if (tour != null) return Ok(tour.Persons);
            else return NotFound($"no tour with id {tourid}");

        }

        [HttpGet("{tourid}/person/{personguid}")]
        public IActionResult GetTourPerson(string tourid, string personguid)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

            if (tour != null)
            {
                var p = tour.Persons.Find(x => x.GUID == personguid);
                if (p != null) return Ok(p);
                else return NotFound($"no person with id {personguid}");
            }
            else return NotFound($"no person with id {tourid}");
        }

        [HttpPost("{tourid}/person")]
        public IActionResult AddTourPerson(string tourid, Person p)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

            if (tour != null)
            {
                p.GUID = IdHelper.NewId();
                tour.Persons.Add(p);
                tourStorage.StoreTour(tour);
            }
            else return NotFound($"no tour with id {tourid}");
            return Ok(p.GUID);
        }

        [HttpPatch("{tourid}/person/{personguid}")]
        public IActionResult UpdateTourPerson(string tourid, string personguid, Person p)
        {
            var t = TourStorageUtilities_LoadFromLiteDBbyId(tourid);
            p.GUID = personguid;

            if (t != null)
            {
                var idx = t.Persons.FindIndex(x => x.GUID == personguid);
                if (idx < 0) return NotFound($"No person with id {personguid} in tour {tourid}");

                t.Persons[idx] = p;

                tourStorage.StoreTour(t);
            }
            else return NotFound($"cannot update: no tour with id {tourid}");
            return Ok(p.GUID);
        }

        [HttpDelete("{tourid}/person/{personguid}")]
        public IActionResult DeleteTourPerson(string tourid, string personguid)
        {
            var t = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

            if (t != null)
            {
                var removedPerson = t.Persons.SingleOrDefault(x => x.GUID == personguid);
                if (removedPerson != null)
                {
                    t.Persons.Remove(removedPerson);
                    tourStorage.StoreTour(t);
                    return Ok(removedPerson.GUID);
                }
                else return NotFound($"cannot delete: no person with id {personguid}");
            }
            else return NotFound($"no tour with id {tourid}");
        }
        #endregion
        #region Spendings
        [HttpGet("{tourid}/spending")]
        public IActionResult GetAllTourSpendings(string tourid)
        {
            var t = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

            if (t != null) return Ok(t.Spendings);
            else return NotFound($"no tour with id {tourid}");
        }

        [HttpPatch("{tourid}/spending/{spendingid}")]
        public IActionResult UpdateTourSpending(string tourid, string spendingid, Spending sp)
        {
            var t = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

            sp.GUID = spendingid;

            if (t != null)
            {
                var idx = t.Spendings.FindIndex(x => x.GUID == spendingid);
                if (idx < 0) return NotFound($"No spending with id {spendingid} in tour {tourid}");
                t.Spendings[idx] = sp;
                tourStorage.StoreTour(t);
            }
            else return NotFound($"no tour with id {tourid}");
            return Ok(sp.GUID);
        }

        [HttpGet("{tourid}/spending/{spendingid}")]
        public IActionResult GetTourSpending(string tourid, string spendingid)
        {
            var t = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

            if (t != null) return Ok(t.Spendings.Find(x => x.GUID == spendingid));
            else return NotFound($"no tour with id {tourid}");
        }

        [HttpPost("{tourid}/spending")]
        public IActionResult AddTourSpending(string tourid, Spending s)
        {
            var t = TourStorageUtilities_LoadFromLiteDBbyId(tourid);
            if (t != null)
            {
                s.GUID = IdHelper.NewId();
                t.Spendings.Add(s);
                tourStorage.StoreTour(t);
            }
            else return NotFound($"no tour with id {tourid}");
            return Ok(s.GUID);
        }

        [HttpDelete("{tourid}/spending/{spendingid}")]
        public IActionResult DeleteTourSpending(string tourid, string spendingid)
        {
            var t = TourStorageUtilities_LoadFromLiteDBbyId(tourid);

            if (t != null)
            {
                var removedSpending = t.Spendings.SingleOrDefault(x => x.GUID == spendingid);
                if (removedSpending != null) t.Spendings.Remove(removedSpending);
                tourStorage.StoreTour(t);
                return Ok(spendingid);
            }
            else return NotFound($"no tour with id {tourid}");
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
            return tourStorage.GetTours(x => authData.IsMaster ? true : (x.AccessCodeMD5 != null && authData.AccessCodeMD5 == x.AccessCodeMD5));
        }

    }
}
