//! What the SDK's lookups read of Python.
//!
//! One [`Dialect`], data rather than code, passed to each lookup that reads
//! one.

use emery_sdk::survey::route::Spelling;
use emery_sdk::survey::{ClassSyntax, Dialect};

pub(super) static DIALECT: Dialect = Dialect {
    self_name: "self",
    generic_stems: &[
        "__init__",
        "__main__",
        "main",
        "app",
        "views",
        "urls",
        "routes",
        "api",
        "handlers",
        "endpoints",
        "tasks",
    ],
    structural: &[
        "map",
        "filter",
        "sorted",
        "sort",
        "reduce",
        "partial",
        "wraps",
        "run",
        "run_until_complete",
        "create_task",
        "gather",
        "ensure_future",
        "run_in_executor",
        "to_thread",
        "register_blueprint",
        "include_router",
        "add_middleware",
        "mount",
        "setdefault",
        "Depends",
        "Security",
        "raises",
        "fixture",
        "parametrize",
        "field",
        "Field",
        "Column",
        "mapped_column",
        "relationship",
    ],
    listeners: &[],
    lifecycle_events: &[],
    // `patch` by its spelling whole: the same name on an application, a
    // router, or an HTTP client is a verb.
    mocking: &["patch", "mock.patch", "unittest.mock.patch", "mocker.patch"],
    // Django's admin site among them: it serves what it registers on the
    // source's behalf, by call or by decorator.
    lifecycle: &[
        "signal.signal",
        "atexit.register",
        "add_signal_handler",
        "on_event",
        "add_event_handler",
        "add_exception_handler",
        "register_error_handler",
        "teardown_appcontext",
        "teardown_request",
        "lifespan",
        "site.register",
        "admin.register",
        "admin.action",
        "admin.display",
    ],
    decorator_noise: &[
        "dataclass",
        "define",
        "frozen",
        "property",
        "setter",
        "getter",
        "deleter",
        "staticmethod",
        "classmethod",
        "cached_property",
        "lru_cache",
        "cache",
        "wraps",
        "overload",
        "abstractmethod",
        "override",
        "final",
        "contextmanager",
        "asynccontextmanager",
        "login_required",
        "permission_required",
        "csrf_exempt",
        "require_http_methods",
        "require_POST",
        "require_GET",
        "atomic",
        "retry",
        "validator",
        "field_validator",
        "model_validator",
        "root_validator",
        "computed_field",
        "total_ordering",
        "unique",
        "fixture",
        "parametrize",
        "mark",
        "skip",
        "skipif",
    ],
    decorator_noise_prefixes: &[],
    decorator_hooks: &[
        "connect",
        "receiver",
        "listens_for",
        "exception_handler",
        "errorhandler",
        "error_handler",
        "before_request",
        "after_request",
        "before_first_request",
        "middleware",
        "context_processor",
        "template_filter",
        "url_value_preprocessor",
        "url_defaults",
    ],
    hook_keywords: &[
        "lifespan",
        "on_startup",
        "on_shutdown",
        "exception_handlers",
        "default_factory",
        "default",
        "key",
        "callback",
        "dependencies",
        "middleware",
        "result_callback",
    ],
    type_imports_reach: false,
    openers: &['[', '{', '('],
    comment_prefixes: &["#"],
    globals: &["open"],
    options: &["add_argument", "add_option", "option", "argument", "Option", "Argument"],
    manifests: &["pyproject.toml", "setup.cfg"],
    barrels: &["__init__"],
    constructs: None,
    env_object: None,
    enum_bases: &["Enum", "Flag"],
    class_syntax: ClassSyntax::INDENTED,
    described: "a test's docstring or name under its class's",
    // A pattern spells a wildcard, a brace, a `<converter>`, or a regex.
    route: Spelling {
        pattern: &['*', '{', '(', '[', '<', '?', '\\'],
        param_name,
    },
};

// `<int:pk>` and `(?P<pk>\d+)` are `pk`; `{id}` and `:id` are `id`; any
// other segment is as written.
fn param_name(segment: &str) -> &str {
    if let Some(inner) = segment.strip_prefix('{').and_then(|rest| rest.strip_suffix('}')) {
        return inner.split_once(':').map_or(inner, |(name, _)| name);
    }
    if let Some(inner) = segment.strip_prefix('<').and_then(|rest| rest.strip_suffix('>')) {
        return inner.rsplit_once(':').map_or(inner, |(_, name)| name);
    }
    if let Some(start) = segment.find("(?P<") {
        let rest = &segment[start + 4..];
        return rest.split_once('>').map_or(rest, |(name, _)| name);
    }
    segment.trim_start_matches(':')
}
