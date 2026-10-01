import pytest


def login(client) -> dict:
    response = client.post("/auth/login", json={"user": "ada", "password": "correct-horse"})
    return {"Authorization": f"Bearer {response.get_json()['token']}"}


def test_placing_an_order_answers_201_with_the_total(client):
    response = client.post(
        "/orders/",
        json={"customer": "cus_1", "lines": [{"sku": "A", "quantity": 2, "unit_cents": 1200}]},
        headers=login(client),
    )
    assert response.status_code == 201
    assert response.get_json()["total_cents"] == 2400 + 850


def test_an_order_at_the_free_shipping_threshold_ships_free(client):
    response = client.post(
        "/orders/",
        json={"customer": "cus_1", "lines": [{"sku": "A", "quantity": 1, "unit_cents": 10_000}]},
        headers=login(client),
    )
    assert response.get_json()["shipping_cents"] == 0


def test_an_order_without_lines_is_rejected(client):
    response = client.post("/orders/", json={"customer": "cus_1", "lines": []}, headers=login(client))
    assert response.status_code == 400
    assert response.get_json()["code"] == "lines_required"


def test_a_request_without_a_token_is_401(client):
    assert client.get("/orders/1").status_code == 401


def test_cancelling_a_shipped_order_is_409(client):
    """A shipped order is past cancelling."""
    headers = login(client)
    placed = client.post(
        "/orders/", json={"customer": "cus_2", "lines": [{"sku": "A", "quantity": 1, "unit_cents": 100}]}, headers=headers
    ).get_json()
    from shop.orders.repository import repository

    repository.set_status(placed["id"], "shipped")
    assert client.post(f"/orders/{placed['id']}/cancel", headers=headers).status_code == 409


@pytest.mark.parametrize("password", ["short", ""])
def test_a_short_password_is_422(client, password):
    assert client.post("/auth/login", json={"user": "ada", "password": password}).status_code == 422


def test_health_reports_the_cache(client):
    assert client.get("/health").status_code in (200, 503)
