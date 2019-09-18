using JavaScriptEngineSwitcher.ChakraCore;
using JavaScriptEngineSwitcher.Extensions.MsDependencyInjection;
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
using React.AspNet;
using System;
using TCalc.Storage;
using TourCalcWebApp.Auth;

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
            ITourStorage tourStorage = new LiteDbTourStorage(Configuration.GetValue<string>("DatabasePath", "AppData/Tour.db"));
            services.AddSingleton<ITourStorage>(tourStorage);

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
            services.AddReact();
            services.AddJsEngineSwitcher(options => options.DefaultEngineName = ChakraCoreJsEngine.EngineName)
                .AddChakraCore();
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
            }
            
            app.UseHttpsRedirection();
            app.UseAuthentication();
            // react
            app.UseReact(config => { });
            app.UseDefaultFiles();
            app.UseStaticFiles();

            app.UseMvc();

        }
    }
}
