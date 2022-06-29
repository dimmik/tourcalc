using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.Logging;
using System;
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
        public StartupInfo GetStartupTime()
        {
            return startupInfo;
        }
        private static readonly HttpClient client = new() { Timeout = TimeSpan.FromMinutes(10) };
        [HttpGet("wakeup/{code}")]
        public string Wakeup([FromRoute] string code)
        {
            var secretCode = Configuration.GetValue("WakeupCode", "secCode");
            if (code != secretCode) return "wrong code";
            lock (client)
            {
                startupInfo.Wakeup();
                Logger.LogInformation($"{DateTimeOffset.Now}: Wakeup");
                // wait xxx min
                var delay = Configuration.GetValue("WaketimePreDelayInMin", 1);
                Logger.LogInformation($"{DateTimeOffset.Now}: Wakeup waiting {delay} mins");
                Task.Delay(TimeSpan.FromMinutes(delay)).Wait();
                // call wakeup url (in background)
                var url = Configuration.GetValue<string>("WakeupUrl");
                Logger.LogInformation($"{DateTimeOffset.Now}: call {url}");
                Task t = client.GetAsync(url);
                // wait another yyy min
                delay = Configuration.GetValue("WaketimePostDelayInMin", 1);
                Logger.LogInformation($"{DateTimeOffset.Now}: Wakeup waiting {delay} mins");
                Task.Delay(TimeSpan.FromMinutes(delay)).Wait();
                Logger.LogInformation($"{DateTimeOffset.Now}: Wakeup call task delay status {t.Status}");

                return "ok";
            }
        }
    }
}
