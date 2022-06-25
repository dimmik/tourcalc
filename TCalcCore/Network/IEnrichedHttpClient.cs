using System.Net.Http;
using System.Threading.Tasks;

namespace TCalcCore.Network
{
    public interface IEnrichedHttpClient
    {
        Task<T> CallWithAuthToken<T>(string url, string token, bool showErrorMessages = true);
        Task<T> CallWithAuthToken<T>(string url, string token, HttpMethod method, object body, bool showErrorMessages = true);
        Task<string> GetStringAsync(string url, bool showErrorMessages = true);
    }
}