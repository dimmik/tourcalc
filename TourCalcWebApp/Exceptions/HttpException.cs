using System;
using System.Net;

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
            StatusCode = httpStatusCode;
        }

        public HttpException(HttpStatusCode httpStatusCode)
        {
            StatusCode = (int)httpStatusCode;
        }

        public HttpException(int httpStatusCode, string message) : base(message)
        {
            StatusCode = httpStatusCode;
        }

        public HttpException(HttpStatusCode httpStatusCode, string message) : base(message)
        {
            StatusCode = (int)httpStatusCode;
        }

        public HttpException(int httpStatusCode, string message, Exception inner) : base(message, inner)
        {
            StatusCode = httpStatusCode;
        }

        public HttpException(HttpStatusCode httpStatusCode, string message, Exception inner) : base(message, inner)
        {
            StatusCode = (int)httpStatusCode;
        }

        public int StatusCode { get; }
    }
}
