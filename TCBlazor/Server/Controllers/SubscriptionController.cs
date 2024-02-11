using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using TCalcCore.Storage;
using TCalcCore.UI;
using TCBlazor.Client.SharedCode;
using TourCalcWebApp;

// For more information on enabling Web API for empty projects, visit https://go.microsoft.com/fwlink/?LinkID=397860

namespace TCBlazor.Server.Controllers
{
    [Route("api/[controller]")]
    [Authorize]
    [ApiController]
    public class SubscriptionController : ControllerBase
    {


        [HttpGet("publickey")]
        public string GetPublicKey([FromServices]ITcConfiguration config)
        {
            return config.GetValue<string>("PushNotificationPublicKey");
        }
        [HttpPost("check/{tourId}")]
        public bool Check([FromRoute] string tourId, [FromBody] NotificationSubscription sub, [FromServices] ISubscriptionStorage Storage)
        {
            return Storage.CheckSubscription(tourId, sub);
        }

        [HttpPost("subscribe/{tourId}")]
        public string Subscribe([FromRoute] string tourId, [FromBody] NotificationSubscription sub, [FromServices] ISubscriptionStorage Storage)
        {
            Storage.AddSubscription(tourId, sub);
            return "OK";
        }
        [HttpPost("unsubscribe/{tourId}")]
        public string Unsubscribe([FromRoute] string tourId, [FromBody] NotificationSubscription sub, [FromServices] ISubscriptionStorage Storage)
        {
            Storage.RemoveSubscription(tourId, sub);
            return "OK";
        }
    }
}
