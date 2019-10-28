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
        private readonly IConfiguration Configuration;

        public HomeController(IConfiguration config)
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
            var index = IndexPage;
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
            //index = index.Replace("__the_path__", Environment.CurrentDirectory);
            return Content(index, "text/html");
        }
        const string IndexPage = @"<!DOCTYPE html>
<html>
<head>
    <meta name='viewport' content='width=device-width' />

<link rel='apple-touch-icon' sizes='57x57' href='/apple-icon-57x57.png'>
<link rel='apple-touch-icon' sizes='60x60' href='/apple-icon-60x60.png'>
<link rel='apple-touch-icon' sizes='72x72' href='/apple-icon-72x72.png'>
<link rel='apple-touch-icon' sizes='76x76' href='/apple-icon-76x76.png'>
<link rel='apple-touch-icon' sizes='114x114' href='/apple-icon-114x114.png'>
<link rel='apple-touch-icon' sizes='120x120' href='/apple-icon-120x120.png'>
<link rel='apple-touch-icon' sizes='144x144' href='/apple-icon-144x144.png'>
<link rel='apple-touch-icon' sizes='152x152' href='/apple-icon-152x152.png'>
<link rel='apple-touch-icon' sizes='180x180' href='/apple-icon-180x180.png'>
<link rel='icon' type='image/png' sizes='192x192'  href='/android-icon-192x192.png'>
<link rel='icon' type='image/png' sizes='32x32' href='/favicon-32x32.png'>
<link rel='icon' type='image/png' sizes='96x96' href='/favicon-96x96.png'>
<link rel='icon' type='image/png' sizes='16x16' href='/favicon-16x16.png'>
<link rel='manifest' href='/manifest.json'>
<meta name='msapplication-TileColor' content='#ffffff'>
<meta name='msapplication-TileImage' content='/ms-icon-144x144.png'>
<meta name='theme-color' content='#ffffff'>
<!-- Version: _ver_ -->
    <title>Tourcalc (v _ver_)</title>
</head>
<body>
    <div id='content'>
    </div>
<br/>
<span style='font-size: xx-small'>v_ver_</span>
    <script type='text/javascript' src='/assets/bundle.js?unic=_md5_' 
        charset=""utf-8""></script>
</body>
</html>";
    }
}