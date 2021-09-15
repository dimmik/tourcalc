using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.IdentityModel.Tokens;
using System;
using System.IO;
using TCalc.Storage;
using TourCalcWebApp.Auth;
using TourCalcWebApp.Exceptions;
using System.Collections.Generic;
using Microsoft.AspNetCore.Cors.Infrastructure;
using TourCalcWebApp.Storage;
using TourCalcWebApp.TgBot;
using Microsoft.OpenApi.Models;
using TourCalcWebApp.Controllers;
using System.Net.Http;
using System.Threading.Tasks;

namespace TourCalcWebApp
{
    public class Startup
    {
        private Task WakeupServiceThread;

        public Startup(IConfiguration configuration)
        {
            Configuration = new TcConfiguration(configuration);
        }

        public ITcConfiguration Configuration { get; }

        // This method gets called by the runtime. Use this method to add services to the container.
        public void ConfigureServices(IServiceCollection services)
        {

            var tgToken = Configuration.GetValue<string>("TelegramBotToken");
            services.AddSingleton<IBotService>(new BotService(tgToken));

            services.AddSingleton<ITcConfiguration>(Configuration);
            //SetupLightDB(services);
            services.AddSingleton<ITourStorage, TourCalcStorage>();
            SetupSwaggerDocs(services);

            SetupReact(services);
            // CORS

            //Add Cors support to the service
            services.AddCors(
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

            services.AddMvc().SetCompatibilityVersion(CompatibilityVersion.Latest)
                .AddJsonOptions(options =>
                {
                    options.JsonSerializerOptions.WriteIndented = true;
                });

            SetupAuth(services);

            services.AddSingleton(new StartupInfo());

            WakeupThread();

        }
        private void WakeupThread()
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

        /*private void SetupLightDB(IServiceCollection services)
        {
            var rootFolder = Configuration.GetValue("DatabaseRootFolder", Path.DirectorySeparatorChar == '\\' ? @"d:\home\" : "/home/");
            var dbPath = $"{rootFolder}{Configuration.GetValue<string>("DatabaseRelativePath", $@"Tour.db")}";
            Directory.CreateDirectory(Path.GetDirectoryName(dbPath));
            bool createVersions = Configuration.GetValue("TourVersioning", true);
            bool isVersionEditable = Configuration.GetValue("TourVersionEditable", false);
            ITourStorage tourStorage = new LiteDbTourStorage(dbPath, createVersions, isVersionEditable);

            services.AddSingleton<ITourStorage>(tourStorage);
        }*/

        private static void SetupReact(IServiceCollection services)
        {
            services.AddSingleton<IHttpContextAccessor, HttpContextAccessor>();
        }

        private void SetupAuth(IServiceCollection services)
        {
            var privateKey = Configuration.GetValue<string>("AuthPrivateECDSAKey");
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
            services.AddMvc(options => { options.EnableEndpointRouting = false; });
        
        }
        // This method gets called by the runtime. Use this method to configure the HTTP request pipeline.
        public void Configure(IApplicationBuilder app, IHostingEnvironment env)
        {
            if (env.IsDevelopment())
            {
               // Microsoft.Extensions.Logging.Console.
                app.UseWebpackDevMiddleware();
            }
            if (env.IsDevelopment() || Configuration.GetValue<bool>("UseDeveloperExceptionPage", false))
            {
                app.UseDeveloperExceptionPage();
            }

            // Enable middleware to serve generated Swagger as a JSON endpoint.
            app.UseSwagger();

            // Enable middleware to serve swagger-ui (HTML, JS, CSS, etc.),
            // specifying the Swagger JSON endpoint.
            app.UseSwaggerUI(c =>
            {
                c.SwaggerEndpoint("/swagger/v1/swagger.json", "My API V1");
            });

            app.UseHttpException();

            app.UseHttpsRedirection();
            app.UseAuthentication();
            app.UseStaticFiles(new StaticFileOptions
            {
                ServeUnknownFileTypes = true
                
            });

            //Use the new policy globally
            app.UseCors("mypolicy");

            app.UseMvc(routes => {
                routes.MapRoute(
                    name: "default",
                    template: "/");
//                routes.MapSpaFallbackRoute("spa-fallback", new { });
            });

        }
    }

    public static class ApplicationBuilderExtensions
    {
        public static IApplicationBuilder UseHttpException(this IApplicationBuilder application)
        {
            return application.UseMiddleware<HttpExceptionMiddleware>();
        }
    }
}
