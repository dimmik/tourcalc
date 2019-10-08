using System;
using System.Diagnostics;
using System.IO;
using System.Net.Http;
using System.Net.Http.Headers;
using System.Text;
using Newtonsoft.Json;
using TCalc.Domain;

namespace ConsoleTest
{
    class Program
    {
        static void Main(string[] args)
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
