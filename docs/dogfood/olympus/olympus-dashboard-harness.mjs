import { createRequire } from "node:module";
import path from "node:path";
import { pathToFileURL } from "node:url";

const olympusRoot = process.env.OLYMPUS_ORCHESTRATOR_ROOT;
const dbPath = process.env.DB_PATH;
const secret = process.env.DASHBOARD_SECRET;
const portValue = process.env.PORT;

if (!olympusRoot) {
  throw new Error("OLYMPUS_ORCHESTRATOR_ROOT is required");
}
if (!dbPath) {
  throw new Error("DB_PATH is required");
}
if (!secret) {
  throw new Error("DASHBOARD_SECRET is required");
}
if (!portValue) {
  throw new Error("PORT is required");
}
const port = Number.parseInt(portValue, 10);
if (!Number.isInteger(port) || port < 1 || port > 65535) {
  throw new Error(`Invalid PORT: ${portValue}`);
}

const fromOlympus = async (relativePath) =>
  import(pathToFileURL(path.join(olympusRoot, relativePath)).href);

const requireFromOlympus = createRequire(path.join(olympusRoot, "package.json"));
const { serve } = await import(requireFromOlympus.resolve("@hono/node-server"));
const { Hono } = await import(requireFromOlympus.resolve("hono"));
// Dogfood shim: Olympus has no dashboard-only export, so the receipt pins the
// target commit that owns these internal module paths.
const { createDatabase } = await fromOlympus("src/db.ts");
const { createDashboardRouter } = await fromOlympus("src/dashboard/route.tsx");

const db = createDatabase(dbPath);
const app = new Hono();

app.get("/", (c) => c.redirect("/dashboard"));
app.get("/health", (c) => c.json({ status: "ok", app: "olympus-dashboard-harness" }));
app.route(
  "/dashboard",
  createDashboardRouter({
    db,
    secret,
  }),
);

const server = serve({ fetch: app.fetch, port }, (info) => {
  console.log(`Olympus dashboard harness listening on http://127.0.0.1:${info.port}`);
});

function shutdown() {
  server.close();
  db.close();
}

process.on("SIGINT", shutdown);
process.on("SIGTERM", shutdown);
