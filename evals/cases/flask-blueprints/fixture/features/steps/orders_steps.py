from behave import given, then, when

from shop import create_app
from shop.orders.repository import repository


@given("a signed-in customer")
def signed_in(context):
    context.client = create_app().test_client()
    response = context.client.post("/auth/login", json={"user": "ada", "password": "correct-horse"})
    context.headers = {"Authorization": f"Bearer {response.get_json()['token']}"}


@when("they place an order with one line of {quantity:d} at {unit:d} cents")
def place(context, quantity, unit):
    context.response = context.client.post(
        "/orders/",
        json={"customer": "cus_1", "lines": [{"sku": "A", "quantity": quantity, "unit_cents": unit}]},
        headers=context.headers,
    )


@then("the order is pending with a total of {total:d} cents")
def pending(context, total):
    body = context.response.get_json()
    assert body["status"] == "pending"
    assert body["total_cents"] == total


@given("a paid order")
def paid(context):
    signed_in(context)
    place(context, 1, 100)
    context.order_id = context.response.get_json()["id"]
    repository.set_status(context.order_id, "paid")


@given("a shipped order")
def shipped(context):
    paid(context)
    repository.set_status(context.order_id, "shipped")


@when("the customer cancels it")
def cancel(context):
    context.response = context.client.post(f"/orders/{context.order_id}/cancel", headers=context.headers)


@then("the order is cancelled")
def cancelled(context):
    assert context.response.get_json()["status"] == "cancelled"


@then("the answer is {status:d} {code}")
def refused(context, status, code):
    assert context.response.status_code == status
    assert context.response.get_json()["code"] == code
