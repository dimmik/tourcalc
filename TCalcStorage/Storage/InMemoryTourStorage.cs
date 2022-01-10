using Newtonsoft.Json;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Linq.Expressions;
using System.Text;
using TCalc.Domain;
using TCalc.Storage;

namespace TCalcStorage.Storage
{
    public class InMemoryTourStorage : ITourStorage
    {
        private readonly List<Tour> Tours;

        public InMemoryTourStorage(List<Tour> tours = null)
        {
            Tours = tours ?? new List<Tour>();
        }

        public void AddTour(Tour tour)
        {
            Tours.Add(tour);
        }

        public void DeleteTour(string tourid)
        {
            Tours.RemoveAll(t => t.Id == tourid);
        }

        public Tour GetTour(string tourid)
        {
            var tour = Tours.FirstOrDefault(t => t.Id == tourid);
            // return a clone
            return JsonConvert.DeserializeObject<Tour>(JsonConvert.SerializeObject(tour));
        }

        public IEnumerable<Tour> GetTours(Expression<Func<Tour, bool>> predicate, bool includeVersions, int from, int count, out int totalCount)
        {
            var coll = Tours;
            Expression<Func<Tour, bool>> isVersion;
            if (includeVersions)
            {
                isVersion = t => true;
            }
            else
            {
                isVersion = t => !t.IsVersion;
            }
            var allTours = coll
                .AsQueryable()
                .Where(isVersion)
                .Where(predicate)
                .OrderByDescending(t => t.DateCreated);
            totalCount = (int)allTours.Count();
            var tours = allTours.Skip(from).Take(count).ToList();
            return tours;
        }

        public IEnumerable<Tour> GetTourVersions(Expression<Func<Tour, bool>> predicate, string tourId, int from, int count, out int totalCount)
        {
            var coll = Tours;
            Expression<Func<Tour, bool>> vpr = (t) => (t.IsVersion && t.VersionFor_Id == tourId);

            var allVersions = coll.AsQueryable<Tour>()
                .Where(predicate)
                .Where(vpr)
                .OrderByDescending(t => t.DateVersioned);

            totalCount = (int)allVersions.Count();
            var versions = allVersions.Skip(from).Take(count).ToList();
            return versions;
        }

        public void StoreTour(Tour tour)
        {
            int idx = Tours.FindIndex(0, Tours.Count, t => t.Id == tour.Id);
            if (idx == -1)
            {
                AddTour(tour);
                return;
            }
            Tours[idx] = tour;
        }
    }
}
