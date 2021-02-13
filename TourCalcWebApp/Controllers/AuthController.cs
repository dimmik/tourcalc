using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IdentityModel.Tokens.Jwt;
using System.IO;
using System.Linq;
using System.Security.Claims;
using System.Text;
using System.Threading.Tasks;
using LiteDB;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Configuration;
using Microsoft.IdentityModel.Tokens;
using Newtonsoft.Json;
using Org.BouncyCastle.Security;
using TCalc.Domain;
using TCalc.Storage;
using TourCalcWebApp.Auth;
using TourCalcWebApp.Exceptions;
using TourCalcWebApp.Utils;

namespace TourCalcWebApp.Controllers
{
    [Route("api/[controller]")]
    [Authorize]
    [AllowAnonymous]
    [ApiController]
    public class AuthController : ControllerBase
    {
        private readonly ITcConfiguration Configuration;
        private readonly ITourStorage tourStorage;

        private readonly static DateTime InstanceCreated = DateTime.UtcNow;

        public AuthController(ITcConfiguration config, ITourStorage storage)
        {
            Configuration = config;
            tourStorage = storage;
        }

        [HttpGet("checkInstance")]
        public string CheckInstance()
        {
            var now = DateTime.UtcNow;
            return $"inst: {InstanceCreated} now: {now} diff: {(now - InstanceCreated).TotalSeconds} s";

        }

        [HttpGet("longr/{delayInSec}/{numberOfIterations}")]
        public async Task<IActionResult> LongRunning(int delayInSec, int numberOfIterations)
        {
            Stopwatch ssw = Stopwatch.StartNew();
            var r = Response;
            await r.SendResponseWithKeepalive(
                delayInSec: delayInSec, 
                contentType: "application/json",  
                howToGetResult: () => {
                    string start = $"{DateTime.Now:yyyy-MM-dd HH:mm:ss}";
                    Task.Delay(delayInSec * numberOfIterations * 1000).Wait();
                    string finish = $"{DateTime.Now:yyyy-MM-dd HH:mm:ss}";
                    return JsonConvert.SerializeObject(new { start = start, finish = finish }); }, 
                statusCode: 200,
                timeout: TimeSpan.FromHours(2)
                ).ConfigureAwait(false);
            return new EmptyResult();
        }

        

        /// <summary>
        /// Random bytes in base64 format.
        /// </summary>
        /// <param name="lengthInBytes">Number of bytes; defalult 32, max 8192</param>
        [HttpGet("random/{lengthInBytes=32}")]
        public string GenerateRandomKey(int lengthInBytes)
        {
            var r = new SecureRandom();
            if (lengthInBytes > 8192) throw HttpException.Forbid($"Length should be up to 8192 bytes. You specified {lengthInBytes}");
            byte[] bytes = new byte[lengthInBytes];
            r.NextBytes(bytes);
            return Convert.ToBase64String(bytes);
        }
        /// <summary>
        /// Authentication token to be used for Bearer authentication
        /// </summary>
        /// <param name="scope">Should be 'code' or 'admin'</param>
        /// <param name="key">code or admin key</param>
        /// <param name="signerKey">ECDSA crypto service</param>
        /// <returns>JWT Token</returns>
        [HttpGet("token/{scope}/{key}")]
        public string GetToken(string scope, string key, [FromServices] IECDsaCryptoKey signerKey)
        {
            if (key == null) key = "";
            AuthData auth = Authorize(scope, key);

            var claims = new Claim[]
            {
                new Claim(ClaimTypes.NameIdentifier, scope),
                new Claim("AuthDataJson", Newtonsoft.Json.JsonConvert.SerializeObject(auth))
            };
            var tokenValidTimeInMinutes = Configuration.GetValue("TokenValidTimeInMinutes", (180 * 60 * 24));
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
            string jwtToken = new JwtSecurityTokenHandler().WriteToken(token);
            return jwtToken;
        }
        /// <summary>
        ///Current authorization status
        /// </summary>
        /// <returns>Auth Data</returns>
        [HttpGet("whoami")]
        public AuthData WhoAmI()
        {
            var auth = AuthHelper.GetAuthData(User, Configuration);
            var tours = tourStorage.GetTours((x) => (x.AccessCodeMD5 != null && auth.AccessCodeMD5 == x.AccessCodeMD5), true, 0, int.MaxValue, out var tc);
            auth.TourIds = tours.Any() 
                ? tours.Select(t => t.Id).ToList()
                : new string[0].ToList();
            return auth;
        }

        [HttpGet("config_values")]
        public Dictionary<string, RequestedConfigValue> GetConfigValues()
        {
            var auth = AuthHelper.GetAuthData(User, Configuration);
            if (!auth.IsMaster) throw HttpException.Forbid("You are not admin");
            return (Configuration as TcConfiguration)?.RequestedValues ?? new Dictionary<string, RequestedConfigValue>();
        }

        private AuthData Authorize(string scope, string accessCode)
        {
            AuthData auth = new AuthData();
            if (scope == "admin")
            {
                // generate for master key
                string keyProvided = accessCode;
                string keyFromConfig = Configuration.GetValue<string>("MasterKey");
                if (keyProvided == keyFromConfig)
                {
                    auth.Type = "Master";
                    auth.IsMaster = true;
                } else
                {
                    throw HttpException.NotAuthenticated($"Wrong Master Key");
                }
            }
            /*else if (scope == "user")
            {
                // can create tours, but not delete. TODO: think about it
                string keyProvided = accessCode;
                string keyFromConfig = Configuration.GetValue<string>("UserKey");
                if (keyProvided == keyFromConfig)
                {
                    auth.Type = "User";
                    auth.IsMaster = false;
                }
            }*/
            else if (scope == "code")
            {
                // TODO change to user-related 
                // Access code
                // get all tours with given access code
                auth.Type = "AccessCode";
                auth.IsMaster = false;
                auth.AccessCodeMD5 = AuthHelper.CreateMD5(accessCode);
            } else
            {
                throw HttpException.NotAuthenticated("Wrong scope. Please try 'code' or 'admin'. Or 'pigeon', who knows. Maybe 'slippery' will work.");
            }

            return auth;
        }
    }
}
