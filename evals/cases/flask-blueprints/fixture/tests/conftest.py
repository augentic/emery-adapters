import pytest

from shop import create_app


@pytest.fixture()
def client():
    app = create_app()
    app.config.update(TESTING=True, WEBHOOK_URL="")
    with app.test_client() as client:
        yield client
