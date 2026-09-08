using System;
using TCalcCore.Logging;
using TCalcCore.UI;

namespace TCBlazor.Client.SharedCode
{
    /// <summary>
    /// Errors are raised from the data layer, which has no component to render into, so
    /// this only announces them: <see cref="Components.TcMessages"/> sits in the layout
    /// and draws whatever arrives. Nobody has to be listening - an error with no host on
    /// screen is still written to the log, which is where it went before anyway.
    /// </summary>
    public class SimpleMessageShower : ISimpleMessageShower
    {
        private readonly ILocalLogger logger;

        public SimpleMessageShower(ILocalLogger logger)
        {
            this.logger = logger;
        }

        public event Action<string> OnError;

        public void ShowError(string txt)
        {
            logger.Log(txt);
            OnError?.Invoke(txt);
        }
    }
}
