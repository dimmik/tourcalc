using TCalcCore.Logging;

namespace TCBlazor.Client.SharedCode
{
    public class LocalLogger : ILocalLogger
    {
        public void Log(string msg)
        {
            Console.WriteLine($"{DateTime.Now:yyyyMMdd-HH:mm:ss} -- {msg}");
        }
    }
}
