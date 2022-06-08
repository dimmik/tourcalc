using System.Net.Http.Json;
using System.Text;
using System.Text.Json;

namespace TCBlazor.Client.Utils
{
    public static class HttpExtensions
    {
        public async static Task<T?> CallWithAuthToken<T>(this HttpClient http, string url, string token)
        {
            return await CallWithAuthToken<T>(http, url, token, HttpMethod.Get, null);
        }

        public async static Task<T?> CallWithAuthToken<T>(this HttpClient http, string url, string token, HttpMethod method, object? body)
        {
            var request = new HttpRequestMessage(method, url);
            request.Headers.Authorization = new System.Net.Http.Headers.AuthenticationHeaderValue("bearer", token);
            if (body != null)
            {
                request.Content = new StringContent(JsonSerializer.Serialize(body), Encoding.UTF8, "application/json");
            }
            var resp = await http.SendAsync(request);
            T? t = default;
            if (resp.IsSuccessStatusCode)
            {
                try
                {
                    t = await resp.Content.ReadFromJsonAsync<T>();
                } catch
                {
                    // no luck
                }
            }
            return t;
        }
    }
}
