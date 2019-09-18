﻿using System;
using System.Collections.Generic;
using System.Text;

namespace TCalc.Domain
{
    public class Tour : AbstractItem
    {
        public List<Person> Persons { get; set; } = new List<Person>();
        public List<Spending> Spendings { get; set; } = new List<Spending>();
        public string Name { get; set; } = $"Tour of {DateTime.Now.ToLongDateString()}";
        public DateTime DateCreated { get; set; } = DateTime.Now;
        public string Id { get { return GUID; } set { GUID = value; } }
        public string AccessCode { get; set; } = Convert.ToBase64String(BitConverter.GetBytes(DateTime.Now.Millisecond)).Substring(0, 6);
    }
}
