using TCBlazor.Shared;

namespace TCBlazor.Server
{
    public class ServerSidePrerenderingContext : IPrerenderingContext
    {
        public bool IsServerRender => true;
    }
}
