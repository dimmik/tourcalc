using System;
using System.Collections.Generic;
using System.Linq;
using System.Net;
using System.Net.Http;
using System.Web.Http;
using TCalc.Domain;
using TCalc.Logic;
using LiteDB;
using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Configuration;

namespace Opds.Controllers
{
    [Route("api/[controller]")]
    //[Authorize]
    [ApiController]
    public class TourController : ControllerBase
    {
        private string path => Configuration.GetValue<string>("DatabasePath");//= @"C:\tmp\Tour2.db";

        private readonly IConfiguration Configuration;

        public TourController(IConfiguration config)
        {
            Configuration = config;
        }
        #region Tours
        [HttpGet("{tourid}")]
        public IActionResult GetTour(string tourid)
        {
            return Ok(TourStorageUtilities_LoadFromLiteDBbyId(path, tourid));
        }
        [HttpGet("{tourid}/calculated")]
        public IActionResult GetTourCalculated(string tourid)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(path, tourid);
            if (tour == null) return NotFound($"no tour with id {tourid}");
            var calculator = new TourCalculator(tour);
            var calculated = calculator.Calculate();
            return Ok(calculated);
        }

        [HttpGet]
        public IActionResult GetAllTours()
        {
            var tours = TourStorageUtilities_LoadAllTours(path);
            return Ok(tours.ToArray());
        }


        [HttpPost]
        public IActionResult AddTour([FromBody]Tour t)
        {
            t.GUID = Guid.NewGuid().ToString();
            TourStorageUtilities.NewLiteDB(t, path);
            return Ok(t.GUID);
        }

        [HttpPatch("{tourid}")]
        public IActionResult UpdateTour(string tourid, Tour t)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(path, tourid);

            if (tour == null) return NotFound($"no tour with id {tourid}");

            t.GUID = tourid;
            t.StoreToLiteDB(path);

            return Ok(t.GUID);
        }

        [HttpDelete("{tourid}")]
        public IActionResult DeleteTour(string tourid)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(path, tourid);
            if (tour == null) return NotFound($"no tour with id {tourid}");

            using (var db = new LiteDatabase(path))
            {
                var col = db.GetCollection<Tour>("Tour");
                col.Delete(x => x.GUID == tourid);
                return Ok(tourid);
            }
        }
        #endregion
        #region Persons
        [HttpGet("{tourid}/person")]
        public IActionResult GetAllTourPersons(string tourid)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(path, tourid);

            if (tour != null) return Ok(tour.Persons);
            else return NotFound($"no tour with id {tourid}");

        }

        [HttpGet("{tourid}/person/{personguid}")]
        public IActionResult GetTourPerson(string tourid, string personguid)
        {
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(path, tourid);

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
            var tour = TourStorageUtilities_LoadFromLiteDBbyId(path, tourid);

            if (tour != null)
            {
                p.GUID = Guid.NewGuid().ToString();
                tour.Persons.Add(p);
                tour.StoreToLiteDB(path);
            }
            else return NotFound($"no tour with id {tourid}");
            return Ok(p.GUID);
        }

        [HttpPatch("{tourid}/person/{personguid}")]
        public IActionResult UpdateTourPerson(string tourid, string personguid, Person p)
        {
            var t = TourStorageUtilities_LoadFromLiteDBbyId(path, tourid);
            p.GUID = personguid;

            if (t != null)
            {
                var idx = t.Persons.FindIndex(x => x.GUID == personguid);
                if (idx < 0) return NotFound($"No person with id {personguid} in tour {tourid}");

                t.Persons[idx] = p;
                
                t.StoreToLiteDB(path);
            }
            else return NotFound($"cannot update: no tour with id {tourid}");
            return Ok(p.GUID);
        }

        [HttpDelete("{tourid}/person/{personguid}")]
        public IActionResult DeleteTourPerson(string tourid, string personguid)
        {
            var t = TourStorageUtilities_LoadFromLiteDBbyId(path, tourid);

            if (t != null)
            {
                var removedPerson = t.Persons.SingleOrDefault(x => x.GUID == personguid);
                if (removedPerson != null)
                {
                    t.Persons.Remove(removedPerson);
                    t.StoreToLiteDB(path);
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
            var t = TourStorageUtilities_LoadFromLiteDBbyId(path, tourid);

            if (t != null) return Ok(t.Spendings);
            else return NotFound($"no tour with id {tourid}");
        }

        [HttpPatch("{tourid}/spending/{spendingid}")]
        public IActionResult UpdateTourSpending(string tourid, string spendingid, Spending sp)
        {
            var t = TourStorageUtilities_LoadFromLiteDBbyId(path, tourid);

            sp.GUID = spendingid;

            if (t != null)
            {
                var idx = t.Spendings.FindIndex(x => x.GUID == spendingid);
                if (idx < 0) return NotFound($"No spending with id {spendingid} in tour {tourid}");
                t.Spendings[idx] = sp;
                t.StoreToLiteDB(path);
            }
            else return NotFound($"no tour with id {tourid}");
            return Ok(sp.GUID);
        }

        [HttpGet("{tourid}/spending/{spendingid}")]
        public IActionResult GetTourSpending(string tourid, string spendingid)
        {
            var t = TourStorageUtilities_LoadFromLiteDBbyId(path, tourid);

            if (t != null) return Ok(t.Spendings.Find(x => x.GUID == spendingid));
            else return NotFound($"no tour with id {tourid}");
        }

        [HttpPost("{tourid}/spending")]
        public IActionResult AddTourSpending(string tourid, Spending s)
        {
            var t = TourStorageUtilities_LoadFromLiteDBbyId(path, tourid);
            if (t != null)
            {
                s.GUID = Guid.NewGuid().ToString();
                t.Spendings.Add(s);
                t.StoreToLiteDB(path);
            }
            else return NotFound($"no tour with id {tourid}");
            return Ok(s.GUID);
        }

        [HttpDelete("{tourid}/spending/{spendingid}")]
        public IActionResult DeleteTourSpending(string tourid, string spendingid)
        {
            var t = TourStorageUtilities_LoadFromLiteDBbyId(path, tourid);

            if (t != null)
            {
                var removedSpending = t.Spendings.SingleOrDefault(x => x.GUID == spendingid);
                if (removedSpending != null) t.Spendings.Remove(removedSpending);
                t.StoreToLiteDB(path);
                return Ok(spendingid);
            }
            else return NotFound($"no tour with id {tourid}");
        }
        #endregion
        private Tour TourStorageUtilities_LoadFromLiteDBbyId(string path, string tourid)
        {
            // TODO check if tour is available for a user
            return TourStorageUtilities.LoadFromLiteDBbyId(path, tourid);
        }
        private IEnumerable<Tour> TourStorageUtilities_LoadAllTours(string path)
        {
            // TODO check what tours are available
            return TourStorageUtilities.LoadAllToursFromDb(path);
        }
    }
}
