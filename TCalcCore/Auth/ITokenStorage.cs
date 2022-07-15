using System;
using System.Collections.Generic;
using System.Text;
using System.Threading.Tasks;

namespace TCalcCore.Auth
{
    public interface ITokenStorage
    {
        Task<string> GetToken();
        Task SetToken(string token);
    }
}
