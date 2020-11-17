using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Logging;
using Newtonsoft.Json;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using TCalc.Storage;
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
        private readonly ITcConfiguration Configuration;
        private readonly IBotService botService;
        private readonly ILogger Log;
        private readonly ITourStorage TourStorage;
        public TgBotController(ITcConfiguration config, IBotService bs, ILogger<TgBotController> logger, ITourStorage storage)
        {
            Configuration = config;
            botService = bs;
            Log = logger;
            TourStorage = storage;
        }
        // /api/tgbot/update
        [HttpPost("update/{token}")]
        public async Task<IActionResult> Update([FromBody] Update update, [FromRoute] string token)
        {
            if (!botService.IsTokenValid(token)) return NotFound("Cannot find the bot service");

            if (update.Type != UpdateType.Message)
            {
                // do nothing
                return Ok();
            }
            try
            {
                await ProcesMessage(update.Message);
            }
            catch (Exception e)
            {
                await botService.Client.SendTextMessageAsync(update.Message.Chat.Id, $"err: {e.Message} {e.StackTrace} [{JsonConvert.SerializeObject(update)}]");
            }
            return Ok();
        }

        private async Task ProcesMessage(Message message)
        {
            var tourBotSvc = new TourBotService(TourStorage, message.Chat, message.From);
            string[] entities = message.EntityValues?.ToArray() ?? message.Text.Split(" ").Where(s => !string.IsNullOrWhiteSpace(s)).ToArray();
            if (entities.Length < 1)
            {
                await botService.Client.SendTextMessageAsync(message.Chat.Id, "Hmm. Something strange happened.");
            }
            else
            {
                var cmd = entities[0];
                var resp = tourBotSvc.Perform(cmd, entities.Skip(1).ToArray());
                await botService.Client.SendTextMessageAsync(message.Chat.Id, resp);
            }
        }
    }

}
