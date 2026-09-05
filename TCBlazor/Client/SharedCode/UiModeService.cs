using TCalcCore.Storage;

namespace TCBlazor.Client.SharedCode
{
    /// <summary>Which of the three interfaces is on screen.</summary>
    public enum UiMode
    {
        /// <summary>The original interface, kept working and untouched.</summary>
        Classic,

        /// <summary>The roomy new interface - cards, avatars, the whole design system.</summary>
        Full,

        /// <summary>
        /// The same app drawn one line per thing: for an old phone, a small screen, or a
        /// terminal browser. It shares the new interface's dialogs and all of its logic.
        /// </summary>
        Mini,
    }

    /// <summary>
    /// Keeps the interface choice. The value lives in localStorage so it survives reloads,
    /// and every component that renders differently subscribes to <see cref="OnChanged"/>.
    /// </summary>
    public class UiModeService
    {
        private const string StorageKey = "__tc_ui_mode";
        private const string NewValue = "new";
        private const string OldValue = "old";
        private const string MiniValue = "mini";

        private readonly ITourcalcLocalStorage storage;
        // The new UI is the default now. Only the absence of a stored value counts as
        // "no choice made": anyone who has ever picked Classic has "old" written down and
        // keeps it, because Get() is called without storeDefaultValue and therefore never
        // wrote the old default to anybody's browser.
        private UiMode _mode = UiMode.Full;
        private bool _initialized = false;

        public UiModeService(ITourcalcLocalStorage storage)
        {
            this.storage = storage;
        }

        public UiMode Mode => _mode;

        /// <summary>
        /// Anything but the classic interface. Mini is a variant of the new one - it shares
        /// its shell, its dialogs and its stylesheet - so every "is this the new UI" test in
        /// the app is answered by this and needs no third branch.
        /// </summary>
        public bool IsNew => _mode != UiMode.Classic;

        /// <summary>The one-line-per-thing variant. Only the screens it redraws ask this.</summary>
        public bool IsMini => _mode == UiMode.Mini;

        /// <summary>The stored spelling of the mode - also what the page's body classes key off.</summary>
        public string ModeName => Serialize(_mode);

        public bool Initialized => _initialized;

        public event Action? OnChanged;

        public async Task Init()
        {
            if (_initialized) return;
            _initialized = true;
            try
            {
                var (val, _) = await storage.Get(StorageKey, defVal: NewValue);
                _mode = Parse(val);
            }
            catch
            {
                _mode = UiMode.Full;
            }
            OnChanged?.Invoke();
        }

        public async Task SetMode(UiMode mode)
        {
            _initialized = true;
            if (_mode == mode) return;
            _mode = mode;
            OnChanged?.Invoke();
            await storage.Set(StorageKey, Serialize(mode));
        }

        /// <summary>An unknown value can never leave the app without an interface.</summary>
        private static UiMode Parse(string? stored) => stored switch
        {
            OldValue => UiMode.Classic,
            MiniValue => UiMode.Mini,
            _ => UiMode.Full,
        };

        private static string Serialize(UiMode mode) => mode switch
        {
            UiMode.Classic => OldValue,
            UiMode.Mini => MiniValue,
            _ => NewValue,
        };
    }
}
