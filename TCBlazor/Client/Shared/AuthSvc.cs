using TCalcCore.Auth;
using TCalcCore.Network;

namespace TCBlazor.Client.Shared
{
    public class AuthSvc
    {
        private readonly TCDataService dataSvc;

        public AuthSvc(TCDataService dataSvc)
        {
            this.dataSvc = dataSvc ?? throw new ArgumentNullException(nameof(dataSvc));
        }

        public AuthData? Auth { get; set; }
        public async Task LogIn(string? scope, string? code)
        {
            await dataSvc.GetAndStoreToken(scope, code);
            await PickUpAuthInfo();
        }
        public async Task PickUpAuthInfo()
        {
            Auth = await dataSvc.GetAuthData();
        }
        public async Task LogOut()
        {
            await dataSvc.ClearToken();
            await PickUpAuthInfo();
        }
    }
}
