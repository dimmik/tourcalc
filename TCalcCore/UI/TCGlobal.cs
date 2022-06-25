using TCalc.Domain;

namespace TCalcCore.UI
{
    public class TCGlobal
    {
        private string _title = "Tourcalc";
        public string Title
        {
            get => _title;
            set
            {
                if (value != _title)
                {
                    _title = value;
                    OnChange?.Invoke();
                }
            }
        }
        private Tour _tour;
        public Tour Tour
        {
            get => _tour;
            set
            {
               _tour = value;
               OnChange?.Invoke();
            }
        }




        public delegate void onchange();
        public onchange OnChange { get; set; } = null;
    }
}
