using AntDesign;
using TCalc.Domain;

namespace TCBlazor.Client.SharedCode
{
    public static class TourExtensions
    {
        public static int GetAmountInCurrentCurrencyFromMinValued(this Tour tour, int amount)
        {
            if (tour == null) return amount;
            if (!tour.IsMultiCurrency()) return amount;
            var currentCurrencyRate = tour.Currency.CurrencyRate;
            var minCurrencyRate = tour.Currencies.Min(c => c.CurrencyRate);
            var rate = minCurrencyRate*1.0/currentCurrencyRate;
            int result = (int)Math.Floor(amount * rate);
            return result;
        }
    }
}
