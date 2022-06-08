using Newtonsoft.Json;
using System;
using System.Collections.Generic;
using System.ComponentModel;

namespace TCalc.Domain
{
    public class AbstractItem : IEquatable<AbstractItem>
    {
        public string Metadata { get; set; }
        public string GUID { get; set; } = "NewId";// = IdHelper.NewId();
        public long Order { get; set; }
        public bool IsChanged { get; set; } = true;
        public bool IsFromJson { get; set; } = false;
        public DateTime DateCreated { get; set; } = DateTime.Now;

        public override bool Equals(object obj)
        {
            var other = obj as AbstractItem;
            if (other == null) return false;
            return GUID == other.GUID;
        }

        public bool Equals(AbstractItem other)
        {
            return other != null &&
                   GUID == other.GUID;
        }

        public override int GetHashCode()
        {
            return GUID.GetHashCode();
        }
        public T SafeClone<T>() where T : AbstractItem
        {
            T clone = JsonConvert.DeserializeObject<T>(JsonConvert.SerializeObject(this));
            return clone;
        }
    }
}
