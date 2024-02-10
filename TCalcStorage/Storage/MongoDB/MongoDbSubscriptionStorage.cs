using MongoDB.Bson.Serialization.Conventions;
using MongoDB.Driver;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Web;
using TCalcCore.Storage;
using TCBlazor.Client.SharedCode;

namespace TCalcStorage.Storage.MongoDB
{
    public class MongoDbSubscriptionStorage : ISubscriptionStorage
    {
        private readonly MongoClient client;

        public MongoDbSubscriptionStorage(string url, string username, string password)
        {
            var conventionPack = new ConventionPack { new IgnoreExtraElementsConvention(true) };
            ConventionRegistry.Register("IgnoreExtraElements", conventionPack, type => true);
            //            client = new MongoClient($"mongodb+srv://mongo:aAdmin001@dimmik-mongo-4su6a.azure.mongodb.net");
            client = new MongoClient($"mongodb+srv://{HttpUtility.UrlEncode(username)}:{HttpUtility.UrlEncode(password)}@{url}?connect=replicaSet");
            
            //check connectivity
            GetSubscriptions("none");
        }

        public void AddSubscription(string tourId, NotificationSubscription sub)
        {
            var db = client.GetDatabase("tour");
            var coll = db.GetCollection<TourIdAndNotificationSubscription>("NSubscriptions");
            var cnt = coll.Find(t => t.TourId == tourId && t.Subscription.Url == sub.Url).CountDocuments();
            if (cnt == 0)
            {
                coll.InsertOne(new TourIdAndNotificationSubscription()
                {
                    TourId = tourId,
                    Subscription = sub
                });
            }
        }

        public IEnumerable<NotificationSubscription> GetSubscriptions(string tourId)
        {
            var db = client.GetDatabase("tour");
            var coll = db.GetCollection<TourIdAndNotificationSubscription>("NSubscriptions");
            var subs = coll.Find(t => t.TourId == tourId).ToList();
            var dist = subs.Select(tn => tn.Subscription).Distinct();
            return dist;
        }
    }
    class TourIdAndNotificationSubscription
    {
        public string TourId { get; set; }
        public NotificationSubscription Subscription { get; set; }
    }
}
