using Newtonsoft.Json;
using System;
using System.Collections.Generic;
using System.Linq;
using TCalc.Domain;
using TCalcCore.Logging;

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
        public SerializableTourOperation(string operationName, string itemId, object item)
        {
            OperationName = operationName;
            ItemId = itemId;
            ItemJson = JsonConvert.SerializeObject(item);
        }

        public Func<Tour, Tour> ApplyOperationFunc(ITourStorageProcessor tourStorageProcessor, string context = "", ILocalLogger logger = null)
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
                case "changename":
                    Tour tcn = JsonConvert.DeserializeObject<Tour>(ItemJson ?? "");
                    logger?.Log($"({context}) new name: {tcn.Name}");
                    return t => { t.Name = tcn.Name; return t; };
                case "finalizing":
                    Tour tf = JsonConvert.DeserializeObject<Tour>(ItemJson ?? "");
                    logger?.Log($"({context}) new finalizing: {tf.IsFinalizing}");
                    return t => { t.IsFinalizing = tf.IsFinalizing; return t; };
                case "archive":
                    Tour ta = JsonConvert.DeserializeObject<Tour>(ItemJson ?? "");
                    logger?.Log($"({context}) new archived: {ta.IsArchived}");
                    return t => { t.IsArchived = ta.IsArchived; return t; };
                case "SetTourCurrency":
                case "ChangeTourCurrency":
                case "UpdateTourCurrency":
                    Currency curr = JsonConvert.DeserializeObject<Currency>(ItemJson ?? "");
                    logger?.Log($"({context}) change tour currency to: {curr.Name}");
                    return t => { t.Currency = curr; return t; };
                case "SetTourCurrencies":
                case "ChangeTourCurrencies":
                case "UpdateTourCurrencies":
                    IEnumerable<Currency> currencies = JsonConvert.DeserializeObject<IEnumerable<Currency>>(ItemJson ?? "");
                    logger?.Log($"({context}) change tour currencies to: {currencies.Count()}");
                    return t => { t.Currencies = currencies; return t; };
                default:
                    return t => t;
            }
        }
    }
}
