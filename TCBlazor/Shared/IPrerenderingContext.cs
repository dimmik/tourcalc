using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace TCBlazor.Shared
{
    public interface IPrerenderingContext
    {
        bool IsServerRender { get; }
    }
}
