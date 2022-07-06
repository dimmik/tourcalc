using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using System;
using System.Collections.Generic;
using System.Text;
using System.Threading.Tasks;
using TCalcCore.Auth;
using TCalcCore.Storage;
using TourCalcWebApp.Auth;

namespace TourCalcWebApp.Controllers
{
    [Route("api/[controller]")]
    [Authorize]
    [AllowAnonymous]
    [ApiController]
    public class LogController : ControllerBase
    {
        private readonly ILogStorage _storage;
        private readonly ITcConfiguration _config;

        public LogController(ILogStorage storage, ITcConfiguration config)
        {
            _storage = storage;
            _config = config;
        }

        [HttpGet("x/{log}")]
        public void Log([FromRoute]string log)
        {
            //  '._-' -> '+/='
            string msg = Encoding.UTF8.GetString(Convert.FromBase64String(log.Replace(".", "+").Replace("_", "/").Replace("-", "=")));
            string ip = HttpContext.Connection.RemoteIpAddress.ToString();
            string userAgent = Request.Headers["User-Agent"];
            var logEntity = new RLogEntry(msg, ip, userAgent);
            _storage.StoreLog(logEntity);
        }
        [HttpGet("logs")]
        public async Task<IEnumerable<RLogEntry>> GetLogs([FromQuery]int hoursAgoFrom = int.MaxValue, [FromQuery]int hoursAgoTo = 0)
        {
            AuthData authData = AuthHelper.GetAuthData(User, _config);
            if (!authData.IsMaster) return new List<RLogEntry>();
            return await _storage.GetLogEntries(hoursAgoFrom, hoursAgoTo);
        }
    }
    
}
