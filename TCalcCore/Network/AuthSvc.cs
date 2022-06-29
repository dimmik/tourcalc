using System;
using System.Threading.Tasks;
using TCalcCore.Auth;

namespace TCalcCore.Network
{
    public class AuthSvc
    {
        private readonly TCDataService dataSvc;

        public AuthSvc(TCDataService dataSvc)
        {
            this.dataSvc = dataSvc ?? throw new ArgumentNullException(nameof(dataSvc));
        }

        public AuthData Auth { get; set; }
        public async Task LogIn(string scope, string code, bool md5Code)
        {
            if (!md5Code)
            {
                await dataSvc.GetAndStoreToken(scope, code);
            } else
            {
                await dataSvc.GetAndStoreTokenForCodeMd5(code);
            }
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
