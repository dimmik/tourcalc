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
        private string Token;
        public BotService(string token)
        {
            Client = new TelegramBotClient(token);
            Token = token;
        }

        public bool IsTokenValid(string token)
        {
            return Token == token;
        }
    }
}
