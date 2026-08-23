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
        // the classic UI stays the default until the new one has had some mileage
        private bool _isNew = false;
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
                var (val, _) = await storage.Get(StorageKey, defVal: OldValue);
                _isNew = val == NewValue;
            }
            catch
            {
                _isNew = false;
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
