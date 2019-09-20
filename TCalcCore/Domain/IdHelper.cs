using System;
using System.Collections.Generic;
using System.Text;

namespace TCalc.Domain
{
    public static class IdHelper
    {
        public static string NewId()
        {
            //return Guid.NewGuid().ToString();
            long ms = DateTimeOffset.Now.ToUnixTimeMilliseconds();
            long msr = reverse(ms);
            byte[] bytes = BitConverter.GetBytes(msr);
            string b64 = Convert.ToBase64String(bytes).TrimEnd(new[] { '=' }).Replace('+', '*').Replace('/', '$')
                .Substring(0, 9);
            return b64;
        }
        private static long reverse(long n)
        {
            long reverse = 0, rem;
            while (n != 0)
            {
                rem = n % 10;
                reverse = reverse * 10 + rem;
                n /= 10;
            }
            return reverse;
        }
    }
}
