from flask import Blueprint, jsonify, request

from ..auth.routes import require_token
from . import service

bp = Blueprint("orders", __name__)


@bp.route("/", methods=["POST"])
@require_token
def place_order():
    order = service.place(request.get_json(silent=True) or {})
    return jsonify(order.as_dict()), 201


@bp.get("/")
@require_token
def list_orders():
    customer = request.args.get("customer", "")
    if not customer:
        raise service.OrderError("customer is required", code="customer_required")
    limit = request.args.get("limit", default=10, type=int)
    return jsonify([order.as_dict() for order in service.recent(customer, limit)])


@bp.get("/<int:order_id>")
@require_token
def read_order(order_id: int):
    return jsonify(service.read(order_id).as_dict())


@bp.post("/<int:order_id>/cancel")
@require_token
def cancel_order(order_id: int):
    return jsonify(service.cancel(order_id).as_dict())
