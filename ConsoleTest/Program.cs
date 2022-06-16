using System;
using System.Diagnostics;
using System.IO;
using System.Net.Http;
using System.Net.Http.Headers;
using System.Text;
using Newtonsoft.Json;
using TCalc.Domain;
using TCalcCore.Auth;
using Telegram.Bot;
using Telegram.Bot.Args;

namespace ConsoleTest
{
    class Program
    {
        static void Main(string[] args)
        {
            var str = @"{""Type"":""AccessCode"",""IsMaster"":false,""AccessCodeMD5"":""1C0369EA42B2F746D3DF1E66BCB2DE46"",""TourIds"":[]}";
            AuthData ad = System.Text.Json.JsonSerializer.Deserialize<AuthData>(str);
        }
        static void Maindddd(string[] args)
        {
            
            botClient = new TelegramBotClient("837732971:AAGSZXh9WcoyvmuuI8b2DzsHsxMV_1dlZBk");

            var me = botClient.GetMeAsync().Result;
            Console.WriteLine(
              $"Hello, World! I am user {me.Id} and my name is {me.FirstName}."
            );

            botClient.OnMessage += Bot_OnMessage;
            botClient.StartReceiving();

            Console.WriteLine("Press any key to exit");
            Console.ReadKey();

            botClient.StopReceiving();
        }
        static ITelegramBotClient botClient;
        static async void Bot_OnMessage(object sender, MessageEventArgs e)
        {
            if (e.Message.Text != null)
            {
                Console.WriteLine($"Received a text message in chat {e.Message.Chat.Id}.");

                await botClient.SendTextMessageAsync(
                  chatId: e.Message.Chat,
                  text: "You said:\n" + e.Message.Text
                );
            }
        }
        static void Mainx(string[] args)
        {
            // get token
            var tokenUrl = @"https://localhost:5001/api/auth/token/code/test";
            string token = "";
            using (var c = new HttpClient())
            {
                var tt = c.GetAsync(tokenUrl).GetAwaiter().GetResult();
                token = tt.Content.ReadAsStringAsync().GetAwaiter().GetResult();
            }
            // perofrm actions
            var tours = 1000;
            var tourJson = File.ReadAllText(@"c:\tmp\testtour.json");
            var tour = JsonConvert.DeserializeObject<Tour>(tourJson);
            using (var c = new HttpClient())
            {
                c.DefaultRequestHeaders.Authorization =
                    new AuthenticationHeaderValue("Bearer", token);
                for (var i = 0; i < tours; i++)
                {
                    Stopwatch sw = Stopwatch.StartNew();
                    tour.Name = $"Тестовый Тур №{i.ToString("0000")}";
                    var content = JsonConvert.SerializeObject(tour);
                    var hContent = new StringContent(content, Encoding.UTF8, "application/json");
                    var resp = c.PostAsync(@"https://localhost:5001/api/tour/add/test", hContent).GetAwaiter().GetResult();
                    if (i%10 == 0)
                    {
                        Console.WriteLine($"Tours added: {i} in {sw.ElapsedMilliseconds}ms");
                    }
                }
            }
            Console.WriteLine("done");
            Console.ReadKey();
        }
    }
}
