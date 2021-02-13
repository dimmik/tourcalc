using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Threading.Tasks;

namespace TourCalcWebApp.Utils
{
    public static class HttpUtils
    {
        public static async Task SendResponseWithKeepalive(
            this Microsoft.AspNetCore.Http.HttpResponse r,
            int delayInSec,
            string contentType,
            Func<string> howToGetResult,
            int statusCode = 200,
            TimeSpan? timeout = null
            )
        {
            Stopwatch timer = Stopwatch.StartNew();
            r.StatusCode = statusCode;
            r.ContentType = contentType;
            var sw = new StreamWriter(r.Body);
            bool respReady = false;
            string response = null;

            var task = Task.Run(
                () =>
                {
                    response = howToGetResult();
                    respReady = true;
                }
                );
            while (!respReady && timer.Elapsed < (timeout != null ? timeout.Value : TimeSpan.MaxValue))
            {
                await sw.WriteAsync($"\r\n").ConfigureAwait(false);
                await sw.FlushAsync().ConfigureAwait(false);
                await Task.Delay(delayInSec * 1000);
            }
            await task;
            await sw.WriteAsync($"{response}\r\n").ConfigureAwait(false);
            await sw.FlushAsync().ConfigureAwait(false);
            await r.Body.FlushAsync().ConfigureAwait(false);
        }
    }
}
