using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;

namespace TCalcCore.Auth
{
    public class AuthData
    {
        public string Type { get; set; } = "None";
        public bool IsMaster { get; set; } = false; // allow all
        public string AccessCodeMD5 { get; set; } = "";
        public List<string> TourIds { get; set; } = new List<string>();
//        public string TourId { get; set; } = "";
    }
}
