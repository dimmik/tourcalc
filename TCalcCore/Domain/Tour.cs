using System;
using System.Collections.Generic;
using System.Text;

namespace TCalc.Domain
{
    public class Tour : AbstractItem
    {
        public List<Person> Persons { get; set; } = new List<Person>();
        public List<Spending> Spendings { get; set; } = new List<Spending>();
        public string Name { get; set; } = $"Tour of {DateTime.Now.ToLongDateString()}";
        public string Id { get { return GUID; } set { GUID = value; } }
        public string AccessCodeMD5 { get; set; } = "-";
        public void StripCalculations()
        {
            // delete spending lists that might be rather large
            Persons.ForEach(p => { p.ReceivedSendingInfo = new List<SpendingInfo>(); p.SpentSendingInfo = new List<SpendingInfo>(); });
        }
    }
}
