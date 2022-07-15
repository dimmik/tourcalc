using TCalcCore.Storage;

namespace TCBlazor.Server.Pages
{
    // Actually, noop
    public class ServerSideTourcalcLocalStorage : ITourcalcLocalStorage
    {
        public Task<(string val, DateTimeOffset stored)> Get(string key, string defVal = "", bool storeDefaultValue = false)
        {
            return Task.FromResult((defVal, DateTimeOffset.Now));
        }

        public Task Set(string key, string val)
        {
            // nothing
            return Task.CompletedTask;
        }
    }
}
