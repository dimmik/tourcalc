using System.Globalization;
using TCalc.Domain;
using TCalcCore.UI;

namespace TCBlazor.Client.SharedCode
{
    /// <summary>Small presentation helpers shared by the new-UI components.</summary>
    public static class NewUi
    {
        public static string Money(long amount) => amount.ToString("N0", GlobConsts.NumGroupSpaceSeparated);

        public static string Money(double amount) => amount.ToString("N0", GlobConsts.NumGroupSpaceSeparated);

        public static string Initials(string? name)
        {
            var n = (name ?? "").Trim();
            if (n.Length == 0) return "?";
            var parts = n.Split(new[] { ' ', '-', '_' }, StringSplitOptions.RemoveEmptyEntries);
            if (parts.Length >= 2) return $"{parts[0][0]}{parts[1][0]}";
            return n.Length >= 2 ? n.Substring(0, 2) : n.Substring(0, 1);
        }

        /// <summary>Stable pastel-ish colour derived from a name, so a person keeps the same avatar everywhere.</summary>
        public static string AvatarColor(string? seed)
        {
            var s = seed ?? "";
            int hash = 17;
            foreach (var ch in s)
            {
                hash = unchecked(hash * 31 + ch);
            }
            int hue = Math.Abs(hash) % 360;
            int hue2 = (hue + 28) % 360;
            return $"linear-gradient(135deg, hsl({hue} 62% 52%), hsl({hue2} 66% 42%))";
        }

        public static string PersonName(Tour? tour, string? guid)
            => tour?.Persons?.FirstOrDefault(p => p.GUID == guid)?.Name ?? "n/a";

        /// <summary>Human day label: Today / Yesterday / 12 Aug 2025.</summary>
        public static string DayLabel(DateTime date)
        {
            var d = date.Date;
            if (d == DateTime.Now.Date) return "Today";
            if (d == DateTime.Now.Date.AddDays(-1)) return "Yesterday";
            return d.ToString("d MMM yyyy", CultureInfo.InvariantCulture);
        }
    }
}
