using System;
using System.IdentityModel.Tokens.Jwt;
using System.Linq;
using System.Security.Claims;
using Microsoft.IdentityModel.Tokens;
using Company.TCBlazor.Auth;

namespace GoldenDump
{
    /// <summary>
    /// Issues and validates tokens with the C# implementation, so the Rust one can be
    /// checked against it in both directions. A token minted by one server has to be
    /// accepted by the other, or the two cannot be run side by side.
    /// </summary>
    public static class Tokens
    {
        const string DevKey = "aSXx0m1XH4K1GfIYR8mi7/XrSWGCH30Eqn074DhewZo=";

        public static void Issue(string authDataJson)
        {
            var key = new ECDSAKey(DevKey);
            var claims = new[]
            {
                new Claim(ClaimTypes.NameIdentifier, "code"),
                new Claim("AuthDataJson", authDataJson),
            };
            var token = new JwtSecurityToken(
                issuer: "TourCalc",
                audience: "Users",
                claims: claims,
                expires: DateTime.Now.AddMinutes(60),
                signingCredentials: new SigningCredentials(key.GetPrivateKey(), key.SigningAlgorithm));
            Console.WriteLine(new JwtSecurityTokenHandler().WriteToken(token));
        }

        public static int Validate(string token)
        {
            var key = new ECDSAKey(DevKey);
            var parameters = new TokenValidationParameters
            {
                ValidateIssuerSigningKey = true,
                IssuerSigningKey = key.GetPublicKey(),
                ValidateIssuer = true,
                ValidIssuer = "TourCalc",
                ValidateAudience = true,
                ValidAudience = "Users",
                ValidateLifetime = true,
                ClockSkew = TimeSpan.FromSeconds(5),
            };
            try
            {
                var principal = new JwtSecurityTokenHandler()
                    .ValidateToken(token, parameters, out _);
                var claim = principal.Claims.FirstOrDefault(c => c.Type == "AuthDataJson")?.Value;
                Console.WriteLine("valid. AuthDataJson = " + (claim ?? "(none)"));
                return 0;
            }
            catch (Exception e)
            {
                Console.WriteLine("REJECTED: " + e.Message);
                return 1;
            }
        }
    }
}
