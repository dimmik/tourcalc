using System;
using System.Diagnostics;
using System.IO;
using System.Net.Http;
using System.Net.Http.Headers;
using System.Text;
using System.Threading;
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
            CancellationTokenSource cts = new CancellationTokenSource();
            var t1 = cts.Token;
            cts.Cancel();
            var t2 = cts.Token;
            cts.Cancel();
        }
        
    }
}
