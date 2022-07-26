using System.Collections.Generic;
using System.Linq;
using TCalc.Domain;

namespace TCalcCore.UI
{
    public static class EnumerableExtensions
    {
        public static IEnumerable<Person> SortedByNameAndDependency(this IEnumerable<Person> persons)
        {
            IEnumerable<Person> pers = persons ?? new List<Person>();
            var res = pers.OrderBy(p =>
            {
                var name = PersonNameWithParentNamesRecursive(p, persons);
                return name;
            });
            return res;
        }
        private static string PersonNameWithParentNamesRecursive(Person p, IEnumerable<Person> persons, int level = 0)
        {
            if (level > 50) return "";
            if (p == null) return "";
            if (string.IsNullOrWhiteSpace(p?.ParentId)) return p?.Name ?? "";
            return $"{PersonNameWithParentNamesRecursive(persons?.FirstOrDefault(pp => pp.GUID == (p?.ParentId ?? "")), persons, level + 1)}{p?.Name ?? ""}";
        }

    }
}
