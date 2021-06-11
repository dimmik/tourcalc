
namespace TourCalcWebApp.Auth
{
    public interface IECDsaCryptoKey
    {
        Microsoft.IdentityModel.Tokens.SecurityKey GetPrivateKey();
        Microsoft.IdentityModel.Tokens.SecurityKey GetPublicKey();
        ECDsaPublicKey GetPublicKeyToShare();
        string SigningAlgorithm { get; }
    }
}
