# Python source survey

This prompt runs once per bound `python` source, before anything is extracted. The caller has parsed the tree already: the message lists every production module, says what the manifest names and whether an entry runs anything, lists every call that hands a function or a class to something a package provides, every function, method, and class under a package's decorator, what the entry modules export, and the packages imported — and lays the modules into the message whole, every line numbered, as far as they fit. Your job: from those facts, name each surface the source exposes — each one thing a caller outside the process does through it — anchor it at the lines where the code registers or declares it, and give it the stem its requirements lead with. You follow no import, group no module, and extract nothing. The caller derives the rest from the anchors you name: the stem the code spells at each, where it spells one, the id that tells a surface from the others under its stem, the modules each surface reaches, the lines a requirement may anchor at, the seams the [extract prompt](extract.md) mines, whose answers the engine reconciles into the specification — see [From sources to a spec](reconciliation.md).

Everything this call needs is in this prompt and the message. The references `read_doc` offers are written for the extract call; load one only where a rule here links it — [claims.md](claims.md) for the `path` grammar — and never to decide what a surface is.

## Inputs

- **The manifest** — what `pyproject.toml` or `setup.cfg` names: the package, and the console scripts it installs with the `module:function` each runs. Absent when the tree has neither.
- **The bootstrap** — the entry that runs something, when the tree has one: the first module a console script names, or the conventional entry the tree holds (`manage.py`, a package's `__main__.py`, `main.py`, `app.py`, `wsgi.py`, `asgi.py`), that a script's function starts, that runs under an `if __name__ == "__main__":` guard, or that constructs the application at module level (`app = FastAPI()`, `app = create_app()`). The caller names the bootstrap `start` itself and owns everything it does — settings read, clients constructed, routers included, blueprints registered, consumers subscribed, jobs scheduled, signal handlers, shutdown — so `start` is never a surface you list, and no surface you list leads with the stem `start`. When no entry runs, the message says so: the tree is a library, or an application a framework enters by a setting's name.
- **Registrations** — every call, outside any handler, that hands a function or a class to something a package provides, with the literal that led it, the package, and the type the receiver is constructed or typed as, at its lines (`` `shop/urls.py#L8` — `path` led by `"orders/"` handed a function, through `django` ``; `` `jobs/scheduler.py#L12` — `scheduler.add_job` led by `"nightly"` handed a function, through `apscheduler` as `BackgroundScheduler` ``). This is where a framework, a queue, a scheduler, or a CLI is told what to run: a URL pattern, a view handed by `as_view()`, a view set registered on a router, a job added to a scheduler, a consumer subscribed. A hook on a surface already registered — a signal receiver connected, an error handler, a startup event — is that surface's or the bootstrap's behaviour, not a surface of its own.
- **Decorators** — every decorator a package provides that registers what it decorates — one that only shapes it (`@dataclass`, `@property`, `@lru_cache`, `@login_required`) is not listed — on a module-level function, a method, or a class, with its literal, at its lines (`` `api/orders.py#L20-L31` — `@router.get("/{order_id}")` on `get_order`, through `fastapi` ``; `` `cli.py#L14-L24` — `@cli.command("import")` on `import_file`, through `click` ``). A verb or `route` decorator on a function is a route; a `command` decorator is a command; a `task` or `shared_task` decorator is a task; a scheduler's decorator is a job; a class decorator carries a prefix or registers a view set.
- **Exports** — what the entry modules export: the modules the manifest's scripts name, and the ones no other module imports, each export with its kind and lines — and what a package's `__init__.py` among them re-exports, listed at the module that declares it, which is where such a surface is anchored. In a tree with no bootstrap these are where a caller enters: a library's public functions and classes. In a tree with a bootstrap they are not: a module nothing imports and no script runs — a settings variant, a constants file, a script run by hand — is dead to the process and goes under `unreached`, unless a framework loads it by a setting's name or a directory convention, in which case the surfaces it registers are named at their registrations.
- **Packages** — the modules of packages the tree imports, as written, with the names bound to each and the modules binding them. The package a registration goes through tells what kind of surface it is: a web framework registers routes, a CLI framework registers commands, a task queue registers tasks, a scheduler registers jobs, a broker client registers consumers.
- **The modules** — every production module, listed by path; an `anchor` names one of these and nothing else. The leading ones are laid out whole with every line numbered: cite `#L<n>` from the numbers shown rather than reading them again; a module listed but not laid is read from `$SOURCE_DIR` only when the facts above leave its surfaces unclear.
- **`$SOURCE_DIR`** — read-only view of the bound source root, the whole tree, for a module the message lists rather than lays out.

Nothing outside the bound source is reachable; writes back into `$SOURCE_DIR` are denied.

## What a surface is

A surface is one thing the source does for a caller outside the process, reached at a boundary the source itself declares:

- a **route** — `POST /orders`, `GET /orders/{order_id}` — registered by a verb or `route` decorator, a URL pattern handed a view, a view set registered on a router, or a rule added by hand;
- a **command** — a CLI verb under a `command` decorator, a subcommand of a group, a management command a module under `management/commands/` declares, or a console script the manifest installs that runs a module of its own, which an operator runs by the script's name: one surface, anchored at the function the script names;
- a **job** — a scheduled task, a queue task or worker, a topic or event consumer, a periodic beat entry;
- an **exported API** — what a library's entry module makes importable: one surface per public function or class the Exports list names, less an exception type (a class extending `Exception` or a name ending in `Error`), which is what a surface raises, and less a function whose body registers surfaces, which hosts them. A class that declares data alone — an `Enum`, a `TypedDict`, a `NamedTuple`, a `Protocol`, or a class of fields and properties with no method of its own (a dataclass, a model, a settings class) — is a type the caller copies from the code, never a surface: the list leaves it out, and so do you.

The grain is a reviewer's: one surface per route, per command, per task, per export. Several surfaces may anchor in one module — a router module declaring four routes is four surfaces at four anchors in one file, a `urls.py` with three patterns is three. A module no surface is registered or declared in — a service, a repository, a model, a schema, a settings module, a client wrapper — is no surface: the caller reaches it from the surfaces whose anchors import it, and mines it through them.

## Anchors

A surface's `anchor` is where the code registers or declares it, in the claim `path` grammar of [claims.md](claims.md): the module, relative to `$SOURCE_DIR`, and the lines of the registration call, the decorated function from its first decorator to the end of its body, or the exported declaration — `shop/urls.py#L8`, `api/orders.py#L20-L31`, `ledger/money.py#L5-L48`. Cite the lines the message gives for the registration, the decorator, or the export; a module alone, with no lines, anchors a surface the whole file declares and nothing else does — a management command. The caller follows the imports the anchored lines reference to find what the surface reaches, so an anchor at the registration — where the view is named — reaches the view's module; one at an unrelated line reaches the wrong code.

## Stems

The stem is the first segment of every `requirement` and `criterion` id the surface's extraction answers, and the engine slices the build plan by it, so two runs over one tree must name the same stems. Derive each by this convention, lowercase kebab-case, and by nothing else. The caller applies the same convention to what the code spells at the anchor you name — a route's path under its router's prefix, its blueprint's mount, or the pattern that includes it; a decorator's or a registration's literal; a command module's file name — and where the code spells a stem there, the caller's reading replaces yours; your stem stands where it spells none (a route on `/`, a schedule, an export, a script), so it is on those that the convention below rests on you alone:

- a **route**'s stem is the first path segment that names a resource, past any prefix a router, a blueprint, or an including pattern adds, with `api`, `rest`, `internal`, and a version (`v1`, `v2`) skipped and a parameter (`{order_id}`, `<int:pk>`) never counted — the first, however specific a later segment is: `GET /api/v1/orders/{order_id}` → `orders`; `POST /customers` → `customers`; `GET /health/live` → `health`; `POST /auth/google/login` → `auth`, not `google`; `GET /users/<int:pk>/orders` → `users`; a route on a router that declares no prefix and is included under none, but is tagged — `APIRouter(tags=["login"])` — sits under its first tag whatever its path spells: `POST /password-recovery/{email}` on that router → `login`; a route whose path names no resource at all — `GET /`, `GET /api` — takes the name of its module, or of its directory when the module is `views`, `urls`, `routes`, `api`, or `__init__`: `routes/home.py` → `home`, `orders/views.py` → `orders`;
- a **command**'s stem is its literal's first word, or its function's name when the decorator spells none: `@cli.command("import")` → `import`; `@cli.command()` on `def reconcile` → `reconcile`; a subcommand of a group takes the group's name — `@orders.command("list")` under `@cli.group()` on `def orders` → `orders`; a management command takes its module's name: `management/commands/send_invoices.py` → `send-invoices`; a console script that runs a module of its own takes the script's name;
- a **task**'s, **job**'s, or **consumer**'s stem is the name its decorator or registration led with — the task name, the queue, the topic, the job id — as its first word, a dotted name by its first segment: `@app.task(name="invoices.send")` → `invoices`; `scheduler.add_job(nightly, "cron", hour=2)` led by a schedule rather than a name → the stem the handler's name gives, `nightly`; `@shared_task` with no literal → the function's name;
- a registration led by no literal takes the stem of the type its receiver is constructed or typed as, kebab-cased: a `KafkaConsumer` subscribed with a callback → `kafka-consumer`;
- an **exported API**'s stem is the export's name kebab-cased: `class OrderService` → `order-service`; `def parse_csv` → `parse-csv`.

Routes on one resource share one stem — `GET /orders`, `POST /orders`, `GET /orders/{order_id}` are three surfaces under `orders` — and the caller tells them apart by their names. Middleware applied to every request — a global dependency, an error handler, a request log, Django's `MIDDLEWARE` — is the bootstrap's behaviour under `start`, which the caller owns; a dependency on one route is that route's. So is what a package serves on the source's behalf with no handler the source writes — a docs UI under a `docs_url`, the admin site, a static directory, a metrics page: the source declares no surface there. Name nothing `start`.

## Unreached modules

Every production module is reached by some surface — through the imports its anchor's lines reference, the bootstrap's among them — or it is not. List under `unreached` each module the surfaces you name do not reach and the bootstrap does not: dead code, a script no entry and no console script runs, a module a framework loads by a setting's name rather than by import (an `INSTALLED_APPS` entry's `apps.py`, a settings variant), a module you cannot place. The caller checks the coverage: a module in neither set that the facts list a registration or declaration in comes back as a finding — name the surface registered there, anchored at the registration whose lines import it, or list it — while a module the facts say nothing of is taken as unreached without one. Never invent a surface to cover a module.

## Output

One JSON object matching the survey schema: `surfaces`, each with a `name`, an `anchor`, and a `stem`; and `unreached`, the modules no surface reaches, empty when every module is reached.

Rules:

- Every `anchor` names a module the message lists, relative to `$SOURCE_DIR`, with lines the file holds.
- Every `stem` is lowercase kebab-case, derived by the convention above.
- Every surface has a name, and no two surfaces share one; the name is what a caller does — `POST /orders`, `import command`, `send_invoice task`, `OrderService` — never a description of the code.
- No surface leads with `start` when the tree has a bootstrap.
- A module no surface is registered or declared in is not a surface, however central.
- An empty `surfaces` is the answer when the tree declares no boundary — nothing registers a route, a command, a task, or a consumer, and no entry exports anything a caller would call. Do not invent one; the caller mines such a tree as a library.

## Worked example

A `click` CLI with a task queue, bound as the source `ledger-cli`. The message says the manifest `pyproject.toml` names the package `ledger` and installs the console script `ledger`, which runs `ledger.cli:main`; the bootstrap is `ledger/cli.py`, the caller's `start`; and lists:

> - `ledger/cli.py#L14-L24` — `@cli.command("import")` on `import_file`, through `click`
> - `ledger/cli.py#L26-L38` — `@cli.command()` on `reconcile`, through `click`
> - `ledger/jobs/nightly.py#L30-L41` — `@scheduler.scheduled_job("cron", hour=2)` on `nightly`, through `apscheduler`
> - `ledger/workers/invoices.py#L12-L31` — `@app.task(name="invoices.send")` on `send_invoice`, through `celery`
> - `ledger/workers/invoices.py#L33-L36` — `@task_failure.connect` on `on_failure`, through `celery`
>
> Packages … `click` — `click` — in `ledger/cli.py`; `apscheduler.schedulers.background` — `BackgroundScheduler` — in `ledger/jobs/nightly.py`; `celery` — `Celery` — in `ledger/workers/celery.py`; `celery.signals` — `task_failure` — in `ledger/workers/invoices.py`; `psycopg` — `psycopg` — in `ledger/db.py` …

The modules are `ledger/__init__.py`, `ledger/cli.py`, `ledger/config.py`, `ledger/db.py`, `ledger/jobs/nightly.py`, `ledger/services/reconcile.py`, `ledger/workers/celery.py`, `ledger/workers/invoices.py`, and `scripts/seed.py`, which nothing imports and no console script runs.

Resulting survey answer:

```json
{
  "surfaces": [
    { "name": "import command", "anchor": "ledger/cli.py#L14-L24", "stem": "import" },
    { "name": "reconcile command", "anchor": "ledger/cli.py#L26-L38", "stem": "reconcile" },
    { "name": "nightly job", "anchor": "ledger/jobs/nightly.py#L30-L41", "stem": "nightly" },
    { "name": "send_invoice task", "anchor": "ledger/workers/invoices.py#L12-L31", "stem": "invoices" }
  ],
  "unreached": ["scripts/seed.py"]
}
```

Four surfaces, each at the decorated function the message lists, each stem by the convention: the commands' literal's first word and function's name, the cron job's function name since its literal is a schedule, the task's name by its first segment. `@task_failure.connect` is a hook on the task already registered, so it is the task's behaviour and no surface. `ledger/cli.py` is the bootstrap: the caller lists it as `start` and owns the connection it opens and the group it runs, so it is not listed here, and the two commands anchored inside it lead with their own stems. `ledger/__init__.py`, `ledger/config.py`, `ledger/db.py`, `ledger/services/reconcile.py`, and `ledger/workers/celery.py` are reached through the surfaces' anchors and the bootstrap, so they are named by none; `scripts/seed.py` is reached by nothing and is listed as unreached rather than given a surface.

## Anti-patterns

- **Naming `start`.** The bootstrap is the caller's. What it constructs, includes, and runs is `start`'s behaviour by the caller's rule; you name what it registers, not the bootstrap itself.
- **Naming a module.** A service, a repository, a model, a helper is not a surface; it is reached through the surfaces whose anchors import it.
- **Cutting by directory or layer.** `api/*` is not a surface, nor is `services/*`, nor a Django app as a whole. A surface is what a caller reaches, wherever the code sits.
- **Inventing a stem.** The stem is derived by the convention from the literal, the type, or the export — not chosen for elegance: `invoice-task` for a task named `invoices.send` is wrong; `invoices` is right.
- **Anchoring at a handler's body.** The anchor is the decorated function, the registration, or the export — where the code declares the surface — not a line inside the function that implements it.
- **Inventing a surface.** A module nothing reaches is listed under `unreached`; a tree that declares no boundary is answered `surfaces: []`.
- **Extracting.** This call names surfaces and nothing else; claims are the extract call's.

## Failure modes

| Condition | Action |
| --------- | ------ |
| The tree has one surface | Answer it alone. |
| A router or a `urls.py` registers many routes | One surface per route, all under the resource's stem; the caller tells them apart by name. |
| The tree is a subtree — a router without the application that includes it | Answer the surfaces the tree itself registers; no bootstrap means no `start`, and a prefix you cannot see is not guessed. |
| A Django project: `manage.py`, settings, `urls.py` modules, apps | `manage.py` is the bootstrap; one surface per URL pattern handed a view, anchored at the pattern, under the resource its path spells; a management command is one surface at its module; `admin.site.register` and signals register no surface. |
| A view set or class-based view is handed to a router or a pattern | One surface at the registration; the caller reads the class's verb methods or actions. |
| The tree declares no boundary | Answer `surfaces: []` and list nothing as unreached; the caller mines it as a library. |
| A module the facts list a registration or declaration in is reached by no surface you named and is not listed as unreached | The caller returns it as a finding: anchor the surface registered or declared there, or list it. |
| A framework loads modules by a setting's name — `INSTALLED_APPS`, a `ROOT_URLCONF` | Name the surfaces those modules register, anchored where each is registered; the modules that register none — an `apps.py`, a `models.py`, a settings variant — are unreached, not surfaces. |
| The answer names a module the tree lacks, a stem outside kebab-case, a surface twice, or a surface under `start` | The caller rejects it and asks again with the findings; correct the named surfaces. |
