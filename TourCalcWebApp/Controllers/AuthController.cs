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
using TCalc.Domain;
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
        private string dbFilePath => Configuration.GetValue<string>("DatabasePath");//= @"C:\tmp\Tour2.db";

        public AuthController(IConfiguration config)
        {
            Configuration = config;
        }
        [HttpGet("ticksb64")]
        public IActionResult GetTicksB64()
        {
            var t = DateTime.Now.Millisecond;
            var tb = BitConverter.GetBytes(t);
            var tb64 = Convert.ToBase64String(tb).Substring(0,6);
            return Ok(tb64);
        }
        [HttpGet("token/{key}")]
        public IActionResult GetToken(string key, [FromServices] IECDsaCryptoKey signerKey)
        {
            if (key == null) key = "";
            AuthData auth = Authorize(key);

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

        private AuthData Authorize(string key)
        {
            AuthData auth = new AuthData();
            if (key.StartsWith("m:"))
            {
                // generate for master key
                string keyProvided = key.Substring("m:".Length);
                string keyFromConfig = Configuration.GetValue<string>("MasterKey");
                if (keyProvided == keyFromConfig)
                {
                    auth.Type = "Master";
                    auth.IsMaster = true;
                }
            }
            else if (key.StartsWith("U:"))
            {
                // generate for creation of tour
                string keyProvided = key.Substring("m:".Length);
                string keyFromConfig = Configuration.GetValue<string>("UserKey");
                if (keyProvided == keyFromConfig)
                {
                    auth.Type = "User";
                    auth.IsMaster = false;
                }
            }
            else
            {
                // Access code
                // get all tours with given access code
                using (var db = new LiteDatabase(dbFilePath))
                {
                    auth.Type = "AccessCode";
                    auth.IsMaster = false;
                    auth.AccessCode = key;
                }

            }

            return auth;
        }
    }
    public class AuthData
    {
        public string Type { get; set; }
        public bool IsMaster { get; set; } = false; // allow all
        public string AccessCode { get; set; }
    }
}
