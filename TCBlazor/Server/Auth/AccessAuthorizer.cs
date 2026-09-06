using System;
using System.IdentityModel.Tokens.Jwt;
using System.Security.Claims;
using Microsoft.IdentityModel.Tokens;
using TCalcCore.Auth;
using Company.TCBlazor.Exceptions;

namespace Company.TCBlazor.Auth
{
    /// <summary>
    /// Turns an access code into an <see cref="AuthData"/>, and an <see cref="AuthData"/>
    /// into the signed token the app carries.
    ///
    /// This used to live inside AuthController, where only the JSON API could reach it.
    /// The no-JavaScript text pages have to answer exactly the same question - is this
    /// code good, and what does it let you see - so the decision was moved here rather
    /// than written a second time. Two implementations of "who may see what" would be
    /// two things to keep in step, and only one of them would be tested.
    /// </summary>
    public static class AccessAuthorizer
    {
        public static AuthData Authorize(ITcConfiguration configuration, string scope, string accessCode, bool accessCodeIsMd5 = false)
        {
            if (accessCode == null) accessCode = "";
            AuthData auth = new AuthData();
            if (scope == "admin")
            {
                // generate for master key
                string keyProvided = accessCode;
                string keyFromConfig = configuration.GetValue<string>("MasterKey");
                if (keyProvided == keyFromConfig)
                {
                    auth.Type = "Master";
                    auth.IsMaster = true;
                }
                else
                {
                    throw HttpException.NotAuthenticated($"Wrong Master Key");
                }
            }
            else if (scope == "code")
            {
                auth.Type = "AccessCode";
                auth.IsMaster = false;
                auth.AccessCodeMD5 = accessCodeIsMd5 ? accessCode : AuthHelper.CreateMD5(accessCode);
            }
            else
            {
                throw HttpException.NotAuthenticated("Wrong scope. Please try 'code' or 'admin'. Or 'pigeon', who knows. Maybe 'slippery' will work.");
            }

            return auth;
        }

        /// <summary>The JWT the browser carries - in a header for the SPA, in a cookie for the text pages.</summary>
        public static string CreateToken(ITcConfiguration configuration, IECDsaCryptoKey signerKey, string scope, AuthData auth)
        {
            var claims = new Claim[]
            {
                new Claim(ClaimTypes.NameIdentifier, scope),
                new Claim("AuthDataJson", Newtonsoft.Json.JsonConvert.SerializeObject(auth))
            };
            var tokenValidTimeInMinutes = configuration.GetValue("TokenValidTimeInMinutes", (180 * 60 * 24));
            var token = new JwtSecurityToken(
                issuer: "TourCalc",
                audience: "Users",
                claims: claims,
                expires: DateTime.Now.AddMinutes(tokenValidTimeInMinutes),
                signingCredentials: new SigningCredentials(
                                       signerKey.GetPrivateKey(),
                                       signerKey.SigningAlgorithm
                                       )
                );
            return new JwtSecurityTokenHandler().WriteToken(token);
        }

        /// <summary>How long the text-page cookie is kept, matching the token's own lifetime.</summary>
        public static TimeSpan TokenLifetime(ITcConfiguration configuration)
            => TimeSpan.FromMinutes(configuration.GetValue("TokenValidTimeInMinutes", (180 * 60 * 24)));
    }
}
