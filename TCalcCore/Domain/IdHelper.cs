using System;
using System.Collections.Generic;
using System.Text;

namespace TCalc.Domain
{
    public static class IdHelper
    {
        private static readonly Random rand = new Random();
        public static string NewId()
        {
            //return Guid.NewGuid().ToString();
            long ms = DateTimeOffset.Now.ToUnixTimeMilliseconds() * 100 + rand.Next(0, 99);
            int msInt = unchecked((int)ms);
//            long msr = reverse(ms);
            byte[] bytes = BitConverter.GetBytes(msInt);
            string result = Convert.ToBase64String(bytes).TrimEnd(new[] { '=' }).Replace('+', '*').Replace('/', '$');
                //.Substring(0, 9);
            return result;
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
