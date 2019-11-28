using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Security.Claims;
using System.Text;
using System.Threading.Tasks;
using Microsoft.Extensions.Configuration;
using TourCalcWebApp.Controllers;

namespace TourCalcWebApp.Auth
{
    public static class AuthHelper
    {
        public static AuthData GetAuthData(ClaimsPrincipal User, ITcConfiguration config)
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
        public static string CreateMD5(string input)
        {
            if (input == null) input = "";
            // Use input string to calculate MD5 hash
            using (System.Security.Cryptography.MD5 md5 = System.Security.Cryptography.MD5.Create())
            {
                byte[] inputBytes = System.Text.Encoding.ASCII.GetBytes(input);
                byte[] hashBytes = md5.ComputeHash(inputBytes);

                // Convert the byte array to hexadecimal string
                StringBuilder sb = new StringBuilder();
                for (int i = 0; i < hashBytes.Length; i++)
                {
                    sb.Append(hashBytes[i].ToString("X2"));
                }
                return sb.ToString();
            }
        }
        public static string CreateMD5(Stream input)
        {
            using (System.Security.Cryptography.MD5 md5 = System.Security.Cryptography.MD5.Create())
            {
                byte[] hashBytes = md5.ComputeHash(input);

                // Convert the byte array to hexadecimal string
                StringBuilder sb = new StringBuilder();
                for (int i = 0; i < hashBytes.Length; i++)
                {
                    sb.Append(hashBytes[i].ToString("X2"));
                }
                return sb.ToString();
            }


        }
    }
}
