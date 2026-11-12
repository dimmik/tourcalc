// Can a Worker reach MongoDB Atlas? `GET /` connects with the official driver and asks
// `hello` - answered before any login - and says how far it got and how long each step took.
// `GET /tcp` tries a bare TLS socket to one member, to tell the driver's trouble from the
// network's.
import { MongoClient } from "mongodb";
import { connect } from "cloudflare:sockets";

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (url.pathname === "/tcp") return tcp(url.searchParams.get("host"));
    if (url.pathname !== "/") return new Response("not here", { status: 404 });
    const steps = [];
    const t0 = Date.now();
    const client = new MongoClient(env.MONGO_URL);
    try {
      await client.connect();
      steps.push(["connect", Date.now() - t0]);
      const hello = await client.db("admin").command({ hello: 1 });
      steps.push(["hello", Date.now() - t0]);
      return Response.json({
        ok: true,
        steps,
        setName: hello.setName,
        maxWireVersion: hello.maxWireVersion,
        isWritablePrimary: hello.isWritablePrimary,
        hosts: hello.hosts,
      });
    } catch (e) {
      return Response.json({ ok: false, steps, after: Date.now() - t0, error: String(e), cause: String(e && e.cause || "") }, { status: 502 });
    } finally {
      await client.close().catch(() => {});
    }
  },
};

async function tcp(host) {
  const t0 = Date.now();
  try {
    const socket = connect({ hostname: host, port: 27017 }, { secureTransport: "on" });
    await socket.opened;
    const opened = Date.now() - t0;
    await socket.close();
    return Response.json({ ok: true, host, opened });
  } catch (e) {
    return Response.json({ ok: false, host, after: Date.now() - t0, error: String(e) }, { status: 502 });
  }
}
