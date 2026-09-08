# Tourcalc

Keeps track of who paid for what on a trip, and works out who owes whom at the end —
with as few payments as possible.

People have weights, so a child can count for half an adult; somebody can pay on behalf
of others and settle for the whole family; expenses can be shared by everyone or by a
few; and a tour can be kept in several currencies.

Self-hosted: one container, one config file.

## Four ways to use it

All of them are the same data and the same arithmetic — they differ only in how much
room they take.

| | |
|---|---|
| **Full** | The default web interface. Cards, avatars, charts. |
| **Mini** | The same thing drawn one line per person and per expense — for a small screen, an old phone, or a terminal browser like carbonyl. Switch in the header. |
| **Text** (`/t`) | Server-rendered HTML with no JavaScript at all, for lynx, w3m and friends. A known text browser is sent there automatically; everyone else gets a link from the `<noscript>` block. |
| **Telegram bot** | Run a trip from a group chat: add people, record expenses, ask who owes whom. Optional, off by default. |

`Classic` is the original interface, still there behind the same switch.

## Quick start

```
docker run -d -p 127.0.0.1:8080:80 \
  --env-file ./tourcalc.env \
  --name tourcalc \
  ghcr.io/dimmik/tourcalc:blazor-latest
```

(`podman run` works the same way.) The container listens on port 80.

The smallest `tourcalc.env` that does something useful:

```
MasterKey=change-me
StorageType=InMemory
AuthPrivateECDSAKey=rczPGwhweUcW9JjQPLoV/xHE9/ennHbwpAUpwkBmbHE=
```

**Generate your own `AuthPrivateECDSAKey`** — it signs the login tokens, and anybody who
knows yours can mint them:

```
openssl rand -base64 32
```

With MongoDB instead, so the data survives a restart:

```
MasterKey=change-me
StorageType=MongoDb
MongoDbUrl=your-cluster.mongodb.net
MongoDbUsername=mongo
MongoDbPassword=...
AuthPrivateECDSAKey=...
```

## Logging in

There are no user accounts. A **tour belongs to an access code**, and anyone who knows
the code sees the tours filed under it — so a code is what you share with the people on
the trip.

- On the login page, type the access code. Any code you like; the first tour under it
  has to be created by an admin.
- To log in as admin, type `admin:` followed by your `MasterKey` — for example
  `admin:change-me`.

Admin is needed to create the first tour under a new code and to delete the last one;
everything else is open to whoever has the code.

Sharing a tour is easier than dictating a code: the app produces a link that logs the
reader in and opens the tour.

## Configuration

Everything is read from environment variables (or `appsettings.json`). Nothing below is
required except where it says so.

### Storage

| Setting | Default | |
|---|---|---|
| `StorageType` | `InMemory` | `InMemory` or `MongoDb`. Anything else is treated as `InMemory`. |
| `InMemoryFileName` | `inmemory-tours.json` | Seed data for `InMemory`. |
| `MongoDbUrl`, `MongoDbUsername`, `MongoDbPassword` | — | Required for `MongoDb`. |

**`InMemory` keeps nothing.** It reads the seed file at startup and holds everything in
RAM: restart the container and the trips are gone. It is meant for trying the app out,
not for a real trip.

### Access

| Setting | Default | |
|---|---|---|
| `AuthPrivateECDSAKey` | — | **Set this.** Base64, 32 bytes, signs the login tokens. |
| `MasterKey` | — | The admin password. |
| `TokenValidTimeInMinutes` | 259200 (180 days) | How long a login lasts. |
| `MaxCountOfToursPerCode` | `-1` | Caps how many tours one access code may hold; `-1` is no limit. |
| `AnonymousIsMaster` | `false` | Everyone becomes admin. **Development only.** |

### Telegram bot

Off unless a token is set. See [the section below](#telegram-bot) for what it does.

| Setting | Default | |
|---|---|---|
| `TelegramBot_Enabled` | `true` | The off switch, leaving the rest of the settings alone. |
| `TelegramBot_Token` | — | From [@BotFather](https://t.me/BotFather). No token, no bot. |
| `TelegramBot_Mode` | `off` | `webhook` in production, `polling` for local work. |
| `TelegramBot_WebhookSecret` | — | Required for `webhook`. `A-Z a-z 0-9 _ -`, up to 256 characters. |
| `TelegramBot_PublicBaseUrl` | — | Where your installation is reachable from outside. **No default on purpose:** a guess would be somebody else's address. |
| `TelegramBot_AllowedChats` | empty | Chat ids allowed to use the bot, `;`-separated. Empty means any chat. |

### Text interface

| Setting | Default | |
|---|---|---|
| `TextBrowserRedirectEnabled` | `true` | Send known text browsers to `/t`. |
| `TextBrowserAgents` | `lynx;w3m;links` | Matched as case-insensitive substrings of the user agent. |

Turning the redirect off does not remove `/t` — it is still there to be typed, and the
`<noscript>` block still offers it to any browser that cannot run the app.

### Push notifications (optional)

| Setting | |
|---|---|
| `PushNotificationPublicKey`, `PushNotificationPrivateKey`, `PushNotificationMailto` | VAPID keys for browser notifications. |

### Odds and ends

| Setting | Default | |
|---|---|---|
| `TourVersioning` | `true` | Keep a version of a tour on every change. |
| `TourVersionEditable` | `false` | Allow editing an old version. |
| `ReturnVersionsInAllTours` | `false` | Include versions in the tour list. |
| `DoRedirectToDomain`, `RedirectDomain` | `false` | Redirect to a canonical domain. |
| `DoWakeup`, `WakeupUrl`, `WakeupCode`, `WaketimePreDelayInMin`, `WaketimePostDelayInMin` | | Pings itself, for hosting that puts an idle app to sleep. |

## Telegram bot

Runs a trip from a chat, so nobody has to leave Telegram to write down an expense.

**The bot speaks Russian.** The rest of the app is in English; the bot was written for a
Russian-speaking chat and its commands, buttons and answers have not been translated.

```
/newtrip Черногория      start a trip; you are on it
[ Я еду ]                others add themselves by tapping
[ Я везу ещё кого-то ]   somebody with no Telegram of their own, and their share
/add Маша 50             the same, typed
/spend 1200 такси        an expense; you paid, split between everyone
   [ платил не я ] [ не на всех ] [ категория ] [ 🗑 ]
/spendings               past expenses, to fix one
/balance                 who owes what
/settle                  who pays whom, with a button to record it
/trips   /use            several trips in one chat
/link                    open it in a browser
```

### Setting it up

1. Get a token from [@BotFather](https://t.me/BotFather).
2. Put it in `TelegramBot_Token`, set `TelegramBot_Mode=webhook`, invent a
   `TelegramBot_WebhookSecret`, and set `TelegramBot_PublicBaseUrl` to your public
   https address.
3. Restart. The bot registers its own webhook and publishes its own command menu —
   there is nothing to do by hand.
4. Add the bot to the chat.

The bot's privacy mode can stay **on** (the default): it only ever sees commands, taps
on its own buttons, and replies to its own questions — never the rest of the
conversation.

For working on the bot locally, `TelegramBot_Mode=polling` needs no public address and
no tunnel. A Mini App does, since Telegram opens those over https only; a quick
[cloudflared](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/do-more-with-tunnels/trycloudflare/)
tunnel is enough.

## Building from source

Needs the .NET 8 SDK.

```
dotnet build Tourcalc.sln
dotnet test TCalcTests/TCalcTests.csproj
dotnet run --project TCBlazor/Server
```

Set `ASPNETCORE_ENVIRONMENT=Development` to run against `appsettings.Development.json`,
which uses in-memory storage.

To build the image yourself:

```
docker build -f tourcalc.blazor.docker -t tourcalc .
```

### What is where

| | |
|---|---|
| `TCalcCore` | The domain and the arithmetic — who owes whom, and the smallest set of payments that settles it. Shared by everything else. |
| `TCalcStorage` | Storage providers: in-memory and MongoDB. |
| `TCBlazor/Client` | The Blazor WebAssembly app: Full, Mini and Classic interfaces. |
| `TCBlazor/Server` | The API, the no-JavaScript text pages, and the Telegram bot. |
| `TCalcTests` | Tests. |
| `TourCalcWebApp` | The older React client. Still builds its own image; no longer where the work happens. |

The bot's own logic knows nothing about Telegram — it takes a message and returns a
reply — so almost all of it is tested without a token, a network, or a bot.

## Images

| Tag | Built from |
|---|---|
| `ghcr.io/dimmik/tourcalc:blazor-latest` | branch `prod` |
| `ghcr.io/dimmik/tourcalc:beta-latest` | branches `beta/**` |
| `ghcr.io/dimmik/tourcalc:react-latest` | the older React client |
