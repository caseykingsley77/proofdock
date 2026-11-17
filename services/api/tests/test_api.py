import pytest

from app.base58 import b58encode
from app.main import app, rpc_dependency
from app.rpc import RpcError
from conftest import ACCOUNT_BYTES, CLIENT_BYTES


class FakeRpc:
    def __init__(self, rows=None, account=None, fail=False):
        self.rows = rows or []
        self.account = account
        self.fail = fail

    async def list_client_escrows(self, program_id, client_key):
        if self.fail:
            raise RpcError("offline")
        return self.rows

    async def get_escrow(self, program_id, address):
        if self.fail:
            raise RpcError("offline")
        return self.account


def test_health(test_client):
    response = test_client.get("/health")
    assert response.status_code == 200
    assert response.json() == {"status": "ok", "service": "proofdock-api"}


def test_list_escrows_returns_decoded_records(test_client, account_data):
    app.dependency_overrides[rpc_dependency] = lambda: FakeRpc(rows=[
        (b58encode(ACCOUNT_BYTES), account_data)
    ])
    try:
        response = test_client.get("/v1/escrows", params={"client": b58encode(CLIENT_BYTES)})
        assert response.status_code == 200
        rows = response.json()
        assert len(rows) == 1
        assert rows[0]["milestone_id"] == 77
        assert rows[0]["amount_lamports"] == 5_000_000
    finally:
        app.dependency_overrides.clear()


def test_invalid_client_address_is_rejected(test_client):
    response = test_client.get("/v1/escrows", params={"client": "not-a-wallet"})
    assert response.status_code == 422


def test_get_not_found(test_client):
    app.dependency_overrides[rpc_dependency] = lambda: FakeRpc()
    try:
        response = test_client.get(f"/v1/escrows/{b58encode(ACCOUNT_BYTES)}")
        assert response.status_code == 404
    finally:
        app.dependency_overrides.clear()


def test_upstream_error_returns_502(test_client):
    app.dependency_overrides[rpc_dependency] = lambda: FakeRpc(fail=True)
    try:
        response = test_client.get("/v1/escrows", params={"client": b58encode(CLIENT_BYTES)})
        assert response.status_code == 502
    finally:
        app.dependency_overrides.clear()


def test_get_escrow_account(test_client, account_data):
    app.dependency_overrides[rpc_dependency] = lambda: FakeRpc(account=account_data)
    try:
        response = test_client.get(f"/v1/escrows/{b58encode(ACCOUNT_BYTES)}")
        assert response.status_code == 200
        assert response.json()["status"] == "funded"
    finally:
        app.dependency_overrides.clear()
