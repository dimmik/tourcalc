using System;
using TCBlazor.Server.Telegram;
using Xunit;

namespace TCalcTests
{
    /// <summary>
    /// The signature check, against a vector produced by an independent implementation
    /// (a few lines of Python using hmac/hashlib) rather than by this code - otherwise the
    /// test would only prove the code agrees with itself.
    /// </summary>
    public class TgWebAppAuthTests
    {
        private const string Token = "123456:TEST-TOKEN-not-a-real-one";

        private const string InitData =
            "auth_date=1757000000&query_id=AAF_test&user=%7B%22id%22%3A777%2C%22first_name%22%3A%22" +
            "%D0%94%D0%B8%D0%BC%D0%B0%22%2C%22username%22%3A%22dmk%22%7D" +
            "&hash=d3a604730595565c865048425cf09dfc1d45504b5201582ebfb607eda4e2fcc5";

        private static readonly DateTimeOffset Signed = DateTimeOffset.FromUnixTimeSeconds(1757000000);

        private static TgWebAppAuth.Verified Check(string data, string token = Token, TimeSpan? age = null, DateTimeOffset? now = null)
            => TgWebAppAuth.Validate(data, token, age ?? TimeSpan.FromHours(24), now ?? Signed.AddMinutes(1));

        [Fact]
        public void AGenuinePayloadIsAcceptedAndSaysWhoItIs()
        {
            var verified = Check(InitData);
            Assert.NotNull(verified);
            Assert.Equal(777, verified.UserId);
            Assert.Equal("dmk", verified.UserName);
            Assert.Equal(Signed, verified.AuthDate);
        }

        [Fact]
        public void AnotherBotsTokenDoesNotValidateIt()
            => Assert.Null(Check(InitData, token: "123456:SOME-OTHER-TOKEN"));

        [Fact]
        public void TamperingWithTheUserIsRejected()
        {
            // the whole point: without the signature anyone could claim to be anyone
            var forged = InitData.Replace("%22id%22%3A777", "%22id%22%3A999");
            Assert.Null(Check(forged));
        }

        [Fact]
        public void TamperingWithTheHashIsRejected()
            => Assert.Null(Check(InitData.Replace("d3a60473", "00000000")));

        [Fact]
        public void APayloadWithNoHashIsRejected()
            => Assert.Null(Check("auth_date=1757000000&user=%7B%22id%22%3A777%7D"));

        [Fact]
        public void AnOldPayloadIsRejectedSoOneCannotBeReplayedLater()
            => Assert.Null(Check(InitData, age: TimeSpan.FromHours(1), now: Signed.AddHours(2)));

        [Fact]
        public void NothingAtAllIsRejected()
        {
            Assert.Null(Check(""));
            Assert.Null(Check(null));
            Assert.Null(Check(InitData, token: ""));
        }
    }
}
