using AntDesign;
using Microsoft.AspNetCore.Components;
using Newtonsoft.Json;
using System.Diagnostics;
using System.Text;

namespace TCBlazor.Client.Shared
{
    public class EnrichedHttpClient
    {
        private readonly HttpClient _httpClient;
        private readonly MessageService _messageService;

        public EnrichedHttpClient(HttpClient httpClient, MessageService messageService)
        {
            _httpClient = httpClient ?? throw new ArgumentNullException(nameof(httpClient));
            _messageService = messageService ?? throw new ArgumentNullException(nameof(messageService));
        }
        public HttpClient Http => _httpClient;

        public async Task<string> GetStringAsync(string url)
        {
            try
            {
                Stopwatch sw = Stopwatch.StartNew();
                var s = await Http.GetStringAsync(url);
                sw.Stop();
                Console.WriteLine($"GET to {url} finished in {sw.Elapsed}");
                return s;
            }
            catch (Exception e)
            {
                ShowError(e.Message);
                throw;
            }
        }
        private static RenderFragment getMessage(string message)
        {
            return __builder =>
            {
                __builder.AddContent(0, message);
            };
        }

        public async Task<T?> CallWithAuthToken<T>(string url, string token)
        {
            T? r = await CallWithAuthToken<T>(url, token, HttpMethod.Get, null);
            return r;
        }

        public async Task<T?> CallWithAuthToken<T>(string url, string token, HttpMethod method, object? body)
        {
            try
            {
                Stopwatch sw = Stopwatch.StartNew();
                var request = new HttpRequestMessage(method, url);
                request.Headers.Authorization = new System.Net.Http.Headers.AuthenticationHeaderValue("bearer", token);
                if (body != null)
                {
                    request.Content = new StringContent(JsonConvert.SerializeObject(body), Encoding.UTF8, "application/json");
                }
                var resp = await Http.SendAsync(request);
                T? t = default;
                if (resp.IsSuccessStatusCode)
                {
                    var s = await resp.Content.ReadAsStringAsync();
                    try
                    {
                        t = JsonConvert.DeserializeObject<T>(s);
                    }
                    catch
                    {
                        try
                        {
                            // (maybe s is just a string?)
                            s = $"\"{s}\"";
                            t = JsonConvert.DeserializeObject<T>(s);
                        }
                        catch
                        {
                            // no luck
                        }
                    }
                }
                else
                {
                    var m = await resp.Content.ReadAsStringAsync();
                    ShowError($"{(int)resp.StatusCode} {resp.StatusCode}: {m}");
                }
                //_messageService.Destroy();
                sw.Stop();
                Console.WriteLine($"{method} to {url} finished in {sw.Elapsed}");
                return t;
            } 
            catch (Exception e)
            {
                ShowError($"Error: {e.Message}");
                return default;
            }
        }
        public void ShowError(string txt)
        {
            _messageService.Error(getMessage(txt));
        }
    }
}
