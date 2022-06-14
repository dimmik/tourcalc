namespace TCBlazor.Client.Shared
{
    public class TCGlobal
    {
        public string Title { get; set; } = "Tourcalcx";
        public delegate void onchange();
        public onchange? OnChange { get; set; } = null;
        public void SetTitle(string t)
        {
            Title = t;
            if (OnChange != null)
            {
                OnChange();
            }
        }
    }
}
