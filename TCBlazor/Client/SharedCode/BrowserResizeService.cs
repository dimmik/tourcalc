using Microsoft.JSInterop;
using System;
using System.Threading.Tasks;

namespace TCBlazor.Client.SharedCode
{
    public class BrowserResizeService
    {
        public static event Func<Task>? OnResize;

        [JSInvokable(nameof(OnBrowserResize))]
        public static async Task OnBrowserResize()
        {
            await (OnResize?.Invoke() ?? Task.CompletedTask);
        }
    }
}