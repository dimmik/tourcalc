using System;
using System.Collections.Generic;
using System.Linq;
using System.Security.Claims;
using System.Threading.Tasks;
using Microsoft.Extensions.Configuration;
using TourCalcWebApp.Controllers;

namespace TourCalcWebApp.Auth
{
    public static class AuthHelper
    {
        public static AuthData GetAuthData(ClaimsPrincipal User, IConfiguration config)
        {
            var claimsIdentity = User.Identity as ClaimsIdentity;
            var authDataJson = claimsIdentity.FindFirst("AuthDataJson")?.Value;
            AuthData authData;
            if (authDataJson != null)
            {
                authData = Newtonsoft.Json.JsonConvert.DeserializeObject<AuthData>(authDataJson);
            }
            else
            {
                // default = master for debugging purposes
                authData = config.GetValue("AnonymousIsMaster", false)
                    ? new AuthData() { Type = "Master", IsMaster = true }
                    : new AuthData(); // default no-access
            }

            return authData;
        }

    }
}
