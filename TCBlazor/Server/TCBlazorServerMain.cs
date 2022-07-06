using Microsoft.AspNetCore.HttpOverrides;
using Microsoft.AspNetCore.Mvc.ApplicationParts;
using Microsoft.AspNetCore.ResponseCompression;
using Microsoft.IdentityModel.Tokens;
using Microsoft.Net.Http.Headers;
using TCalc.Storage;
using TCalcCore.Storage;
using TCalcStorage.Storage;
using TCalcStorage.Storage.MongoDB;
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

            // Add services to the container.
            var assembly = typeof(TourController).Assembly;
            builder.Services.AddControllers()
                .AddApplicationPart(assembly)
                .AddControllersAsServices();
            //.PartManager.ApplicationParts.Add(new AssemblyPart(assembly));
            //builder.Services.AddControllersWithViews();

            builder.Services.AddControllers();
            builder.Services.AddRazorPages();

            // services for tourcalc
            var Configuration = new TcConfiguration(builder.Configuration);
            builder.Services.AddSingleton<ITcConfiguration>(Configuration);
            builder.Services.AddSingleton<ITourStorage, TourCalcStorage>();
            var providerType = Configuration.GetValue("StorageType", "InMemory");
            if (providerType.ToLower() == "InMemory".ToLower())
            {
                builder.Services.AddSingleton<ILogStorage, InMemoryLogStorage>();
            } 
            else if (providerType.ToLower() == "MongoDb".ToLower())
            {
                var url = Configuration.GetValue<string>("MongoDbUrl");
                var username = Configuration.GetValue<string>("MongoDbUsername");
                var password = Configuration.GetValue<string>("MongoDbPassword");
                var provider = new MongoDbLogStorage(url, username, password);
                builder.Services.AddSingleton<ILogStorage>(provider);
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


            // so that HttpContext.Connection.RemoteIpAddress returns real user ip address, not address of local proxy (nginx for example)
            builder.Services.Configure<ForwardedHeadersOptions>(options =>
            {
                options.ForwardedHeaders =
                    ForwardedHeaders.XForwardedFor | ForwardedHeaders.XForwardedProto;
            });

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
            // MvcOptions.EnableEndpointRouting = false;
            //services.AddMvc(options => { options.EnableEndpointRouting = false; });

        }

    }
}