using TCalcCore.Logging;

namespace TCBlazor.Client.Shared
{
    public class LocalLogger : ILocalLogger
    {
        public void Log(string msg)
        {
            Console.WriteLine($"{DateTime.Now:yyyyMMdd-HH:mm:ss} -- {msg}");
        }
    }
}
