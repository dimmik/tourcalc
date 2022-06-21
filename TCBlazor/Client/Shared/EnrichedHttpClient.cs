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
        private readonly LocalLogger logger;

        public EnrichedHttpClient(HttpClient httpClient, MessageService messageService, LocalLogger logger)
        {
            _httpClient = httpClient ?? throw new ArgumentNullException(nameof(httpClient));
            _messageService = messageService ?? throw new ArgumentNullException(nameof(messageService));
            this.logger = logger ?? throw new ArgumentNullException(nameof(logger));
        }
        public HttpClient Http => _httpClient;

        public async Task<string> GetStringAsync(string url, bool showErrorMessages = true)
        {
            try
            {
                Stopwatch sw = Stopwatch.StartNew();
                var s = await Http.GetStringAsync(url);
                sw.Stop();
                logger.Log($"GET to {url} finished in {sw.Elapsed}");
                return s;
            }
            catch (Exception e)
            {
                if (showErrorMessages) ShowError(e.Message);
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

        public async Task<T?> CallWithAuthToken<T>(string url, string token, bool showErrorMessages = true)
        {
            T? r = await CallWithAuthToken<T>(url, token, HttpMethod.Get, null, showErrorMessages);
            return r;
        }

        public async Task<T?> CallWithAuthToken<T>(string url, string token, HttpMethod method, object? body, bool showErrorMessages = true)
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
                    if (showErrorMessages) ShowError($"{(int)resp.StatusCode} {resp.StatusCode}: {m}");
                }
                //_messageService.Destroy();
                sw.Stop();
                logger.Log($"{method} to {url} finished in {sw.Elapsed}");
                return t;
            } 
            catch (Exception e)
            {
                if (showErrorMessages) ShowError($"{url}: {e.Message}");
                return default;
            }
        }
        public void ShowError(string txt)
        {
            _messageService.Error(getMessage(txt));
            logger.Log(txt);
        }
    }
}
