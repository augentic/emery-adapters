import pytest
from fastapi.testclient import TestClient

from app.main import app
from app.settings import settings


@pytest.fixture
def client() -> TestClient:
    settings.api_tokens = ["test-token"]
    return TestClient(app, headers={"authorization": "Bearer test-token"})
