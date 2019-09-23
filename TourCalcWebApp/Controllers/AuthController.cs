using System;
using System.Collections.Generic;
using System.IdentityModel.Tokens.Jwt;
using System.Linq;
using System.Security.Claims;
using System.Threading.Tasks;
using LiteDB;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Configuration;
using Microsoft.IdentityModel.Tokens;
using Org.BouncyCastle.Security;
using TCalc.Domain;
using TCalc.Storage;
using TourCalcWebApp.Auth;

// For more information on enabling MVC for empty projects, visit https://go.microsoft.com/fwlink/?LinkID=397860

namespace TourCalcWebApp.Controllers
{
    [Route("api/[controller]")]
    [Authorize]
    [AllowAnonymous]
    [ApiController]
    public class AuthController : ControllerBase
    {
        private readonly IConfiguration Configuration;
        private readonly ITourStorage tourStorage;


        public AuthController(IConfiguration config, ITourStorage storage)
        {
            Configuration = config;
            tourStorage = storage;
        }

        [HttpGet("random/{numb=32}")]
        public IActionResult GenerateRandomKey(int numb)
        {
            var r = new SecureRandom();
            if (numb > 8192) numb = 8192;
            byte[] bytes = new byte[numb];
            r.NextBytes(bytes);
            return Ok(Convert.ToBase64String(bytes));
        }

        [HttpGet("token/{scope}/{key}")]
        public IActionResult GetToken(string scope, string key, [FromServices] IECDsaCryptoKey signerKey)
        {
            if (key == null) key = "";
            AuthData auth = Authorize(scope, key);

            var claims = new Claim[]
            {
                new Claim(ClaimTypes.NameIdentifier, key),
                new Claim("AuthDataJson", Newtonsoft.Json.JsonConvert.SerializeObject(auth))
            };
            var token = new JwtSecurityToken(
                issuer: "TourCalc",
                audience: "Users",
                claims: claims,
                expires: DateTime.Now.AddMinutes(5 * 60 * 24), // 5 days TODO: configuration
                signingCredentials: new SigningCredentials(
                                       signerKey.GetPrivateKey(),
                                       signerKey.SigningAlgorithm
                                       )
                );
            string jwtToken = new JwtSecurityTokenHandler().WriteToken(token);
            return Ok(jwtToken);
        }
        [HttpGet("whoami")]
        public IActionResult WhoAmI()
        {
            var auth = AuthHelper.GetAuthData(User, Configuration);
            auth.TourIds = tourStorage.GetTours((x) => (x.AccessCodeMD5 != null && auth.AccessCodeMD5 == x.AccessCodeMD5))
                .Select(t => t.Id).ToList();
            return Ok(auth);
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
                }
            }
            else if (scope == "user")
            {
                // can create tours, but not delete. TODO: think about it
                string keyProvided = accessCode;
                string keyFromConfig = Configuration.GetValue<string>("UserKey");
                if (keyProvided == keyFromConfig)
                {
                    auth.Type = "User";
                    auth.IsMaster = false;
                }
            }
            else if (scope == "code")
            {
                // TODO change to user-related 
                // Access code
                // get all tours with given access code
                auth.Type = "AccessCode";
                auth.IsMaster = false;
                auth.AccessCodeMD5 = AuthHelper.CreateMD5(accessCode);
            }

            return auth;
        }
    }
}
