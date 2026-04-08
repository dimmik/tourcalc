# "WrongMd5" Tour AccessCodeMD5 Issue

## Summary

Tours are sometimes created with `AccessCodeMD5 = "WrongMd5"` (a sentinel string literal).  
Such tours are invisible to all regular users, can only be "found" by other unauthenticated/invalid-token requests, and the problem is self-reinforcing once even one such tour exists.

---

## Root Cause

### 1. Sentinel set in `AuthHelper.GetAuthData()` — `TCBlazor/Server/Auth/AuthHelper.cs:36-39`

```csharp
if (string.IsNullOrWhiteSpace(authData.AccessCodeMD5))
{
    authData.AccessCodeMD5 = "WrongMd5";   // ← sentinel value
}
```

This runs for **every request**. Any user whose JWT carries an empty `AccessCodeMD5` (or who sends no JWT at all) ends up with `authData.AccessCodeMD5 = "WrongMd5"`.

Users who produce an empty `AccessCodeMD5`:
- **Anonymous** (no token / empty bearer): `AnonymousIsMaster = false` → `new AuthData()` → `AccessCodeMD5 = ""`
- **Admin JWT** (scope = `admin`): `AuthData { IsMaster: true, AccessCodeMD5: "" }` — but these users are masters and follow the safe path (see §2)

### 2. Tour creation assigns the sentinel to the tour — `TCBlazor/Server/Controllers/TourController.cs:187`

```csharp
tourJson.AccessCodeMD5 = authData.IsMaster
    ? AuthHelper.CreateMD5(accessCode)   // master → real MD5
    : authData.AccessCodeMD5s().First(); // non-master → user's stored MD5 ← "WrongMd5" ends up here
```

Non-master anonymous requests get `authData.AccessCodeMD5 = "WrongMd5"`, which is then written to the new tour.

### 3. Self-reinforcing: the `allowed` check uses the same tainted AccessCodeMD5 — `TourController.cs:175`

```csharp
var tours = TourStorageUtilities_LoadAllTours();   // filtered by authData.AccessCodeMD5s()
bool noTours = !tours.Tours.Any();
allowed = !noTours && !maxToursReached;
```

`TourStorageUtilities_LoadAllTours()` for a non-master with `"WrongMd5"` uses the predicate:
```csharp
t => t.AccessCodeMD5 != null && authData.AccessCodeMD5s().Contains(t.AccessCodeMD5)
// = t => t.AccessCodeMD5 == "WrongMd5"
```

So once **one** `"WrongMd5"` tour exists, anonymous/empty-token users pass the `allowed` check and can freely create more.

### 4. Secondary bug: inverted null-check in `AccessCodeMD5s()` — `TCalcCore/Auth/AuthData.cs:25`

```csharp
// BUG: returns { me.AccessCodeMD5 } (the bad/empty value) when it IS null/whitespace
//      should return an empty enumerable instead
if (string.IsNullOrWhiteSpace(me.AccessCodeMD5)) return new[] { me.AccessCodeMD5 };
return me.AccessCodeMD5.Split(Delim);
```

The condition is inverted. In the WrongMd5 scenario this branch is never reached (because `"WrongMd5"` is not whitespace), but the method would silently pass an empty-string match to predicates in other edge cases.

### 5. `UpdateTour` (PATCH) does not sanitise `AccessCodeMD5` — `TourController.cs:232`

```csharp
tourJson.GUID = tourid;
TourStorage_StoreTour(tourJson);   // stores whatever AccessCodeMD5 is in the body
```

Any tour that already has `AccessCodeMD5 = "WrongMd5"` will preserve it through every subsequent save/update cycle.

---

## Bootstrap: how does the very first "WrongMd5" tour appear?

The `allowed` check prevents anonymous creation of the first "WrongMd5" tour through the normal `AddTour` path (no existing tours → `noTours = true` → Forbid).

Likely sources of the initial seeded tour:

| Vector | How |
|--------|-----|
| **Older code** | A previous version of the app may have had different logic that allowed this |
| **Direct DB/file manipulation** | If using `InMemory` storage backed by `inmemory-tours.json`, a manually edited file could introduce it |
| **`TourVersions.RestoreTour()` bug** | Restores pass `Guid.NewGuid().ToString("N")` as the access code (`TourVersions.razor:77`). For a master user this creates a tour with `CreateMD5(randomGuid)` — an inaccessible but real MD5. Not "WrongMd5" itself, but a related creation bug |
| **Config variation** | `AnonymousIsMaster = true` (in a non-default deploy) would allow anonymous master creation — but master always uses `CreateMD5(accessCode)`, so still doesn't produce "WrongMd5" via tour creation |

> **Conclusion**: The current code cannot produce the first "WrongMd5" tour through the standard UI flow alone. It requires either a historical code path or a DB-level entry. Once one exists, the cycle is completely self-sustaining.

---

## Reproduction Steps

### Precondition: create the bootstrap tour (direct API call)

1. Ensure there is at least one tour in the database whose `AccessCodeMD5` field equals exactly `WrongMd5`.  
   The easiest way to produce this artificially is a direct curl (no bearer token) — but this is only possible if such a tour already exists in the target environment. If you are setting up a clean repro environment:
   - Use the InMemory file storage (`StorageType = InMemory`)
   - Manually add to `inmemory-tours.json` (or the configured `InMemoryToursFile`):
     ```json
     [{ "Id": "seed", "GUID": "seed", "Name": "seed", "AccessCodeMD5": "WrongMd5", "IsVersion": false }]
     ```
   - Restart the server

### Reproduce the self-perpetuating cycle

2. Make a `POST /api/Tour/add/anycode` with **no Authorization header** (or an expired/empty bearer token) and a minimal tour body:
   ```
   POST /api/Tour/add/testcode
   Content-Type: application/json

   { "Name": "repro tour" }
   ```
3. The server will:
   - Parse auth as anonymous → `AccessCodeMD5 = "WrongMd5"`
   - Find the seed tour (step 1) → `allowed = true`
   - Write `tourJson.AccessCodeMD5 = "WrongMd5"` → new "WrongMd5" tour stored

4. Repeat step 2 to see the count grow. Each new tour also has `AccessCodeMD5 = "WrongMd5"`.

### Reproduce via a UI session with a cleared token

1. Log in normally with a valid access code and verify you can see your tours.
2. Programmatically clear the token from localStorage (or call `ClearToken()` on the `TCDataService`) while leaving the UI page open.
3. Before the app re-authenticates, trigger a "Clone Tour" or "Add Tour" action.
4. The client sends the request with an empty bearer token.
5. If a "WrongMd5" tour already exists in storage (see precondition), a second one is created.

---

## Fix Recommendations

| Location | Fix |
|----------|-----|
| `AuthHelper.GetAuthData()` | Instead of silently substituting "WrongMd5", reject the request (throw `HttpException.NotAuthenticated`) or leave `AccessCodeMD5` empty and propagate the empty state explicitly |
| `TourController.AddTour()` | Validate that `authData.AccessCodeMD5` is a plausible MD5 (32 hex chars) before writing it to a tour |
| `ADHelpers.AccessCodeMD5s()` | Fix the inverted condition: when `IsNullOrWhiteSpace`, return `Enumerable.Empty<string>()` instead of wrapping the empty value |
| `TourVersions.RestoreTour()` | Pass the real access code (from `engine.Auth.AccessCodeMD5` or a stored code) instead of a random GUID; for master users the code in the URL is used directly via `CreateMD5`, so a random GUID creates an inaccessible tour |
| `UpdateTour` (PATCH) | Re-validate (or preserve from stored record) `AccessCodeMD5` instead of blindly trusting the request body |

---

## Relevant Code Locations

| File | Lines | Notes |
|------|-------|-------|
| `TCBlazor/Server/Auth/AuthHelper.cs` | 36–39 | Sentinel assignment |
| `TCBlazor/Server/Controllers/TourController.cs` | 167–191 | `AddTour` — non-master path |
| `TCBlazor/Server/Controllers/TourController.cs` | 200–235 | `UpdateTour` — no AccessCodeMD5 sanitisation |
| `TCBlazor/Server/Controllers/TourController.cs` | 489–523 | `TourStorageUtilities_LoadAllTours` — predicate filters by tainted MD5 |
| `TCalcCore/Auth/AuthData.cs` | 23–27 | `AccessCodeMD5s()` — inverted null-check |
| `TCBlazor/Client/Components/TourVersions.razor` | 77 | `RestoreTour` passes random GUID as code |
