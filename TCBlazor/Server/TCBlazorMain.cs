using Microsoft.AspNetCore.ResponseCompression;
using Microsoft.IdentityModel.Tokens;
using TCalc.Storage;
using TourCalcWebApp;
using TourCalcWebApp.Auth;
using TourCalcWebApp.Controllers;
using TourCalcWebApp.Storage;

namespace Company.TCBlazor
{
    public class TCBlazorMain
    {
        public static void Main(string[] args)
        {
            var builder = WebApplication.CreateBuilder(args);

            // Add services to the container.

            //builder.Services.AddControllersWithViews();
            builder.Services.AddControllers();
            builder.Services.AddRazorPages();

            // services for tourcalc
            var Configuration = new TcConfiguration(builder.Configuration);
            builder.Services.AddSingleton<ITcConfiguration>(Configuration);
            builder.Services.AddSingleton<ITourStorage, TourCalcStorage>();
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


            // Configure the HTTP request pipeline.
            if (app.Environment.IsDevelopment())
            {
                app.UseWebAssemblyDebugging();
            }
            else
            {
                app.UseExceptionHandler("/Error");
                // The default HSTS value is 30 days. You may want to change this for production scenarios, see https://aka.ms/aspnetcore-hsts.
                app.UseHsts();
            }

            app.UseHttpsRedirection();

            app.UseBlazorFrameworkFiles();
            app.UseStaticFiles();

            app.UseRouting();

            app.UseAuthentication();
            app.UseAuthorization();


            app.MapRazorPages();
            app.MapControllers();
            app.MapFallbackToFile("index.html");

            app.Run();
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
            services.AddMvc(options => { options.EnableEndpointRouting = false; });

        }

    }
}