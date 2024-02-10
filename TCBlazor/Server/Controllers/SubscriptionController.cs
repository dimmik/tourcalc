using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using TCalcCore.Storage;
using TCBlazor.Client.SharedCode;

// For more information on enabling Web API for empty projects, visit https://go.microsoft.com/fwlink/?LinkID=397860

namespace TCBlazor.Server.Controllers
{
    [Route("api/[controller]")]
    [Authorize]
    [ApiController]
    public class SubscriptionController : ControllerBase
    {
        // GET: api/<SubscriptionController>
        [HttpPost("subscribe/{tourId}")]
        public string Subscribe([FromRoute] string tourId, [FromBody]NotificationSubscription sub, [FromServices] ISubscriptionStorage Storage)
        {
            //return new string[] { "value1", "value2" };
            Storage.AddSubscription(tourId, sub);
            return "OK";
        }
    }
}
