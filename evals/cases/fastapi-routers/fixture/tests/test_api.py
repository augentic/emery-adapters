def test_a_request_without_a_token_is_unauthorised(client):
    response = client.get("/api/v1/orders", headers={"authorization": ""})
    assert response.status_code == 401


def test_health_reports_each_dependency(client):
    response = client.get("/health")
    assert response.status_code in (200, 503)
    assert set(response.json()["checks"]) == {"database", "cache"}


def test_an_unknown_order_is_404(client):
    response = client.get("/api/v1/orders/missing")
    assert response.status_code == 404
    assert response.json()["error"] == "not_found"


def test_placing_an_order_with_a_repeated_sku_is_422(client):
    body = {
        "customer_id": "c-1",
        "lines": [{"sku": "A-1", "quantity": 1}, {"sku": "A-1", "quantity": 2}],
        "ship_to": {"name": "Ada", "line1": "1 Queen St", "city": "Auckland", "postcode": "1010"},
    }
    response = client.post("/api/v1/orders", json=body)
    assert response.status_code == 422


def test_every_response_carries_server_timing(client):
    response = client.get("/health/ready")
    assert response.headers["server-timing"].startswith("app;dur=")
