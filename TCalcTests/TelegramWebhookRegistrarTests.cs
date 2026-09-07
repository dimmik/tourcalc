using TCBlazor.Server.Telegram;
using Xunit;

namespace TCalcTests
{
    public class TelegramWebhookRegistrarTests
    {
        private static TelegramBotOptions Ok() => new TelegramBotOptions
        {
            Token = "123:abc",
            Mode = "webhook",
            WebhookSecret = "a-good-secret_123",
            PublicBaseUrl = "https://tourcalc.example.org",
        };

        [Fact]
        public void TheWebhookGoesToTheControllersRoute()
            => Assert.Equal("https://tourcalc.example.org/api/tg/update", Ok().WebhookUrl);

        [Fact]
        public void ATrailingSlashDoesNotDoubleUp()
        {
            var options = Ok();
            options.PublicBaseUrl = "https://tourcalc.example.org/";
            Assert.Equal("https://tourcalc.example.org/api/tg/update", options.WebhookUrl);
        }

        [Fact]
        public void AGoodConfigurationHasNoComplaints()
            => Assert.Null(TelegramWebhookRegistrar.Problem(Ok()));

        [Fact]
        public void PlainHttpIsRefusedBecauseTelegramOnlyDeliversOverHttps()
        {
            var options = Ok();
            options.PublicBaseUrl = "http://localhost:5399";
            var problem = TelegramWebhookRegistrar.Problem(options);

            Assert.Contains("https", problem);
            Assert.Contains("polling", problem);   // and says what to use instead
        }

        [Fact]
        public void AnEmptySecretIsRefusedBecauseTheEndpointWouldRejectEveryDelivery()
        {
            var options = Ok();
            options.WebhookSecret = "";
            Assert.Contains("WebhookSecret", TelegramWebhookRegistrar.Problem(options));
        }

        [Theory]
        [InlineData("has spaces")]
        [InlineData("привет")]
        [InlineData("with:colon")]
        public void ASecretTelegramWouldNotAcceptIsRefused(string secret)
        {
            var options = Ok();
            options.WebhookSecret = secret;
            Assert.Contains("A-Z", TelegramWebhookRegistrar.Problem(options));
        }

        [Fact]
        public void ANonsenseBaseAddressIsRefused()
        {
            var options = Ok();
            options.PublicBaseUrl = "не адрес";
            Assert.Contains("valid address", TelegramWebhookRegistrar.Problem(options));
        }
    }
}
