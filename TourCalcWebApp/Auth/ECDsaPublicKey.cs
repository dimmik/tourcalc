using Microsoft.IdentityModel.Tokens;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;

namespace TourCalcWebApp.Auth
{
    public class ECDsaPublicKey
    {
        public byte[] X { get; set; }
        public byte[] Y { get; set; }
        public string Algorithm => SecurityAlgorithms.EcdsaSha256;

    }
}
