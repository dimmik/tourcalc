using System.Net.Http.Json;

namespace TCBlazor.Client.Utils
{
    public static class HttpExtensions
    {
        public async static Task<T?> GetFromJsonWithAuthToken<T>(this HttpClient http, string url, string token)
        {
            var request = new HttpRequestMessage(HttpMethod.Get, url);
            request.Headers.Authorization = new System.Net.Http.Headers.AuthenticationHeaderValue("bearer", token);
            var resp = await http.SendAsync(request);
            T? t = default;
            if (resp.IsSuccessStatusCode)
            {
                t = await resp.Content.ReadFromJsonAsync<T>();
            }
            return t;
        }
    }
}
