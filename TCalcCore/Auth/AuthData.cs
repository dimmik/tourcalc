using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Threading.Tasks;

namespace TCalcCore.Auth
{
    public class AuthData
    {
        public AuthData()
        {
        }

        public string Type { get; set; } = "None";
        public bool IsMaster { get; set; } = false; // allow all
        public string AccessCodeMD5 { get; set; } = "";

    }
    public static class ADHelpers
    {
        private static readonly char Delim = ';';
        public static IEnumerable<string> AccessCodeMD5s(this AuthData me) 
        {
            if (string.IsNullOrWhiteSpace(me.AccessCodeMD5)) return new[] { me.AccessCodeMD5 };
            return me.AccessCodeMD5.Split(Delim);
        }
        public static char AuthTokensDelim(this AuthData _)
        {
            return Delim;
        }
    }
}
