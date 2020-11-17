using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Logging;
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
        public async Task<IActionResult> Update([FromBody] Update update, [FromRoute]string token)
        {
            if (!botService.IsTokenValid(token)) return NotFound("Cannot find the bot service");
            try
            {
                if (update.Type != UpdateType.Message)
                    return Ok();
                var message = update.Message;

                if (message.Chat.Type != ChatType.Group)
                {
                    await botService.Client.SendTextMessageAsync(message.Chat.Id, $"I work in groups only, sorry");
                    return Ok();
                }
                var commands = new string[] { "/new", "/addme", "/show" };
                if (message.Entities.Any() && message.Entities[0].Type == MessageEntityType.BotCommand)
                {
                    var command = message.EntityValues.First();
                    if (!commands.Any(c => c == command.ToLower()))
                    {
                        await botService.Client.SendTextMessageAsync(message.Chat.Id, $"I understand the following: {string.Join(",", commands)}");
                        return Ok();
                    }
                    if (command == "/new")
                    {
                        var rest = command.Substring(4);
                        var tour = TourStorage.GetTour($"tg-{message.Chat.Id}");
                        if (tour != null)
                        {
                            await botService.Client.SendTextMessageAsync(message.Chat.Id, $"There is already tour for this chat");
                        }
                        else
                        {
                            TourStorage.AddTour(new TCalc.Domain.Tour()
                            {
                                Id = $"tg-{message.Chat.Id}",
                                Name = string.IsNullOrWhiteSpace(rest) ? $"Tg Tour for {message.Chat.Title}" : rest
                            });
                        }
                        return Ok();
                    }
                    if (command == "/addme")
                    {
                        var tour = TourStorage.GetTour($"tg-{message.Chat.Id}");
                        if (tour == null)
                        {
                            // TODO - add automatically in the future
                            await botService.Client.SendTextMessageAsync(message.Chat.Id, $"First create a tour with /new");
                        }
                        else
                        {
                            if (tour.Persons.Any(p => p.GUID == $"{message.From.Id}"))
                            {
                                await botService.Client.SendTextMessageAsync(message.Chat.Id, $"You ({message.From.FirstName} {message.From.LastName}) are already here");
                                return Ok();
                            }
                            tour.Persons.Add(new TCalc.Domain.Person()
                            {
                                GUID = $"{message.From.Id}",
                                Name = $"{message.From.FirstName} {message.From.LastName}"
                            });
                            TourStorage.StoreTour(tour);
                        }
                    }
                    if (command == "/show")
                    {
                        var tour = TourStorage.GetTour($"tg-{message.Chat.Id}");
                        if (tour == null)
                        {
                            await botService.Client.SendTextMessageAsync(message.Chat.Id, $"First create a tour with /new");
                        }
                        else
                        {
                            var msg = string.Join("\r\n", tour.Persons.Select(p => $"{p.Name}"));
                            await botService.Client.SendTextMessageAsync(message.Chat.Id, $"Tour '{tour.Name}'\r\nPersons ({tour.Persons.Count}):\r\n{msg}");
                            return Ok();
                        }
                    }
                }
                else
                {
                    await botService.Client.SendTextMessageAsync(message.Chat.Id, $"Je ne parle pas. I understand the following: {string.Join(",", commands)}");
                    return Ok();
                }

                return Ok();
            } catch (Exception e)
            {
                await botService.Client.SendTextMessageAsync(update.Message.Chat.Id,$"Error {e.Message} {e.StackTrace}");
                return Ok();
            }
        }
    }

}
