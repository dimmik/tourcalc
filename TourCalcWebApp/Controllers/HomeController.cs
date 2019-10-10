using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Configuration;
using TourCalcWebApp.Auth;

namespace TourCalcWebApp.Controllers
{
    [Route("/{*anything}")]
    [Authorize]
    [ApiController]
    [AllowAnonymous]
    public class HomeController : ControllerBase
    {
        private readonly IConfiguration Configuration;

        public HomeController(IConfiguration config)
        {
            Configuration = config;
        }

        [HttpGet]
        public IActionResult Index()
        {
            var index = IndexPage;
            try
            {
                var pathToBundleJs = Configuration.GetValue("PathToBundleJs", @"wwwroot\assets\bundle.js");
                var fileInfo = new FileInfo(pathToBundleJs);
                using (var contentStream = System.IO.File.OpenRead(pathToBundleJs))
                {
                    var md5 = AuthHelper.CreateMD5(contentStream);
                    index = IndexPage.Replace("_md5_", $"{md5}");
                }
            }
            catch (Exception e)
            {
                //index = index.Replace("__error__", e.Message);
            }
            //index = index.Replace("__the_path__", Environment.CurrentDirectory);
            return Content(index, "text/html");
        }
        const string IndexPage = @"<!DOCTYPE html>
<html>
<head>
    <meta name='viewport' content='width=device-width' />
    <title>Tourcalc</title>
</head>
<body>
    <div id='content'>
    </div>
    <script type='text/javascript' src='/assets/bundle.js?unic=_md5_'></script>
</body>
</html>";
    }
}