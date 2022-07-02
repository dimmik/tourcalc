using System;
using System.Collections.Generic;
using System.Text;
using System.Threading.Tasks;
using TCalc.Domain;

namespace TCalcCore.Engine
{
    public class TourcalcDelegates
    {
        public delegate Task OnTourLoaded(Tour tour, bool isFromServer, DateTimeOffset updatedDt);
        public delegate Task OnTourListLoaded(TourList tours, bool isFromServer, DateTimeOffset updatedDt);
        public delegate Task OnTourPartSubmitting();
        public delegate void OnMessageReceived(string type, object payload);
    }
}
