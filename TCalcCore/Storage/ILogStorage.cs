using System;
using System.Collections.Generic;
using System.Text;
using System.Threading.Tasks;

namespace TCalcCore.Storage
{
    public interface ILogStorage
    {
        Task StoreLog(RLogEntry entry);
        Task<IEnumerable<RLogEntry>> GetLogEntries(int hoursAgoFrom = int.MaxValue, int hoursAgoTo = 0);
    }
    public class RLogEntry
    {
        public string Msg { get; set; }
        public DateTimeOffset Timestamp { get; set; } = DateTimeOffset.Now;
        public string Ip { get; set; }
        public string UserAgent { get; set; }

        public RLogEntry(string msg, string ip, string userAgent)
        {
            Msg = msg;
            Ip = ip;
            UserAgent = userAgent;
        }
    }
}
