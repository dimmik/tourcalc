using TCalcCore.Storage;

namespace TCBlazor.Client.SharedCode
{
    /// <summary>
    /// Keeps the "new UI / classic UI" choice. The value lives in localStorage so it survives reloads,
    /// and every component that renders differently in the two modes subscribes to <see cref="OnChanged"/>.
    /// </summary>
    public class UiModeService
    {
        private const string StorageKey = "__tc_ui_mode";
        private const string NewValue = "new";
        private const string OldValue = "old";

        private readonly ITourcalcLocalStorage storage;
        // The new UI is the default now. Only the absence of a stored value counts as
        // "no choice made": anyone who has ever picked Classic has "old" written down and
        // keeps it, because Get() is called without storeDefaultValue and therefore never
        // wrote the old default to anybody's browser.
        private bool _isNew = true;
        private bool _initialized = false;

        public UiModeService(ITourcalcLocalStorage storage)
        {
            this.storage = storage;
        }

        public bool IsNew => _isNew;
        public bool Initialized => _initialized;

        public event Action? OnChanged;

        public async Task Init()
        {
            if (_initialized) return;
            _initialized = true;
            try
            {
                var (val, _) = await storage.Get(StorageKey, defVal: NewValue);
                _isNew = val == NewValue;
            }
            catch
            {
                _isNew = true;
            }
            OnChanged?.Invoke();
        }

        public async Task SetMode(bool isNew)
        {
            _initialized = true;
            if (_isNew == isNew) return;
            _isNew = isNew;
            OnChanged?.Invoke();
            await storage.Set(StorageKey, isNew ? NewValue : OldValue);
        }

        public Task Toggle() => SetMode(!_isNew);
    }
}
