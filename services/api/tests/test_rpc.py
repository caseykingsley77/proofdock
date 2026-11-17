import base64

import httpx
import pytest

from app.base58 import b58encode
from app.codec import ESCROW_ACCOUNT_SIZE, CLIENT_MEMCMP_OFFSET
from app.rpc import RpcError, SolanaRpc
from conftest import CLIENT_BYTES, ACCOUNT_BYTES


@pytest.mark.asyncio
async def test_rpc_program_account_query_filters(rpc_account):
    client_key = b58encode(CLIENT_BYTES)
    account_key = b58encode(ACCOUNT_BYTES)
    program_id = rpc_account["owner"]

    def responder(request: httpx.Request) -> httpx.Response:
        body = __import__("json").loads(request.content)
        assert body["method"] == "getProgramAccounts"
        assert body["params"][0] == program_id
        assert body["params"][1]["filters"] == [
            {"dataSize": ESCROW_ACCOUNT_SIZE},
            {"memcmp": {"offset": CLIENT_MEMCMP_OFFSET, "bytes": client_key}},
        ]
        return httpx.Response(200, json={"jsonrpc": "2.0", "id": 1, "result": [
            {"pubkey": account_key, "account": rpc_account}
        ]})

    async with httpx.AsyncClient(transport=httpx.MockTransport(responder)) as http:
        rpc = SolanaRpc("https://rpc.invalid", http)
        accounts = await rpc.list_client_escrows(program_id, client_key)
    assert len(accounts) == 1
    assert accounts[0][0] == account_key
    assert len(accounts[0][1]) == ESCROW_ACCOUNT_SIZE


@pytest.mark.asyncio
async def test_rpc_unavailable_account_returns_none():
    def responder(request):
        return httpx.Response(200, json={"jsonrpc": "2.0", "id": 1, "result": {"value": None}})

    async with httpx.AsyncClient(transport=httpx.MockTransport(responder)) as http:
        rpc = SolanaRpc("https://rpc.invalid", http)
        assert await rpc.get_escrow("ignored", b58encode(ACCOUNT_BYTES)) is None


@pytest.mark.asyncio
async def test_rpc_rejects_malicious_owner(rpc_account):
    def responder(request):
        return httpx.Response(200, json={"jsonrpc": "2.0", "id": 1, "result": {
            "value": {**rpc_account, "owner": "attacker"}
        }})

    async with httpx.AsyncClient(transport=httpx.MockTransport(responder)) as http:
        rpc = SolanaRpc("https://rpc.invalid", http)
        assert await rpc.get_escrow(rpc_account["owner"], b58encode(ACCOUNT_BYTES)) is None


@pytest.mark.asyncio
async def test_rpc_rejects_upstream_error():
    def responder(request):
        return httpx.Response(200, json={"jsonrpc": "2.0", "id": 1, "error": {"code": -32601}})

    async with httpx.AsyncClient(transport=httpx.MockTransport(responder)) as http:
        with pytest.raises(RpcError):
            await SolanaRpc("https://rpc.invalid", http).call("notReal", [])


@pytest.mark.parametrize("encoded", ["?!!!", "", "YQ==!!"])
def test_rpc_rejects_bad_base64(encoded):
    with pytest.raises(RpcError):
        SolanaRpc.decode_base64_account({"data": [encoded, "base64"]})


def test_rpc_rejects_non_base64_encoding():
    with pytest.raises(RpcError):
        SolanaRpc.decode_base64_account({"data": [base64.b64encode(b"hi").decode(), "base58"]})
