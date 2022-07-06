using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace TCalcCore.Storage
{
    public interface ILogStorage
    {
        Task StoreLog(RLogEntry entry);
        Task<IEnumerable<RLogEntry>> GetLogEntries(int hoursAgoFrom = int.MaxValue, int hoursAgoTo = 0);
    }
    public class VoidLogStorage : ILogStorage
    {
        public Task<IEnumerable<RLogEntry>> GetLogEntries(int hoursAgoFrom = int.MaxValue, int hoursAgoTo = 0)
        {
            return Task.FromResult(Enumerable.Empty<RLogEntry>());
        }

        public Task StoreLog(RLogEntry entry)
        {
            //nothing
            return Task.CompletedTask;
        }
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
