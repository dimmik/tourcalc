namespace TCalcCore.UI
{
    public class UISettings
    {
        public int MinimumMeaningfulDebt { get; set; } = 49;
        public int Magic_Piechart_Color_Scheme_Number { get; set; } = 1630;
        /*public DateTimeOffset SomeDate { get; set; } = DateTimeOffset.Now;
        public string SomeString { get; set; } = "aaa";
        public string NotAProp = "";*/
        public bool Default_Tour_Page_Is_Add_Spending { get; set; } = true;
        public bool Show_Debug_UI { get; set; } = false;
        public bool Show_Mass_Spending_Change { get; set; } = false;
        public bool Collapse_Columns_In_Person_List_On_Smaller_Screen { get; set; } = true;
        public int Smaller_Screen_Width { get; set; } = 500;
    }
}
