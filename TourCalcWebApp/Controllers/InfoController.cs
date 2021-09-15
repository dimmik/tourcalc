using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.Logging;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Net.Http;
using System.Threading.Tasks;

namespace TourCalcWebApp.Controllers
{
    [Route("api/[controller]")]
    [ApiController]
    public class InfoController : ControllerBase
    {
        private readonly StartupInfo startupInfo;
        private readonly IConfiguration Configuration;
        private readonly ILogger Logger;

        public InfoController(StartupInfo startupInfo, IConfiguration conf, ILogger<InfoController> log)
        {
            this.startupInfo = startupInfo;
            Configuration = conf;
            Logger = log;
        }

        [HttpGet("start")]
        public DateTimeOffset GetStartupTime()
        {
            return startupInfo.StartTime.ToOffset(TimeSpan.FromHours(3));
        }
        private static HttpClient client = new() { Timeout = TimeSpan.FromMinutes(10) };
        [HttpGet("wakeup/{code}")]
        public async Task<string> Wakeup([FromRoute] string code)
        {
            var secretCode = Configuration.GetValue("WakeupCode", "secCode");
            if (code != secretCode) return "wrong code";
            Logger.LogInformation($"{DateTimeOffset.Now}: Wakeup");
            // wait xxx min
            var delay = Configuration.GetValue("WaketimePreDelayInMin", 5);
            Logger.LogInformation($"{DateTimeOffset.Now}: Wakeup waiting {delay} mins");
            await Task.Delay(TimeSpan.FromMinutes(delay));
            // call wakeup url (in background)
            var url = Configuration.GetValue<string>("WakeupUrl");
            Logger.LogInformation($"{DateTimeOffset.Now}: call {url}");
            Task t = client.GetAsync(url);
            // wait another yyy min
            delay = Configuration.GetValue("WaketimePostDelayInMin", 3);
            Logger.LogInformation($"{DateTimeOffset.Now}: Wakeup waiting {delay} mins");
            await Task.Delay(TimeSpan.FromMinutes(delay));
            Logger.LogInformation($"{DateTimeOffset.Now}: Wakeup call task delay status {t.Status}");

            return "ok";
        }
    }
    public class StartupInfo
    {
        public readonly DateTimeOffset StartTime = DateTimeOffset.Now;
    }
}
