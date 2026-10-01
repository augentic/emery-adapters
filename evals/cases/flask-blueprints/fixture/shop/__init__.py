import logging
import time

from flask import Flask, g, jsonify, request
from werkzeug.exceptions import HTTPException

from .auth.routes import bp as auth_bp
from .cache import cache
from .config import Config
from .orders.routes import bp as orders_bp
from .orders.service import OrderError

log = logging.getLogger("shop")


def health() -> tuple[dict, int]:
    ready = cache.client is not None
    return {"status": "ok" if ready else "degraded", "cache": ready}, 200 if ready else 503


def create_app(config: type = Config) -> Flask:
    app = Flask(__name__)
    app.config.from_object(config)
    cache.init_app(app)

    app.register_blueprint(orders_bp, url_prefix="/orders")
    app.register_blueprint(auth_bp)
    app.add_url_rule("/health", view_func=health)

    @app.before_request
    def started() -> None:
        g.started = time.perf_counter()

    @app.after_request
    def timed(response):
        elapsed_ms = (time.perf_counter() - g.get("started", time.perf_counter())) * 1000
        response.headers["X-Elapsed-Ms"] = f"{elapsed_ms:.1f}"
        log.info("%s %s -> %s in %.1fms", request.method, request.path, response.status_code, elapsed_ms)
        return response

    @app.errorhandler(OrderError)
    def order_error(error: OrderError):
        return jsonify({"error": str(error), "code": error.code}), error.status

    @app.errorhandler(HTTPException)
    def http_error(error: HTTPException):
        return jsonify({"error": error.description, "code": error.name.lower().replace(" ", "_")}), error.code

    return app
