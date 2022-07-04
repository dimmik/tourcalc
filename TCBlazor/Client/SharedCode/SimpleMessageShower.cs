using AntDesign;
using Microsoft.AspNetCore.Components;
using TCalcCore.Logging;
using TCalcCore.UI;

namespace TCBlazor.Client.SharedCode
{
    public class SimpleMessageShower : ISimpleMessageShower
    {
        private readonly MessageService _messageService;
        private readonly ILocalLogger logger;

        public SimpleMessageShower(MessageService messageService, ILocalLogger logger)
        {
            _messageService = messageService;
            this.logger = logger;
        }

        private static RenderFragment getMessage(string message)
        {
            return __builder =>
            {
                __builder.AddContent(0, message);
            };
        }
        public void ShowError(string txt)
        {
            _messageService.Error(getMessage(txt));
            logger.Log(txt);
        }
    }
}
