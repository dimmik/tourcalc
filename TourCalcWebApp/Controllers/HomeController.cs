using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;

namespace TourCalcWebApp.Controllers
{
    [Route("/{*anything}")]
    [Authorize]
    [ApiController]
    [AllowAnonymous]
    public class HomeController : ControllerBase
    {
        [HttpGet]
        public IActionResult Index()
        {
            return Content(IndexPage, "text/html");
        }
        const string IndexPage = @"<!DOCTYPE html>
<html>
<head>
    <meta name='viewport' content='width=device-width' />
    <title>Index</title>
</head>
<body>
    <div id='content'>
    </div>
    <script type='text/javascript' src='/assets/bundle.js'></script>
</body>
</html>";
    }
}