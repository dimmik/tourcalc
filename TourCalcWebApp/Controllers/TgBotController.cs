using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Logging;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using Telegram.Bot;
using Telegram.Bot.Types;
using Telegram.Bot.Types.Enums;
using TourCalcWebApp.TgBot;

namespace TourCalcWebApp.Controllers
{
    [Route("api/[controller]")]
    [ApiController]
    public class TgBotController : ControllerBase
    {
        ITcConfiguration Configuration { get;  }
        IBotService botService { get;  }
        ILogger Log { get; }
        public TgBotController(ITcConfiguration config, IBotService bs, ILogger<TgBotController> logger)
        {
            Configuration = config;
            botService = bs;
            Log = logger;
        }
        // /api/tgbot/update
        [HttpPost("update")]
        public async Task<IActionResult> Update([FromBody] Update update)
        {
            if (update.Type != UpdateType.Message)
                return Ok();
            var message = update.Message;

            Log.LogInformation("Received Message from {0}", message.Chat.Id);
            if (message.Type == MessageType.Text)
            {
                await botService.Client.SendTextMessageAsync(message.Chat.Id, $"Received {message.Text}");
            } else
            {
                await botService.Client.SendTextMessageAsync(message.Chat.Id, $"Received non-text message");
            }
            return Ok();
        }
    }

}
