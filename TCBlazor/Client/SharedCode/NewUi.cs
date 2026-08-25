using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using TCalc.Domain;
using TCalcCore.UI;

namespace TCBlazor.Client.SharedCode
{
    /// <summary>Small presentation helpers shared by the new-UI components.</summary>
    public static class NewUi
    {
        public static string Money(long amount) => amount.ToString("N0", GlobConsts.NumGroupSpaceSeparated);

        public static string Money(double amount) => amount.ToString("N0", GlobConsts.NumGroupSpaceSeparated);

        public static string Initials(string? name) => Initials(name, (IEnumerable<string>?)null);

        /// <summary>Convenience overload: the peers are the other people of the tour.</summary>
        public static string Initials(string? name, Tour? tour)
            => Initials(name, tour?.Persons?.Select(p => p.Name));

        /// <summary>
        /// Avatar label. Normally the plain first two letters; it only reaches for something
        /// more distinctive when another name in the same tour would produce the same label
        /// (which is how "Родители" and "Рома" both ended up as "РО").
        /// </summary>
        public static string Initials(string? name, IEnumerable<string>? peers)
        {
            var n = (name ?? "").Trim();
            if (n.Length == 0) return "?";

            var mine = Candidates(n).ToList();
            if (peers == null) return mine[0];

            var others = peers
                .Select(p => (p ?? "").Trim())
                .Where(p => p.Length > 0 && !string.Equals(p, n, StringComparison.OrdinalIgnoreCase))
                .ToList();
            if (others.Count == 0) return mine[0];

            var taken = new HashSet<string>(others.Select(p => Candidates(p).First()), StringComparer.OrdinalIgnoreCase);
            foreach (var candidate in mine)
            {
                if (!taken.Contains(candidate)) return candidate;
            }
            return mine[0];
        }

        private static IEnumerable<string> Candidates(string n)
        {
            var parts = n.Split(new[] { ' ', '-', '_' }, StringSplitOptions.RemoveEmptyEntries);
            if (parts.Length >= 2)
            {
                yield return $"{parts[0][0]}{parts[1][0]}";
                if (parts[1].Length >= 2) yield return $"{parts[0][0]}{parts[1][0]}{parts[1][1]}";
                yield break;
            }
            var w = parts.Length == 1 ? parts[0] : n;
            if (w.Length == 1) { yield return w; yield break; }
            yield return w.Substring(0, 2);
            yield return $"{w[0]}{w[w.Length - 1]}";
            if (w.Length >= 3) yield return w.Substring(0, 3);
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
