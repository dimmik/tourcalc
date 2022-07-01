﻿using System;
using System.Diagnostics;
using System.Threading.Tasks;
using TCalcCore.Logging;
using TCalcCore.UI;

namespace TCalcCore.Storage
{
    public interface ITourcalcLocalStorage
    {
        Task<(string val, DateTimeOffset stored)> Get(string key, string defVal = "", bool storeDefaultValue = false);
        Task Set(string key, string val);
    }
    public static class ITourcalcLocalStorageExt
    {
        private static string GetTokenKey() => "__tc_token";
        private static string GetUISettingsKey() => "__tc_ui_settings";

        public static async Task<(T val, DateTimeOffset stored)> GetObject<T>(
            this ITourcalcLocalStorage ts,
            string key,
            Func<T> defValFactory,
            bool storeDefaultValue = false,
            ILocalLogger logger = null
            )
        {
            Stopwatch sw = Stopwatch.StartNew();
            var (json, dt) = await ts.Get(key);
            //logger?.Log($"j: {json}");
            try
            {
                T res = Newtonsoft.Json.JsonConvert.DeserializeObject<T>(json ?? "");
                sw.Stop();
                logger?.Log($"Get object {key} (null? {res == null}) in {sw.Elapsed}");
                return (res, dt);
            }
            catch (Exception e)
            {
                logger?.Log($"Error get obj: {e.Message}");
                var val = defValFactory();
                if (storeDefaultValue)
                {
                    await ts.SetObject(key, val);
                }
                return (val, dt);
            }
        }
        public static async Task<(string val, DateTimeOffset stored)> GetToken(this ITourcalcLocalStorage ts)
        {
            return await ts.Get(GetTokenKey());
        }
        public static async Task SetToken(this ITourcalcLocalStorage ts, string token)
        {
            await ts.Set(GetTokenKey(), token);
        }
        public static async Task<(UISettings val, DateTimeOffset stored)> GetUISettings(this ITourcalcLocalStorage ts)
        {
            return await ts.GetObject(GetUISettingsKey(), () => (new UISettings()), storeDefaultValue: true);
        }
        public static async Task SetObject<T>(this ITourcalcLocalStorage ts, string key, T obj, ILocalLogger logger = null)
        {
            Stopwatch sw = Stopwatch.StartNew();
            string json = Newtonsoft.Json.JsonConvert.SerializeObject(obj);
            await ts.Set(key, json);
            sw.Stop();
            logger?.Log($"Set object {key} in {sw.Elapsed}");
        }
        public static async Task SetUISettings(this ITourcalcLocalStorage ts, UISettings s)
        {
            await ts.SetObject(GetUISettingsKey(), s);
        }
    }
}