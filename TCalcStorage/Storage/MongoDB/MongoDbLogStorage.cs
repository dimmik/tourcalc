using MongoDB.Bson.Serialization.Conventions;
using MongoDB.Driver;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Web;
using TCalcCore.Storage;

namespace TCalcStorage.Storage.MongoDB
{
    public class MongoDbLogStorage : ILogStorage
    {
        private readonly MongoClient client;

        public MongoDbLogStorage(string url, string username, string password)
        {
            var conventionPack = new ConventionPack { new IgnoreExtraElementsConvention(true) };
            ConventionRegistry.Register("IgnoreExtraElements", conventionPack, type => true);
            //            client = new MongoClient($"mongodb+srv://mongo:aAdmin001@dimmik-mongo-4su6a.azure.mongodb.net");
            client = new MongoClient($"mongodb+srv://{HttpUtility.UrlEncode(username)}:{HttpUtility.UrlEncode(password)}@{url}?connect=replicaSet");

            //check connectivity
            GetLogEntries(0, 0).Wait();
        }

        public async Task<IEnumerable<RLogEntry>> GetLogEntries(int hoursAgoFrom = int.MaxValue, int hoursAgoTo = 0)
        {
            var db = client.GetDatabase("tour");
            var coll = db.GetCollection<RLogEntry>("Logs");
            var now = DateTimeOffset.Now;
            // .Where(l => (l.Timestamp > (now - TimeSpan.FromHours(hoursAgoFrom))) && l.Timestamp < (now - TimeSpan.FromHours(hoursAgoTo)));
            var logs = (await coll
                .FindAsync(l => (l.Timestamp > (now - TimeSpan.FromHours(hoursAgoFrom))) && l.Timestamp < (now - TimeSpan.FromHours(hoursAgoTo))))
                .ToList();
                ;
            return logs;
        }

        public async Task StoreLog(RLogEntry entry)
        {
            var db = client.GetDatabase("tour");
            var coll = db.GetCollection<RLogEntry>("Logs");
            await coll.InsertOneAsync(entry);
        }
    }
}
