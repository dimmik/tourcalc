//! English: the language the app was written in.

use super::*;

pub const TEXTS: Texts = Texts {
    shell: ShellTexts {
        tour_list: "Tour list",
        back_to_tours: "Back to your tours",
        back_to_tour: "Back to the tour",
        back_to: |name| format!("Back to {name}"),
        help_title: "Help",
        settings_title: "Settings",
        menu: "Menu",
        help: "Help",
        help_hint: "What everything here means",
        settings: "Settings",
        log_out: "Log out",
        a_tour_changed: "A tour you are notified about has changed.",
        open: "Open",
        dismiss: "Dismiss",
        signing_in: "Signing in…",
        switch_interface: "Switch the interface",
        full_hint: "Full interface - the roomy view",
        mini_hint: "Mini interface - one line per thing, for a small screen or a slow device",
        login_lead: "Enter the access code for your tour to continue.",
        access_code: "Access code",
        your_code: "your code",
        logging_in: "Logging in…",
        log_in: "Log in",
        no_code: "No code at hand? Opening a tour link signs you in by itself — ask whoever \
            shares the tour to send it again.",
    },
    settings: SettingsTexts {
        title: "Settings",
        language: "Language",
        language_desc: "What the app is written in on this device. Names of tours, people \
            and categories stay as they were typed; notifications and the change log are in \
            English.",
        language_auto: |now| format!("As the browser ({now})"),
        min_debt: "Ignore debts smaller than",
        min_debt_desc: "Anything below this is treated as settled, so rounding leftovers of a \
            few coins stop showing up as debts. On a tour with several currencies it is scaled, \
            so the same amount of money counts as noise whichever one you are reading in.",
        accent: "Accent colour",
        accent_desc: "The header, the buttons and the highlights. Everything else stays the \
            neutral grey it is now, so this only changes the accent, never the text. The last \
            sample opens the colour picker if none of the seven is the one you want; a very \
            light pick is darkened a little so the white text on top of it stays readable.",
        accent_other: "Any other colour",
        accent_names: ["Indigo (default)", "Ocean", "Teal", "Forest", "Plum", "Crimson", "Graphite"],
        saved: "Saved in this browser. Settings live here and not on the server, so they \
            follow the device rather than the tour.",
        device: "This device",
        check: "Check for other people's changes",
        check_desc: "While a tour is open and on screen, it asks the server whether somebody \
            else has changed it, and brings the change in if so. It always asks the moment you \
            come back to the tab; this is how often it asks in between. Each question is a few \
            bytes, and nothing is asked while the tab is hidden or the phone is locked. Applies \
            to the next tour you open.",
        check_on_return: "only when I come back",
        check_minute: "every minute",
        check_seconds: |n| format!("every {n} s"),
        elsewhere: "Elsewhere",
        interface: "Interface",
        interface_desc: "Full is the roomy view; Mini draws it one line per thing. The switch \
            is in the header, next to the help.",
        notifications: "Notifications",
        notifications_desc: "A bell on each tour rather than one switch for all of them — \
            being told about the flat-share is not the same as being told about last year's trip.",
        explain: "Tap any number for details",
        explain_desc: "Always on here. It is what makes a figure checkable, and a setting for \
            it is a setting for “show me less”.",
        how_it_counts: "How Tourcalc counts",
        how_it_counts_desc: "What a weight is, why a total can be smaller than the sum of the \
            expenses, what “uncounted” means, and every other word on screen.",
    },
};
