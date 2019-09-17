using Microsoft.IdentityModel.Tokens;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;

namespace TourCalcWebApp.Auth
{
    public interface IECDsaCryptoKey
    {
        SecurityKey GetPrivateKey();
        SecurityKey GetPublicKey();
        ECDsaPublicKey GetPublicKeyToShare();
        string SigningAlgorithm { get; }
    }
}
