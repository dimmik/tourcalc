﻿using AntDesign;
using Microsoft.AspNetCore.Components;
using System.Diagnostics;
using System.Net.Http.Json;
using System.Text;
using System.Text.Json;

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
#pragma warning disable CS4014 // (show message. will run somewhere there) Because this call is not awaited, execution of the current method continues before the call is completed
                _messageService.Error(getMessage(e.Message));
#pragma warning restore CS4014 // Because this call is not awaited, execution of the current method continues before the call is completed
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
            //_messageService.Info(getMessage("request to server"));
            Stopwatch sw = Stopwatch.StartNew();
            var request = new HttpRequestMessage(method, url);
            request.Headers.Authorization = new System.Net.Http.Headers.AuthenticationHeaderValue("bearer", token);
            if (body != null)
            {
                request.Content = new StringContent(JsonSerializer.Serialize(body), Encoding.UTF8, "application/json");
            }
            var resp = await Http.SendAsync(request);
            T? t = default;
            if (resp.IsSuccessStatusCode)
            {
                try
                {
                    t = await resp.Content.ReadFromJsonAsync<T>();
                }
                catch
                {
                    // no luck
                }
            }
            else
            {
                var m = await resp.Content.ReadAsStringAsync();
#pragma warning disable CS4014 // (show message. will run somewhere there) Because this call is not awaited, execution of the current method continues before the call is completed
                _messageService.Error(getMessage($"{(int)resp.StatusCode} {resp.StatusCode}: {m}"));
#pragma warning restore CS4014 // Because this call is not awaited, execution of the current method continues before the call is completed
            }
            //_messageService.Destroy();
            sw.Stop();
            Console.WriteLine($"{method} to {url} finished in {sw.Elapsed}");
            return t;
        }
    }
}
