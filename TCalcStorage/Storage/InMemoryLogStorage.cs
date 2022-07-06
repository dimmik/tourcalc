using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using TCalcCore.Storage;

namespace TCalcStorage.Storage
{
    public class InMemoryLogStorage : ILogStorage
    {

        private List<RLogEntry> _logs = new();

        public Task<IEnumerable<RLogEntry>> GetLogEntries(int hoursAgoFrom = int.MaxValue, int hoursAgoTo = 0)
        {
            var now = DateTimeOffset.Now;
            if (hoursAgoFrom > 24 * 365 * 100) hoursAgoFrom = 24 * 365 * 100;
            var res = _logs.Where(l => (l.Timestamp > (now - TimeSpan.FromHours(hoursAgoFrom))) && l.Timestamp < (now - TimeSpan.FromHours(hoursAgoTo)));
            return Task.FromResult(res);
        }

        public Task StoreLog(RLogEntry entry)
        {
            _logs.Add(entry);
            return Task.CompletedTask;
        }
    }
}
