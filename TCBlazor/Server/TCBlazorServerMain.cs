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
using Company.TCBlazor.Storage;
using Microsoft.OpenApi.Models;
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

            SetupSwaggerDocs(builder.Services);

            // services for tourcalc
            builder.Services.AddSingleton<ITcConfiguration>(Configuration);
            // notifier
            builder.Services.AddSingleton<INotifier, WebPushNotifier>();
            // /notifier
            builder.Services.AddSingleton<ITourStorage, TourCalcStorage>();
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

            app.UseSwagger();
            app.UseSwaggerUI(c =>
            {
                c.SwaggerEndpoint("/swagger/v1/swagger.json", "Tourcalc API");
            });

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

        private static void SetupSwaggerDocs(IServiceCollection services)
        {
            // Register the Swagger generator, defining 1 or more Swagger documents
            services.AddSwaggerGen(c =>
            {
                c.SwaggerDoc("v1", new OpenApiInfo { Title = "Tourcalc API", Version = "v1" });
                // https://stackoverflow.com/questions/43447688/setting-up-swagger-asp-net-core-using-the-authorization-headers-bearer
                c.AddSecurityDefinition("Bearer",
                        new Microsoft.OpenApi.Models.OpenApiSecurityScheme
                        {
                            In = Microsoft.OpenApi.Models.ParameterLocation.Header,
                            Description = "Please enter into field the word 'Bearer' following by space and JWT",
                            Name = "Authorization",
                            Type = Microsoft.OpenApi.Models.SecuritySchemeType.ApiKey
                        });
                c.AddSecurityRequirement(new OpenApiSecurityRequirement() {
                    { new Microsoft.OpenApi.Models.OpenApiSecurityScheme
                        {
                            In = Microsoft.OpenApi.Models.ParameterLocation.Header,
                            Description = "Please enter into field the word 'Bearer' following by space and JWT",
                            Name = "Authorization",
                            Type = Microsoft.OpenApi.Models.SecuritySchemeType.ApiKey
                        }, new List<string>() } }

                    );
                // Set the comments path for the Swagger JSON and UI.
                var xmlFile = $"{System.Reflection.Assembly.GetExecutingAssembly().GetName().Name}.xml";
                var xmlPath = Path.Combine(AppContext.BaseDirectory, xmlFile);
                c.IncludeXmlComments(xmlPath);
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