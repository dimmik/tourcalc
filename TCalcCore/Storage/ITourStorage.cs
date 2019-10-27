using System;
using System.Collections.Generic;
using System.Linq.Expressions;
using System.Text;
using TCalc.Domain;

namespace TCalc.Storage
{
    public interface ITourStorage
    {
        IEnumerable<Tour> GetTours(Expression<Func<Tour, bool>> predicate, bool includeVersions, int from, int count, out int totalCount);
        Tour GetTour(string tourid);
        IEnumerable<Tour> GetTourVersions(Expression<Func<Tour, bool>> predicate, string tourId, int from, int count, out int totalCount);
        void AddTour(Tour tour);
        void DeleteTour(string tourid);
        void StoreTour(Tour tour);
    }
}
