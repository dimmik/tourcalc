using Newtonsoft.Json;
using System;
using TCalc.Domain;
using TCalc.Storage;

namespace TCalc.Storage
{
    public class SerializableTourOperation
    {
        public string OperationName { get; set; } = "";
        public string ItemId { get; set; }
        public string ItemJson { get; set; }
        public bool Failed { get; set; } = false;

        public SerializableTourOperation()
        {

        }
        public SerializableTourOperation(string operationName, string itemId, string itemJson)
        {
            OperationName = operationName;
            ItemId = itemId;
            ItemJson = itemJson;
        }
        public SerializableTourOperation(string operationName, string itemId, AbstractItem item)
        {
            OperationName = operationName;
            ItemId = itemId;
            ItemJson = JsonConvert.SerializeObject(item);
        }

        public Func<Tour, Tour> ApplyOperationFunc(ITourStorageProcessor tourStorageProcessor)
        {
            switch (OperationName)
            {
                case "AddSpending":
                    Spending sa = JsonConvert.DeserializeObject<Spending>(ItemJson ?? "");
                    return t => tourStorageProcessor.AddSpending(t, sa) ?? t;
                case "EditSpending":
                case "UpdateSpending":
                    Spending se = JsonConvert.DeserializeObject<Spending>(ItemJson ?? "");
                    return t => tourStorageProcessor.UpdateSpending(t, se, ItemId) ?? t;
                case "DeleteSpending":
                    return t => tourStorageProcessor.DeleteSpending(t, ItemId) ?? t;
                case "AddPerson":
                    Person pa = JsonConvert.DeserializeObject<Person>(ItemJson ?? "");
                    return t => tourStorageProcessor.AddPerson(t, pa) ?? t;
                case "EditPerson":
                case "UpdatePerson":
                    Person pe = JsonConvert.DeserializeObject<Person>(ItemJson ?? "");
                    return t => tourStorageProcessor.UpdatePerson(t, pe, ItemId) ?? t;
                case "DeletePerson":
                    return t => tourStorageProcessor.DeletePerson(t, ItemId) ?? t;
                default:
                    return t => t;
            }
        }
    }
}
