using System;
using System.Collections.Generic;
using System.Linq.Expressions;
using System.Text;
using TCalc.Domain;

namespace TCalc.Storage
{
    public interface ITourStorage
    {
        IEnumerable<Tour> GetTours(Expression<Func<Tour, bool>> predicate = null);
        Tour GetTour(string tourid);
        void AddTour(Tour tour);
        void DeleteTour(string tourid);
        void StoreTour(Tour tour);
    }
}
