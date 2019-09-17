using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Text;

namespace TCalc.Domain
{
    public class AbstractItem
    {
        public string Metadata { get; set; }
        public string GUID { get; set; } = Guid.NewGuid().ToString();
        public long Order { get; set; }
        public bool IsChanged { get; set; } = true;
        public bool IsFromJson { get; set; } = false;

        public string FileId()
        {
                return $"{Order.ToString("0000")}_{GUID}";
        }
    }
}
