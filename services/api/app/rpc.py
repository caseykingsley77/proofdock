"""Small, read-only, timeout-bounded Solana JSON-RPC client."""

import base64
import binascii
from typing import Any

import httpx

from .codec import CLIENT_MEMCMP_OFFSET, ESCROW_ACCOUNT_SIZE


class RpcError(Exception):
    """An RPC transport, protocol, or upstream application error."""


class SolanaRpc:
    def __init__(self, endpoint: str, client: httpx.AsyncClient | None = None):
        self.endpoint = endpoint
        self._client = client

    async def call(self, method: str, params: list[Any]) -> Any:
        payload = {"jsonrpc": "2.0", "id": 1, "method": method, "params": params}
        try:
            if self._client is None:
                async with httpx.AsyncClient(timeout=12.0) as client:
                    response = await client.post(self.endpoint, json=payload)
            else:
                response = await self._client.post(self.endpoint, json=payload)
            response.raise_for_status()
            result = response.json()
        except (httpx.HTTPError, ValueError) as exc:
            raise RpcError("Solana RPC request failed") from exc
        if not isinstance(result, dict) or result.get("error"):
            raise RpcError("Solana RPC returned an error")
        if "result" not in result:
            raise RpcError("Solana RPC response is missing result")
        return result["result"]

    @staticmethod
    def decode_base64_account(account: dict[str, Any]) -> bytes:
        try:
            encoded = account["data"]
            if not isinstance(encoded, list) or len(encoded) != 2 or encoded[1] != "base64":
                raise ValueError("unsupported account encoding")
            return base64.b64decode(encoded[0], validate=True)
        except (KeyError, TypeError, ValueError, binascii.Error) as exc:
            raise RpcError("Solana RPC account data was invalid") from exc

    async def list_client_escrows(
        self, program_id: str, client_key: str
    ) -> list[tuple[str, bytes]]:
        response = await self.call(
            "getProgramAccounts",
            [
                program_id,
                {
                    "encoding": "base64",
                    "filters": [
                        {"dataSize": ESCROW_ACCOUNT_SIZE},
                        {"memcmp": {"offset": CLIENT_MEMCMP_OFFSET, "bytes": client_key}},
                    ],
                },
            ],
        )
        if not isinstance(response, list):
            raise RpcError("expected a list of accounts")
        result = []
        for item in response:
            if not isinstance(item, dict) or not isinstance(item.get("pubkey"), str):
                raise RpcError("malformed getProgramAccounts item")
            account = item.get("account")
            if not isinstance(account, dict) or account.get("owner") != program_id:
                raise RpcError("account owner does not match requested program")
            result.append((item["pubkey"], self.decode_base64_account(account)))
        return result

    async def get_escrow(self, program_id: str, address: str) -> bytes | None:
        response = await self.call("getAccountInfo", [address, {"encoding": "base64"}])
        if not isinstance(response, dict):
            raise RpcError("malformed getAccountInfo result")
        account = response.get("value")
        if account is None:
            return None
        if not isinstance(account, dict) or account.get("owner") != program_id:
            return None
        return self.decode_base64_account(account)
