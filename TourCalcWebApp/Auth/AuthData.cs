using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;

namespace TourCalcWebApp.Auth
{
    public class AuthData
    {
        public string Type { get; set; } = "None";
        public bool IsMaster { get; set; } = false; // allow all
        public string AccessCode { get; set; } = "";
        public string TourId { get; set; } = "";
    }
}
