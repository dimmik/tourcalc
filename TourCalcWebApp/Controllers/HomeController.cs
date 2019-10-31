using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Threading.Tasks;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Configuration;
using TourCalcWebApp.Auth;

namespace TourCalcWebApp.Controllers
{
    [Route("/{*pathToStaticContent}")]
    [Authorize]
    [ApiController]
    [AllowAnonymous]
    public class HomeController : ControllerBase
    {
        private readonly ITcConfiguration Configuration;

        public HomeController(ITcConfiguration config)
        {
            Configuration = config;
        }
        /// <summary>
        /// Index page to host SPA
        /// </summary>
        /// <returns>html content</returns>
        [HttpGet]
        public IActionResult Index()
        {
            var index = System.IO.File.ReadAllText(Configuration.GetValue("PathToIndexTpl", @"IndexTpl.html"));//IndexPage;
            try
            {
                var pathToBundleJs = Configuration.GetValue("PathToBundleJs", @"wwwroot\assets\bundle.js");
                var fileInfo = new FileInfo(pathToBundleJs);
                using (var contentStream = System.IO.File.OpenRead(pathToBundleJs))
                {
                    var md5 = AuthHelper.CreateMD5(contentStream);
                    index = index.Replace("_md5_", $"{md5}");
                }
                var ver = Assembly.GetEntryAssembly().GetCustomAttribute<AssemblyInformationalVersionAttribute>().InformationalVersion;
                index = index.Replace("_ver_", $"{ver}");
            }
            catch (Exception /*e*/)
            {
                //index = index.Replace("__error__", e.Message);
            }
            index = index.Replace("_now_", DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss"));
            //index = index.Replace("__the_path__", Environment.CurrentDirectory);
            return Content(index, "text/html");
        }
    }
}