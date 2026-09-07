using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Newtonsoft.Json;
using TCalc.Domain;
using TCalc.Storage;
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
        private readonly ITourStorage tourStorage;

        public TelegramWebhookController(TelegramBotOptions options, ITourStorage tourStorage, TgDispatcher dispatcher = null)
        {
            this.options = options;
            this.tourStorage = tourStorage;
            this.dispatcher = dispatcher;
        }

        public class WebAppEnterRequest
        {
            public string Tour { get; set; }
            public string InitData { get; set; }
        }

        /// <summary>
        /// Lets a Mini App in. Telegram signs who is looking with a key derived from the bot
        /// token, so the signature is checked first and everything the page said about itself
        /// before that counts for nothing.
        ///
        /// Being a genuine Telegram user is not enough on its own: the tour is opened only
        /// for somebody who is actually on it. That is what keeps the access code out of the
        /// button - it is handed over here, after the check, rather than travelling in a link
        /// anyone in the chat could forward.
        /// </summary>
        [HttpPost("webapp/enter")]
        public IActionResult WebAppEnter([FromBody] WebAppEnterRequest request)
        {
            if (!options.Enabled) return NotFound();
            if (request == null || string.IsNullOrWhiteSpace(request.Tour)) return BadRequest();

            var verified = TgWebAppAuth.Validate(request.InitData, options.Token,
                TimeSpan.FromHours(24), DateTimeOffset.UtcNow);
            if (verified == null) return Unauthorized();

            var tour = tourStorage.GetTour(request.Tour);
            if (tour == null) return NotFound();

            var isOnTheTrip = (tour.Persons ?? new List<Person>())
                .Any(p => TgMeta.UserId(p) == verified.UserId);
            if (!isOnTheTrip) return Forbid();

            return new JsonResult(new { url = $"/goto/{tour.AccessCodeMD5}/{tour.Id}" });
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
