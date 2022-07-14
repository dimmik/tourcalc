using TCBlazor.Shared;

namespace TCBlazor.Server
{
    public class PrerenderingContext : IPrerenderingContext
    {
        public bool IsServerRender => true;
    }
}
