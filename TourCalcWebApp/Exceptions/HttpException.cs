using System;
using System.Collections.Generic;
using System.Linq;
using System.Net;
using System.Threading.Tasks;

namespace TourCalcWebApp.Exceptions
{
    public class HttpException : Exception
    {
        public static HttpException NotFound(string msg)
        {
            return new HttpException(404, msg);
        }

        public static HttpException Forbid(string msg)
        {
            return new HttpException(403, msg);
        }

        public static HttpException NotAuthenticated(string msg)
        {
            return new HttpException(401, msg);
        }

        public HttpException(int httpStatusCode)
        {
            this.StatusCode = httpStatusCode;
        }

        public HttpException(HttpStatusCode httpStatusCode)
        {
            this.StatusCode = (int)httpStatusCode;
        }

        private HttpException(int httpStatusCode, string message) : base(message)
        {
            this.StatusCode = httpStatusCode;
        }

        public HttpException(HttpStatusCode httpStatusCode, string message) : base(message)
        {
            this.StatusCode = (int)httpStatusCode;
        }

        public HttpException(int httpStatusCode, string message, Exception inner) : base(message, inner)
        {
            this.StatusCode = httpStatusCode;
        }

        public HttpException(HttpStatusCode httpStatusCode, string message, Exception inner) : base(message, inner)
        {
            this.StatusCode = (int)httpStatusCode;
        }

        public int StatusCode { get; }
    }
}
