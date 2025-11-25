using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using Telegram.Bot;

namespace Company.TCBlazor.TgBot
{
    public interface IBotService
    {
        TelegramBotClient Client { get; }
        bool IsTokenValid(string token);
    }
}
