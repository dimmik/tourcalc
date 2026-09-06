using System.IO;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Newtonsoft.Json;
using Telegram.Bot.Types;

namespace TCBlazor.Server.Telegram
{
    /// <summary>
    /// Where Telegram delivers updates in production.
    ///
    /// The address alone is not a secret worth relying on, so every request must carry the
    /// secret token Telegram was told to send; without it anyone who guessed the path
    /// could feed the bot forged updates.
    /// </summary>
    [ApiController]
    [Route("api/tg")]
    [AllowAnonymous]
    public class TelegramWebhookController : ControllerBase
    {
        private readonly TelegramBotOptions options;
        private readonly TgDispatcher dispatcher;

        public TelegramWebhookController(TelegramBotOptions options, TgDispatcher dispatcher = null)
        {
            this.options = options;
            this.dispatcher = dispatcher;
        }

        [HttpPost("update")]
        public async Task<IActionResult> Update(CancellationToken token)
        {
            if (dispatcher == null || !options.IsWebhook) return NotFound();

            // checked before the body is even read: an unauthenticated caller should not
            // get us to parse anything on their behalf
            var provided = Request.Headers["X-Telegram-Bot-Api-Secret-Token"].ToString();
            if (string.IsNullOrEmpty(options.WebhookSecret) || provided != options.WebhookSecret)
            {
                return Unauthorized();
            }

            // Deserialized by hand, with Newtonsoft. Telegram.Bot's types carry Newtonsoft
            // attributes and converters - "date" is a Unix timestamp, for one - and this
            // app configures MVC with System.Text.Json, which rejected every real update
            // with "The JSON value could not be converted to System.DateTime".
            Update update;
            using (var reader = new StreamReader(Request.Body))
            {
                var body = await reader.ReadToEndAsync();
                try
                {
                    update = JsonConvert.DeserializeObject<Update>(body);
                }
                catch
                {
                    return BadRequest();
                }
            }
            if (update == null) return BadRequest();

            await dispatcher.Dispatch(update, token);
            return Ok();
        }
    }
}
