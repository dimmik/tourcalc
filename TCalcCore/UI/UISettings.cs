namespace TCalcCore.UI
{
    public class UISettings
    {
        public int MinimumMeaningfulDebt { get; set; } = 49;
        public int Magic_Piechart_Color_Scheme_Number { get; set; } = 1630;
        public bool Default_Tour_Page_Is_Add_Spending { get; set; } = true;
        public bool Show_Debug_UI { get; set; } = false;
        public bool Show_Mass_Spending_Change { get; set; } = false;
        public bool Collapse_Columns_In_Person_List_On_Smaller_Screen { get; set; } = true;
        public int Smaller_Screen_Width { get; set; } = 500;
        public bool In_Add_Spending_Page_Filter_by_Chosen_Payer { get; set; } = false;
        public bool Spending_ToAll_DefaultOn { get; set; } = true;
        public bool Web_Push_Notifications { get; set; } = false;
    }
}
