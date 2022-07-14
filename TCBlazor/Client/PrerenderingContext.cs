using TCBlazor.Shared;

namespace TCBlazor.Client
{
    public class PrerenderingContext : IPrerenderingContext
    {
        public bool IsServerRender => false;
    }
}
