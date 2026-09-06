using Newtonsoft.Json.Linq;
using TCalc.Domain;
using TCBlazor.Server.Telegram;
using Xunit;

namespace TCalcTests
{
    public class TgMetaTests
    {
        [Fact]
        public void ChatBindingSurvivesARoundTrip()
        {
            var tour = new Tour();
            Assert.Null(TgMeta.ChatId(tour));
            Assert.False(TgMeta.IsActive(tour));

            TgMeta.BindChat(tour, -1001234567, active: true);
            Assert.Equal(-1001234567, TgMeta.ChatId(tour));
            Assert.True(TgMeta.IsActive(tour));

            TgMeta.SetActive(tour, false);
            Assert.False(TgMeta.IsActive(tour));
            Assert.Equal(-1001234567, TgMeta.ChatId(tour));  // still bound
        }

        [Fact]
        public void UserBindingSurvivesARoundTrip()
        {
            var person = new Person();
            Assert.False(TgMeta.IsTelegramPerson(person));

            TgMeta.BindUser(person, 777, "masha");
            Assert.Equal(777, TgMeta.UserId(person));
            Assert.Equal("masha", TgMeta.UserName(person));
            Assert.True(TgMeta.IsTelegramPerson(person));
        }

        [Fact]
        public void SomebodyElsesMetadataIsNotTrampled()
        {
            // Metadata is a shared free field: writing ours must merge, not overwrite
            var tour = new Tour { Metadata = "{\"other\":{\"keep\":1}}" };
            TgMeta.BindChat(tour, 5, true);

            var root = JObject.Parse(tour.Metadata);
            Assert.Equal(1, (int)root["other"]["keep"]);
            Assert.Equal(5, TgMeta.ChatId(tour));
        }

        [Fact]
        public void NonJsonMetadataIsKeptRatherThanLost()
        {
            var tour = new Tour { Metadata = "some legacy note" };
            TgMeta.BindChat(tour, 5, true);

            var root = JObject.Parse(tour.Metadata);
            Assert.Equal("some legacy note", (string)root["_raw"]);
            Assert.Equal(5, TgMeta.ChatId(tour));
        }
    }
}
