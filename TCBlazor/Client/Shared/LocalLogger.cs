namespace TCBlazor.Client.Shared
{
    public class LocalLogger
    {
        public void Log(string msg)
        {
            Console.WriteLine($"{DateTime.Now:yyyyMMdd-HH:mm:ss} -- {msg}");
        }
    }
}
