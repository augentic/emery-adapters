# Component structure

How a TypeScript or JavaScript tree is organised, and how to follow a surface from its entry through it. The survey names the surface and the module a caller enters it at; this reference is how to read the rest of the tree from there.

## The manifest

`package.json` says what the tree exposes and how it is built:

- `bin` names the commands a caller runs, each with its entry module.
- `main`, `module`, and `exports` name what a library makes importable; each export path's target is an entry.
- `scripts` name what is run — `start`, `serve`, a worker — and, through the command each runs, its entry module.
- `dependencies` name the frameworks in play (`express`, `fastify`, `@nestjs/core`, `koa`, `hono`, `commander`, `yargs`, `bullmq`, `kafkajs`, `@azure/service-bus`), which decide what a route, a command, or a consumer looks like below.

`tsconfig.json` `paths` and `baseUrl` resolve the aliases imports use (`@app/users` → `src/users`); `rootDir` and `include` bound the compiled tree. Resolve every import you follow through them, relative to `$SOURCE_DIR`.

## Entries and what they reach

- **A route** enters where the framework registers it — `app.post("/users", handler)`, a `router.get`, a Nest `@Controller` method, a Fastify `route` call. The handler it names, or the controller method, is where behaviour starts.
- **A command** enters at its `bin` module or the `commander` / `yargs` registration that names it; its action function is where behaviour starts.
- **A job or consumer** enters at the registration — `cron.schedule`, a `setInterval` at module scope, a queue `Worker` or `consumer.run` handler; the callback is where behaviour starts.
- **An exported API** enters at the module `exports` names; each exported function or class is where a surface's behaviour starts.

From the entry, follow imports outward: the handler, the services it calls, the repositories and clients they use, the types they take and return. Under a dependency-injection container — Nest's `@Injectable()` providers, `inversify`, `tsyringe` — a constructor parameter names an interface or a token, and what runs is the provider the module's `providers` (or the container's `bind`) resolves it to: follow the binding, not the parameter type, and where a token is bound differently per environment, claim the binding the production module makes. Stop where the surface stops — a module the surface never reaches is another surface's, or nobody's.

## The entry layer

What sits between the caller and the handler shapes what the caller observes, so it is claimed for this surface wherever the surface passes through it:

- middleware the route mounts — authentication, CORS, rate limits, body parsing, validation (`zod`, `joi`, `class-validator` pipes);
- error mapping — an error class or code translated to an HTTP status or an exit code;
- parameter sourcing — which of path, query, header, and body a value comes from, and the coercion applied (`Number(req.query.limit)`).

Global middleware every route passes through is claimed under this surface's noun, for what it does to this surface's requests — never as a surface of its own.

## Async boundaries

`await` in sequence is sequential; `Promise.all` and `Promise.allSettled` are parallel; `Promise.race` is first-wins; a call without `await` completes after the response. Where the difference is observable — the order of side effects, what happens on partial failure, what a caller gets back before a publish completes — the requirement statement says which.
