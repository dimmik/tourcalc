using Microsoft.AspNetCore.HttpOverrides;
using Microsoft.IdentityModel.Tokens;
using Microsoft.Net.Http.Headers;
using TCalc.Storage;
using TCalcCore.Storage;
using TCalcCore.UI;
using TCalcStorage.Storage;
using TCalcStorage.Storage.MongoDB;
using TCBlazor.Server;
using TourCalcWebApp;
using TourCalcWebApp.Auth;
using TourCalcWebApp.Controllers;
using TourCalcWebApp.Storage;

namespace Company.TCBlazor
{
    public class TCBlazorServerMain
    {
        public static void Main(string[] args)
        {
            var builder = WebApplication.CreateBuilder(args);

            // so that HttpContext.Connection.RemoteIpAddress returns real user ip address, not address of local proxy (nginx for example)
            builder.Services.Configure<ForwardedHeadersOptions>(options =>
            {
                options.ForwardedHeaders =
                    ForwardedHeaders.XForwardedFor | ForwardedHeaders.XForwardedProto;
            });


            // Add services to the container.
            var assembly = typeof(TourController).Assembly;
            builder.Services.AddControllers()
                .AddApplicationPart(assembly)
                .AddControllersAsServices();

            builder.Services.AddControllers()
                .AddJsonOptions(options =>
                {
                    options.JsonSerializerOptions.WriteIndented = true;
                });
            builder.Services.AddRazorPages();

            builder.Services.AddSwaggerGen();

            // services for tourcalc
            var Configuration = new TcConfiguration(builder.Configuration);
            builder.Services.AddSingleton<ITcConfiguration>(Configuration);
            // notifier
            builder.Services.AddSingleton<INotifier, WebPushNotifier>();
            // /notifier
            builder.Services.AddSingleton<ITourStorage, TourCalcStorage>();
            var providerType = Configuration.GetValue("StorageType", "InMemory");
            if (providerType.ToLower() == "InMemory".ToLower())
            {
                builder.Services.AddSingleton<ILogStorage, InMemoryLogStorage>();
                builder.Services.AddSingleton<ISubscriptionStorage, InMemorySubscriptionStorage>();
            }
            else if (providerType.ToLower() == "MongoDb".ToLower())
            {
                var url = Configuration.GetValue<string>("MongoDbUrl");
                var username = Configuration.GetValue<string>("MongoDbUsername");
                var password = Configuration.GetValue<string>("MongoDbPassword");
                var provider = new MongoDbLogStorage(url, username, password);
                builder.Services.AddSingleton<ILogStorage>(provider);
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

            app.UseSwagger();
            app.UseSwaggerUI(c =>
            {
                c.SwaggerEndpoint("/swagger/v1/swagger.json", "Tourcalc API");
            });

            app.UseHttpsRedirection();

            app.UseHttpException();

            app.UseBlazorFrameworkFiles();
            app.UseStaticFiles();

            app.UseRouting();

            app.UseAuthentication();
            app.UseAuthorization();

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

        private static void SetupAuth(IServiceCollection services, ITcConfiguration configuration)
        {
            var privateKey = configuration.GetValue<string>("AuthPrivateECDSAKey");
            var sk = new ECDSAKey(privateKey);
            services.AddSingleton<IECDsaCryptoKey>(sk);

            const string jwtSchemeName = "JwtBearer";
            services
                .AddAuthentication(options =>
                {
                    options.DefaultAuthenticateScheme = jwtSchemeName;
                    options.DefaultChallengeScheme = jwtSchemeName;
                })
                .AddJwtBearer(jwtSchemeName, jwtBearerOptions =>
                {
                    jwtBearerOptions.TokenValidationParameters = new TokenValidationParameters
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
                });
        }

    }
}