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
        /// <summary>New UI: make every number tappable, opening a dialog that explains where it comes from.</summary>
        public bool Explain_Numbers { get; set; } = false;
        /// <summary>
        /// New UI: which accent the interface is painted in. One of the keys the stylesheet
        /// knows (indigo, blue, teal, green, plum, crimson, graphite); anything else falls
        /// back to the default, so an unknown value can never leave the app colourless.
        /// </summary>
        public string Accent_Colour { get; set; } = "indigo";
    }
}
