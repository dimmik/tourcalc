using TCalc.Domain;

namespace TCalc.Storage
{
    public interface ITourStorageProcessor
    {
        Tour AddPerson(Tour tour, Person p);
        Tour AddSpending(Tour t, Spending s);
        Tour DeletePerson(Tour t, string personguid);
        Tour DeleteSpending(Tour t, string spendingid);
        Tour UpdatePerson(Tour t, Person p, string personguid);
        Tour UpdateSpending(Tour t, Spending sp, string spendingid);
    }
}