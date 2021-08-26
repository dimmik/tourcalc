using System;
using System.Collections.Generic;
using System.Linq.Expressions;
using System.Text;
using TCalc.Domain;
using TCalc.Storage;

namespace TCalcStorage.Storage.Github
{
    class GithubTourStorage : ITourStorage
    {
        public void AddTour(Tour tour)
        {
            throw new NotImplementedException();
        }

        public void DeleteTour(string tourid)
        {
            throw new NotImplementedException();
        }

        public Tour GetTour(string tourid)
        {
            throw new NotImplementedException();
        }

        public IEnumerable<Tour> GetTours(Expression<Func<Tour, bool>> predicate, bool includeVersions, int from, int count, out int totalCount)
        {
            throw new NotImplementedException();
        }

        public IEnumerable<Tour> GetTourVersions(Expression<Func<Tour, bool>> predicate, string tourId, int from, int count, out int totalCount)
        {
            throw new NotImplementedException();
        }

        public void StoreTour(Tour tour)
        {
            throw new NotImplementedException();
        }
    }
}
