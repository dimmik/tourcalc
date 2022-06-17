using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using TCalc.Domain;

namespace TCalc.Storage
{
    public class TourStorageProcessor
    {
        public Tour AddPerson(Tour tour, Person p)
        {
            p.GUID = IdHelper.NewId();
            tour.Persons.Add(p);
            tour.Spendings.RemoveAll(s => s.Planned);
            return tour;
        }

        public Tour UpdatePerson(Tour t, Person p, string personguid)
        {
            p.GUID = personguid;
            var idx = t.Persons.FindIndex(x => x.GUID == personguid);
            if (idx < 0) return null;
            p.DateCreated = t.Persons[idx].DateCreated; // preserve

            t.Persons[idx] = p;

            t.Spendings.RemoveAll(s => s.Planned);

            return t;
        }

        public Tour DeletePerson(Tour t, string personguid)
        {
            var removedPerson = t.Persons.SingleOrDefault(x => x.GUID == personguid);
            if (removedPerson == null) return null;

            t.Persons.Remove(removedPerson);
            t.Persons.Where(p => p.ParentId == removedPerson.GUID).ToList().ForEach(p => p.ParentId = null);
            t.Spendings.RemoveAll(s => s.FromGuid == removedPerson.GUID);
            t.Spendings.RemoveAll(s => s.Planned);
            t.Spendings.ForEach(s => s.ToGuid.RemoveAll(g => g == removedPerson.GUID));

            // We might have removed the single toguid from the spending. Let's remove such spendings that are not toall at the same time
            //(or maybe better to make the to all?..)
            t.Spendings.RemoveAll(s => (!s.ToAll && !s.ToGuid.Any()));

            return t;
        }

        public Tour UpdateSpending(Tour t, Spending sp, string spendingid)
        {
            sp.GUID = spendingid;
            var idx = t.Spendings.FindIndex(x => x.GUID == spendingid);
            if (idx < 0) return null;
            sp.DateCreated = t.Spendings[idx].DateCreated; // preserve
            t.Spendings[idx] = sp;
            return t;
        }

        public Tour AddSpending(Tour t, Spending s)
        {
            if (t.Spendings.Contains(s))
            {
                t.Spendings.Remove(s);
            }
            // keep some planned spendings, so that is someone is adding from middle of the list - not huge recalc
            var plannedAndTheSame = t.Spendings.Where(ss => ss.Planned && ss.IsAlmostTheSame(s)).ToList();
            if (plannedAndTheSame.Any())
            {
                plannedAndTheSame.ForEach(pp => t.Spendings.Remove(pp));
            }
            s.GUID = IdHelper.NewId();
            s.DateCreated = DateTime.UtcNow;
            s.Planned = false;
            t.Spendings.Add(s);
            return t;
        }

        public Tour DeleteSpending(Tour t, string spendingid)
        {
            var removedSpending = t.Spendings.SingleOrDefault(x => x.GUID == spendingid);
            if (removedSpending != null)
            {
                t.Spendings.Remove(removedSpending);
            } else
            {
                return null;
            }
            return t;
        }
    }
}
