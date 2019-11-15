using System;
using System.Collections.Generic;
using System.Text;

namespace TCalc.Domain
{
    public class TourList
    {
        public IEnumerable<Tour> Tours { get; set; }
        public int TotalCount { get; set; }
        public int From { get; set; }
        public int Count { get; set; }
        public int RequestedCount { get; set; }
    }

}
