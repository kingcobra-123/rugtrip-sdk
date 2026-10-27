import json
from pathlib import Path

import pytest

FIXTURES = Path(__file__).resolve().parents[2] / "fixtures"


def load(name: str):
    return json.loads((FIXTURES / name).read_text())


def load_text(name: str) -> str:
    return (FIXTURES / name).read_text()


@pytest.fixture(autouse=True)
def _no_env_url(monkeypatch, request):
    if "live" not in request.node.nodeid:
        monkeypatch.delenv("RUGTRIP_API_URL", raising=False)
