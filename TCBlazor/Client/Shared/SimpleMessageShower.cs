using AntDesign;
using Microsoft.AspNetCore.Components;

namespace TCBlazor.Client.Shared
{
    public class SimpleMessageShower
    {
        private readonly MessageService _messageService;
        private readonly LocalLogger logger;

        public SimpleMessageShower(MessageService messageService, LocalLogger logger)
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
