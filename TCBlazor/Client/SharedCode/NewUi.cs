using System.Collections.Generic;
using System.Globalization;
using System.Text.RegularExpressions;
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

        /// <summary>
        /// A hand-picked colour marks a rare, out-of-the-ordinary expense - a tourist tax for
        /// the whole group, an unexpected fine - so it stops the eye in the list. Only the hex
        /// the colour input produces counts as a human choice: "lightgreen"/"lightgray" are
        /// written by the app itself when a payment is marked paid, and must not light up.
        /// </summary>
        public static bool IsMarked(string? color) => ParseHex(color) != null;

        /// <summary>
        /// Inline custom properties for a marked row. The chosen hue is kept, but its lightness
        /// is not: a near-white or near-black pick would otherwise make the mark invisible or
        /// murky, and whether the mark is noticeable must not depend on what the colour dialog
        /// happened to offer under the finger.
        /// </summary>
        public static string MarkStyle(string? color)
        {
            var rgb = ParseHex(color);
            if (rgb == null) return "";
            var (h, s, _) = ToHsl(rgb.Value);
            // black, white and grey carry no hue to keep; they become a heavy neutral outline
            // rather than the dusty pink that clamping a hue-less colour would produce
            var flat = s < .05;
            var line = flat ? FromHsl(0, 0, .32) : FromHsl(h, Clamp(s, .30, .85), .40);
            var bg = flat ? FromHsl(0, 0, .93) : FromHsl(h, Clamp(s, .25, .80), .945);
            return $"--tcn-mark-line:{line};--tcn-mark-bg:{bg};";
        }

        private static (int r, int g, int b)? ParseHex(string? color)
        {
            var c = (color ?? "").Trim();
            if (c.Length == 0 || c[0] != '#') return null;
            c = c.Substring(1);
            if (c.Length == 3)
            {
                c = new string(new[] { c[0], c[0], c[1], c[1], c[2], c[2] });
            }
            if (c.Length != 6) return null;
            if (!int.TryParse(c, NumberStyles.HexNumber, CultureInfo.InvariantCulture, out var v)) return null;
            return ((v >> 16) & 0xFF, (v >> 8) & 0xFF, v & 0xFF);
        }

        private static double Clamp(double v, double min, double max) => v < min ? min : v > max ? max : v;

        private static (double h, double s, double l) ToHsl((int r, int g, int b) c)
        {
            double r = c.r / 255.0, g = c.g / 255.0, b = c.b / 255.0;
            double max = Math.Max(r, Math.Max(g, b)), min = Math.Min(r, Math.Min(g, b));
            double l = (max + min) / 2;
            if (Math.Abs(max - min) < 1e-9) return (0, 0, l);
            double d = max - min;
            double s = l > .5 ? d / (2 - max - min) : d / (max + min);
            double h;
            if (max == r) h = (g - b) / d + (g < b ? 6 : 0);
            else if (max == g) h = (b - r) / d + 2;
            else h = (r - g) / d + 4;
            return (h * 60, s, l);
        }

        private static string FromHsl(double h, double s, double l)
        {
            double c = (1 - Math.Abs(2 * l - 1)) * s;
            double hp = ((h % 360) + 360) % 360 / 60;
            double x = c * (1 - Math.Abs(hp % 2 - 1));
            double r = 0, g = 0, b = 0;
            if (hp < 1) { r = c; g = x; }
            else if (hp < 2) { r = x; g = c; }
            else if (hp < 3) { g = c; b = x; }
            else if (hp < 4) { g = x; b = c; }
            else if (hp < 5) { r = x; b = c; }
            else { r = c; b = x; }
            double m = l - c / 2;
            return $"#{Byte(r + m):x2}{Byte(g + m):x2}{Byte(b + m):x2}";
        }

        private static int Byte(double v) => (int)Math.Round(Clamp(v, 0, 1) * 255);

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

        // "X 'Женя К.' -> 'Паша'" / "Family 'Олежка' -> 'Саша О.'" - the descriptions the
        // calculator writes for the rows it generates itself
        private static readonly Regex ServiceRow =
            new Regex(@"^(?<kind>X|Family) '(?<from>.+)' -> '(?<to>.+)'$", RegexOptions.Compiled);

        /// <summary>
        /// Generated rows are shown to people alongside their own expenses, so the quotes and
        /// the ASCII arrow of the internal wording have no business on screen. Returns null for
        /// anything a human typed.
        /// </summary>
        public static (string Kind, string From, string To)? AsServiceTransfer(string? description)
        {
            var m = ServiceRow.Match(description ?? "");
            if (!m.Success) return null;
            return (m.Groups["kind"].Value, m.Groups["from"].Value, m.Groups["to"].Value);
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
