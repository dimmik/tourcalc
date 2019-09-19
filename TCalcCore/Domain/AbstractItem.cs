using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Text;

namespace TCalc.Domain
{
    public class AbstractItem
    {
        public string Metadata { get; set; }
        public string GUID { get; set; } = IdHelper.NewId();
        public long Order { get; set; }
        public bool IsChanged { get; set; } = true;
        public bool IsFromJson { get; set; } = false;
    }
}
