using Microsoft.AspNetCore.Authentication.JwtBearer;
using Microsoft.AspNetCore.HttpOverrides;
using Microsoft.IdentityModel.Tokens;
using Microsoft.Net.Http.Headers;
using TCalc.Storage;
using TCalcCore.Storage;
using TCalcCore.UI;
using TCalcStorage.Storage;
using TCalcStorage.Storage.MongoDB;
using TCBlazor.Server;
using Company.TCBlazor.Auth;
using TCBlazor.Server.Telegram;
using Telegram.Bot;
using Company.TCBlazor.Storage;
using Company.TCBlazor.Controllers;

namespace Company.TCBlazor
{
    public class TCBlazorServerMain
    {
        private static Task WakeupServiceThread;

        public static void Main(string[] args)
        {
            var builder = WebApplication.CreateBuilder(args);

            var Configuration = new TcConfiguration(builder.Configuration);

            // so that HttpContext.Connection.RemoteIpAddress returns real user ip address, not address of local proxy (nginx for example)
            builder.Services.Configure<ForwardedHeadersOptions>(options =>
            {
                options.ForwardedHeaders =
                    ForwardedHeaders.XForwardedFor | ForwardedHeaders.XForwardedProto;
            });


            // Add services to the container.
            builder.Services.AddControllers()
                .AddJsonOptions(options =>
                {
                    options.JsonSerializerOptions.WriteIndented = true;
                });
            builder.Services.AddRazorPages();

            // services for tourcalc
            builder.Services.AddSingleton<ITcConfiguration>(Configuration);
            // notifier
            builder.Services.AddSingleton<INotifier, WebPushNotifier>();
            // /notifier
            builder.Services.AddSingleton<ITourStorage, TourCalcStorage>();

            // Telegram bot. Nothing is registered without a token and a mode, so a build
            // with neither behaves exactly as it did before the bot existed.
            SetupTelegram(builder.Services, Configuration);
            var providerType = Configuration.GetValue("StorageType", "InMemory");
            if (providerType.ToLower() == "InMemory".ToLower())
            {
                //builder.Services.AddSingleton<ILogStorage, InMemoryLogStorage>();
                builder.Services.AddSingleton<ILogStorage, VoidLogStorage>();
                builder.Services.AddSingleton<ISubscriptionStorage, InMemorySubscriptionStorage>();
            }
            else if (providerType.ToLower() == "MongoDb".ToLower())
            {
                var url = Configuration.GetValue<string>("MongoDbUrl");
                var username = Configuration.GetValue<string>("MongoDbUsername");
                var password = Configuration.GetValue<string>("MongoDbPassword");
                var provider = new MongoDbLogStorage(url, username, password);
                //builder.Services.AddSingleton<ILogStorage>(provider);
                builder.Services.AddSingleton<ILogStorage, VoidLogStorage>(); // never needed the logs
                // subs storage
                var subStorage = new MongoDbSubscriptionStorage(url, username, password);
                builder.Services.AddSingleton<ISubscriptionStorage>(subStorage);
            }
            else
            {
                // for now - just dumb
                builder.Services.AddSingleton<ILogStorage, VoidLogStorage>();
            }
            SetupAuth(builder.Services, Configuration);

            builder.Services.AddSingleton(new StartupInfo());
            builder.Services.AddSingleton<IHttpContextAccessor, HttpContextAccessor>();


            builder.Services.AddCors(
                options => {
                    options.AddPolicy("mypolicy",
                        builder => builder
                        .AllowAnyMethod()
                        .AllowAnyHeader()
                        .AllowCredentials()
                        .SetIsOriginAllowed(hostName => true));
                    options.AddDefaultPolicy(builder => builder
                        .AllowAnyMethod()
                        .AllowAnyHeader()
                        .AllowCredentials()
                        .SetIsOriginAllowed(hostName => true));
                }
            );

            WakeupThread(Configuration);

            var app = builder.Build();
            // so that HttpContext.Connection.RemoteIpAddress returns real user ip address, not address of local proxy (nginx for example)
            app.UseForwardedHeaders();
            app.Use(
                (ctx, next) =>
                {
                    ctx.Response.OnStarting(
                        () =>
                        {
                            ctx.Response.Headers[HeaderNames.CacheControl] = "no-cache";
                            ctx.Response.Headers["X-Tourcalc-Version"] = "#{BuildType}# v #{Build.BuildNumber}#";
                            return Task.CompletedTask;
                        }
                        );
                    return next(ctx);
                }

                );


            // Configure the HTTP request pipeline.
            if (app.Environment.IsDevelopment())
            {
                app.UseWebAssemblyDebugging();
                app.MapGet("/debug/routes", (IEnumerable<EndpointDataSource> endpointSources) =>
                    {
                        return string.Join("\n", endpointSources.SelectMany(source => source.Endpoints));
                    }
                );

            }
            else
            {
                app.UseExceptionHandler("/Error");
                // The default HSTS value is 30 days. You may want to change this for production scenarios, see https://aka.ms/aspnetcore-hsts.
                app.UseHsts();
            }

            app.UseHttpsRedirection();

            app.UseHttpException();

            app.UseBlazorFrameworkFiles();
            app.UseStaticFiles(new StaticFileOptions
            {
                ServeUnknownFileTypes = true
            });

            app.UseRouting();

            app.UseCors("mypolicy");

            app.UseAuthentication();
            // the text pages' cookie, read before the endpoint filters so antiforgery
            // sees the same reader the form was rendered for
            app.UseTextAuth();
            app.UseAuthorization();

            // after the static files, so assets are already served and never reach it,
            // and before the endpoints, so it can answer instead of the SPA shell
            app.UseTextBrowserRedirect();

            app.MapRazorPages();
            app.MapControllers();
            app.Map("api/{**any}", HandleApiFallback);
            app.MapFallbackToFile("{**any}", "index.html", new StaticFileOptions
            {
                OnPrepareResponse = ctx =>
                {
                        ctx.Context.Response.Headers[HeaderNames.CacheControl] =
                            "no-cache";
                }
            });




            app.Run();
        }
        private static Task HandleApiFallback(HttpContext context)
        {
            context.Response.StatusCode = StatusCodes.Status404NotFound;
            return Task.CompletedTask;
        }

        private static void SetupTelegram(IServiceCollection services, ITcConfiguration configuration)
        {
            var options = TelegramBotOptions.Read(configuration);
            services.AddSingleton(options);
            // one line at startup saying whether the bot is on, and precisely why not when
            // it is off: a silent no-op was impossible to tell apart from a broken token
            Console.WriteLine(options.Enabled
                ? $"Telegram bot: on, mode={options.Mode}"
                : $"Telegram bot: off - {options.DisabledBecause}");
            if (!options.Enabled) return;

            services.AddSingleton<ITelegramBotClient>(_ => new TelegramBotClient(options.Token));
            services.AddSingleton<ITourStorageProcessor, TourStorageProcessor>();
            services.AddSingleton<TourcalcBot>(sp => new TourcalcBot(
                sp.GetRequiredService<ITourStorage>(),
                sp.GetRequiredService<ITourStorageProcessor>(),
                options));
            services.AddSingleton<TgDispatcher>();

            // the menu goes up whichever way updates arrive
            services.AddHostedService<TelegramCommandsRegistrar>();

            if (options.IsPolling)
            {
                services.AddHostedService<TelegramPollingService>();
            }
            else if (options.IsWebhook)
            {
                services.AddHostedService<TelegramWebhookRegistrar>();
            }
        }

        private static void SetupAuth(IServiceCollection services, ITcConfiguration configuration)
        {
            var privateKey = configuration.GetValue<string>("AuthPrivateECDSAKey");
            var sk = new ECDSAKey(privateKey);
            services.AddSingleton<IECDsaCryptoKey>(sk);

            const string jwtSchemeName = "JwtBearer";

            TokenValidationParameters Validation() => new TokenValidationParameters
            {
                ValidateIssuerSigningKey = true,
                IssuerSigningKey = sk.GetPublicKey(),

                ValidateIssuer = true,
                ValidIssuer = "TourCalc",

                ValidateAudience = true,
                ValidAudience = "Users",

                ValidateLifetime = true,

                ClockSkew = TimeSpan.FromSeconds(5)
            };

            services
                .AddAuthentication(options =>
                {
                    options.DefaultAuthenticateScheme = jwtSchemeName;
                    options.DefaultChallengeScheme = jwtSchemeName;
                })
                .AddJwtBearer(jwtSchemeName, jwtBearerOptions =>
                {
                    jwtBearerOptions.TokenValidationParameters = Validation();
                })
                // The text pages carry the very same token, only in a cookie: a browser
                // without JavaScript cannot set an Authorization header. It is a scheme of
                // its own rather than an extra source for the one above, because a cookie
                // travels on every request by itself - accepting it for the JSON API would
                // hand any other site the ability to call that API as the reader. Nothing
                // asks for this scheme except the pages under /t.
                .AddJwtBearer(TextAuth.Scheme, jwtBearerOptions =>
                {
                    jwtBearerOptions.TokenValidationParameters = Validation();
                    jwtBearerOptions.Events = new JwtBearerEvents
                    {
                        OnMessageReceived = ctx =>
                        {
                            ctx.Token = ctx.Request.Cookies[TextAuth.CookieName];
                            return Task.CompletedTask;
                        }
                    };
                });
        }

        private static void WakeupThread(ITcConfiguration Configuration)
        {
            try
            {
                // wakeup thread
                if (Configuration.GetValue("DoWakeup", false))
                {
                    var url = Configuration.GetValue<string>("WakeupUrl");
                    HttpClient client = new()
                    {
                        Timeout = TimeSpan.FromMinutes(10)
                    };
                    WakeupServiceThread = client.GetAsync(url);
                    Console.WriteLine($"thread status: {WakeupServiceThread.Status}");
                }
            }
            catch
            {
                // well ...
            }
        }
    }
}