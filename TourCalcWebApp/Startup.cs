using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.HttpsPolicy;
using Microsoft.AspNetCore.Mvc;
using Microsoft.AspNetCore.SpaServices.ReactDevelopmentServer;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.IdentityModel.Tokens;
using Newtonsoft.Json;
using System;
using System.IO;
using TCalc.Storage;
using TourCalcWebApp.Auth;

using Microsoft.OpenApi.Models;
using Swashbuckle.AspNetCore.Swagger;
using TourCalcWebApp.Exceptions;

namespace TourCalcWebApp
{
    public class Startup
    {
        public Startup(IConfiguration configuration)
        {
            Configuration = configuration;
        }

        public IConfiguration Configuration { get; }

        // This method gets called by the runtime. Use this method to add services to the container.
        public void ConfigureServices(IServiceCollection services)
        {
            var rootFolder = Configuration.GetValue("DatabaseRootFolder", Path.DirectorySeparatorChar == '\\' ? @"d:\home\" : "/home/");
            var dbPath = $"{rootFolder}{Configuration.GetValue<string>("DatabaseRelativePath", $@"Tour.db")}";
            Directory.CreateDirectory(Path.GetDirectoryName(dbPath));
            ITourStorage tourStorage = new LiteDbTourStorage(dbPath);
            services.AddSingleton<ITourStorage>(tourStorage);

            // Register the Swagger generator, defining 1 or more Swagger documents
            services.AddSwaggerGen(c =>
            {
                c.SwaggerDoc("v1", new Info { Title = "Tourcalc API", Version = "v1" });
                // Set the comments path for the Swagger JSON and UI.
                var xmlFile = $"{System.Reflection.Assembly.GetExecutingAssembly().GetName().Name}.xml";
                var xmlPath = Path.Combine(AppContext.BaseDirectory, xmlFile);
                c.IncludeXmlComments(xmlPath);
            });


            SetupReact(services);

            services.AddMvc().SetCompatibilityVersion(CompatibilityVersion.Version_2_2)
                .AddJsonOptions(options =>
                {
                    options.SerializerSettings.Formatting = Formatting.Indented;
                });

            SetupAuth(services);


        }

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
        }
        // This method gets called by the runtime. Use this method to configure the HTTP request pipeline.
        public void Configure(IApplicationBuilder app, IHostingEnvironment env)
        {
            if (env.IsDevelopment())
            {
                app.UseDeveloperExceptionPage();
                app.UseWebpackDevMiddleware();
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
            app.UseStaticFiles();

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
