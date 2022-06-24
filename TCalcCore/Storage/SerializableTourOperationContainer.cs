using System.Collections.Generic;

namespace TCalc.Storage
{
    public class SerializableTourOperationContainer
    {
        public List<SerializableTourOperation> operations { get; set; } = new List<SerializableTourOperation>();
    }
}
