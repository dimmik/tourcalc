using System;
using Newtonsoft.Json.Linq;
using TCalc.Domain;

namespace TCBlazor.Server.Telegram
{
    /// <summary>
    /// Where the Telegram bindings live: a "tg" object inside the free-form
    /// <see cref="AbstractItem.Metadata"/> string that both Tour and Person inherit.
    ///
    /// Using that field is what lets the bot exist without a storage of its own and
    /// without touching the shared model - it is already persisted by every provider.
    /// The field belongs to nobody in particular, though, so everything here *merges*:
    /// it must never drop a key some later feature put beside ours. Content that is not
    /// a JSON object at all is kept verbatim under "_raw" rather than thrown away.
    /// </summary>
    public static class TgMeta
    {
        private const string Root = "tg";

        public static long? ChatId(Tour tour) => (long?)Read(tour?.Metadata)?["chat"];

        public static bool IsActive(Tour tour) => (bool?)Read(tour?.Metadata)?["active"] ?? false;

        public static long? UserId(Person person) => (long?)Read(person?.Metadata)?["user"];

        public static string UserName(Person person) => (string)Read(person?.Metadata)?["username"];

        /// <summary>A person the bot created for a Telegram account, as opposed to a dependent.</summary>
        public static bool IsTelegramPerson(Person person) => UserId(person).HasValue;

        public static void BindChat(Tour tour, long chatId, bool active)
        {
            var tg = Read(tour.Metadata) ?? new JObject();
            tg["chat"] = chatId;
            tg["active"] = active;
            tour.Metadata = Write(tour.Metadata, tg);
        }

        public static void SetActive(Tour tour, bool active)
        {
            var tg = Read(tour.Metadata) ?? new JObject();
            tg["active"] = active;
            tour.Metadata = Write(tour.Metadata, tg);
        }

        public static void BindUser(Person person, long userId, string userName)
        {
            var tg = Read(person.Metadata) ?? new JObject();
            tg["user"] = userId;
            if (!string.IsNullOrWhiteSpace(userName)) tg["username"] = userName;
            person.Metadata = Write(person.Metadata, tg);
        }

        private static JObject Read(string metadata)
        {
            var root = Parse(metadata);
            return root?[Root] as JObject;
        }

        private static JObject Parse(string metadata)
        {
            if (string.IsNullOrWhiteSpace(metadata)) return null;
            try
            {
                return JObject.Parse(metadata);
            }
            catch
            {
                return null;
            }
        }

        private static string Write(string metadata, JObject tg)
        {
            var root = Parse(metadata);
            if (root == null)
            {
                root = new JObject();
                // something was there that is not JSON - keep it rather than lose it
                if (!string.IsNullOrWhiteSpace(metadata)) root["_raw"] = metadata;
            }
            root[Root] = tg;
            return root.ToString(Newtonsoft.Json.Formatting.None);
        }
    }
}
