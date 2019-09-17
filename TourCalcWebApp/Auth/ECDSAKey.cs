using Microsoft.IdentityModel.Tokens;
using Org.BouncyCastle.Asn1.Sec;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Security.Cryptography;
using System.Threading.Tasks;

namespace TourCalcWebApp.Auth
{
    public class ECDSAKey : IECDsaCryptoKey
    {

        private readonly byte[] privateKey;

        public string SigningAlgorithm => SecurityAlgorithms.EcdsaSha256;

        public ECDSAKey(string privateKeyB64)
        {
            privateKey = Convert.FromBase64String(privateKeyB64);
        }
        public ECDSAKey(byte[] privateKeyBA)
        {
            privateKey = privateKeyBA;
        }


        public SecurityKey GetPrivateKey()
        {
            var ecdsa = GetECDsaFromPrivateKey(privateKey);
            var securityKey = new ECDsaSecurityKey(ecdsa);
            return securityKey;
        }

        public SecurityKey GetPublicKey()
        {
            var ecdsa = GetECDsaFromPrivateKey(privateKey);
            var ecdsaPub = ECDsa.Create(ecdsa.ExportParameters(includePrivateParameters: false));
            var securityKey = new ECDsaSecurityKey(ecdsaPub);
            return securityKey;
        }

        private static ECDsa GetECDsaFromPrivateKey(byte[] key, ECDsaPublicKey publicKeyToFill = null)
        {
            var privKeyInt = new Org.BouncyCastle.Math.BigInteger(+1, key);
            var parameters = SecNamedCurves.GetByName("secp256r1");
            var ecPoint = parameters.G.Multiply(privKeyInt);
            var pubKeyX = ecPoint.Normalize().XCoord.ToBigInteger().ToByteArrayUnsigned();
            var pubKeyY = ecPoint.Normalize().YCoord.ToBigInteger().ToByteArrayUnsigned();
            if (publicKeyToFill != null)
            {
                publicKeyToFill.X = pubKeyX;
                publicKeyToFill.Y = pubKeyY;
            }
            return ECDsa.Create(new ECParameters
            {
                Curve = ECCurve.NamedCurves.nistP256,
                D = privKeyInt.ToByteArrayUnsigned(),
                Q = new ECPoint
                {
                    X = pubKeyX,
                    Y = pubKeyY
                }
            });
        }

        public ECDsaPublicKey GetPublicKeyToShare()
        {
            var pubKey = new ECDsaPublicKey();
            var ecdsa = GetECDsaFromPrivateKey(privateKey, pubKey);
            var p = ecdsa.ExportParameters(false);
            //p.
            return pubKey;
        }
    }

}
