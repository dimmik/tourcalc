using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using Telegram.Bot;

namespace TourCalcWebApp.TgBot
{
    public class BotService : IBotService
    {
        public TelegramBotClient Client { get; private set;  }
        public BotService(string token)
        {
            Client = new TelegramBotClient(token);
        }
    }
}
