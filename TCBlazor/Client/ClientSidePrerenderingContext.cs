using TCBlazor.Shared;

namespace TCBlazor.Client
{
    public class ClientSidePrerenderingContext : IPrerenderingContext
    {
        public bool IsServerRender => false;
    }
}
