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
using TCalcCore.Auth;
using Company.TCBlazor.Auth;
using Company.TCBlazor.Exceptions;
using Company.TCBlazor.Utils;

namespace Company.TCBlazor.Controllers
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
        /// <param name="isMd5">if provided, code is treaten as md5 hash of real code</param>
        /// <param name="signerKey">ECDSA crypto service</param>
        /// <returns>JWT Token</returns>
        [HttpGet("token/{scope}/{key}/{*isMd5}")]
        public string GetToken([FromServices] IECDsaCryptoKey signerKey, string scope, string key, string? isMd5 = null)
        {

            AuthData auth = AccessAuthorizer.Authorize(Configuration, scope, key, (isMd5 != null));
            return AccessAuthorizer.CreateToken(Configuration, signerKey, scope, auth);
        }
        /// <summary>
        ///Current authorization status
        /// </summary>
        /// <returns>Auth Data</returns>
        [HttpGet("whoami")]
        public AuthData WhoAmI()
        {
            var auth = AuthHelper.GetAuthData(User, Configuration);
            //var tours = tourStorage.GetTours((x) => (x.AccessCodeMD5 != null && auth.AccessCodeMD5 == x.AccessCodeMD5), true, 0, int.MaxValue, out var tc);
            //auth.TourIds = tours.Any() 
            //    ? tours.Select(t => t.Id).ToList()
            //    : new string[0].ToList();
            return auth;
        }

    }
}
