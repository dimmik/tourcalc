using System.Collections.Generic;

namespace TCBlazor.Client.SharedCode
{
    /// <summary>
    /// How the people list is arranged while the app is open: who is folded, whose family
    /// is showing its people, and what is in the search box.
    /// <para>
    /// It lives out here rather than in the component because switching tabs destroys
    /// PeopleTab, and folding ten people out of fifteen is work worth keeping across a
    /// glance at the expenses. It is deliberately not written to storage: a reload starts
    /// from a list where everything is in plain sight, so nothing folded a week ago can
    /// quietly hide today's numbers.
    /// </para>
    /// </summary>
    public class PeopleViewState
    {
        public class TourView
        {
            /// <summary>Opened by hand in the dense list, where a person starts folded.</summary>
            public HashSet<string> Expanded { get; } = new();

            /// <summary>Folded by hand in the roomy list, where a person starts open.</summary>
            public HashSet<string> Collapsed { get; } = new();

            public string Search { get; set; } = "";
        }

        private readonly Dictionary<string, TourView> byTour = new();

        public TourView For(string? tourId)
        {
            var key = tourId ?? "";
            if (!byTour.TryGetValue(key, out var v))
            {
                v = new TourView();
                byTour[key] = v;
            }
            return v;
        }
    }
}
