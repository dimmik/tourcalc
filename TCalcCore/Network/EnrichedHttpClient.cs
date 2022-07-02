using Newtonsoft.Json;
using System;
using System.Diagnostics;
using System.Net.Http;
using System.Text;
using System.Threading.Tasks;
using TCalcCore.Logging;
using TCalcCore.UI;

namespace TCalcCore.Network
{
    public class EnrichedHttpClient
    {
        private readonly HttpClient _httpClient;
        private readonly ISimpleMessageShower _messageShower;
        private readonly ILocalLogger logger;

        public EnrichedHttpClient(HttpClient httpClient, ISimpleMessageShower messageShower, ILocalLogger logger)
        {
            _httpClient = httpClient ?? throw new ArgumentNullException(nameof(httpClient));
            _messageShower = messageShower ?? throw new ArgumentNullException(nameof(messageShower));
            this.logger = logger ?? throw new ArgumentNullException(nameof(logger));
        }
        private HttpClient Http => _httpClient;

        public async Task<string> GetStringAsync(string url, Action<string> onError)
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
                onError?.Invoke(e.Message);
                throw;
            }
        }
        

        public async Task<T> CallWithAuthToken<T>(string url, string token, Action<string> onError)
        {
            T r = await CallWithAuthToken<T>(url, token, HttpMethod.Get, null, onError);
            return r;
        }

        public async Task<T> CallWithAuthToken<T>(string url, string token, HttpMethod method, object body, Action<string> onError)
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
                T t = default;
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
                    onError?.Invoke($"{(int)resp.StatusCode} {resp.StatusCode}: {m}");
                }
                //_messageService.Destroy();
                sw.Stop();
                logger.Log($"{method} to {url} finished in {sw.Elapsed}");
                return t;
            }
            catch (Exception e)
            {
                onError.Invoke($"{url}: {e.Message}");
                return default;
            }
        }
        
    }
}
