"""ProofDock HTTP API: on-chain state is authoritative; endpoints are read-only."""

import os
from functools import lru_cache

from fastapi import Depends, FastAPI, HTTPException, Query
from pydantic import BaseModel

from .base58 import validate_pubkey
from .codec import decode_escrow
from .rpc import RpcError, SolanaRpc

DEV_PROGRAM_ID = "Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS"


class EscrowResponse(BaseModel):
    address: str
    milestone_id: int
    client: str
    contractor: str
    amount_lamports: int
    deadline_unix: int
    metadata_sha256: str
    bump: int
    status: str


@lru_cache
def program_id() -> str:
    configured = os.getenv("PROOFDOCK_PROGRAM_ID", DEV_PROGRAM_ID)
    return validate_pubkey(configured)


def rpc_dependency() -> SolanaRpc:
    return SolanaRpc(os.getenv("SOLANA_RPC_URL", "http://127.0.0.1:8899"))


app = FastAPI(
    title="ProofDock API",
    version="0.1.0",
    description="Read-only Solana milestone escrow account explorer",
)


@app.get("/health")
async def health() -> dict[str, str]:
    return {"status": "ok", "service": "proofdock-api"}


@app.get("/v1/escrows", response_model=list[EscrowResponse])
async def list_escrows(
    client: str = Query(..., description="Client's 32-byte base58 Solana wallet"),
    rpc: SolanaRpc = Depends(rpc_dependency),
) -> list[dict]:
    try:
        validate_pubkey(client)
    except ValueError as exc:
        raise HTTPException(status_code=422, detail=str(exc)) from exc
    try:
        raw_accounts = await rpc.list_client_escrows(program_id(), client)
        return [decode_escrow(data, address).as_dict() for address, data in raw_accounts]
    except (RpcError, ValueError) as exc:
        raise HTTPException(status_code=502, detail="upstream Solana data unavailable") from exc


@app.get("/v1/escrows/{address}", response_model=EscrowResponse)
async def get_escrow(
    address: str,
    rpc: SolanaRpc = Depends(rpc_dependency),
) -> dict:
    try:
        validate_pubkey(address)
    except ValueError as exc:
        raise HTTPException(status_code=422, detail=str(exc)) from exc
    try:
        account = await rpc.get_escrow(program_id(), address)
        if account is None:
            raise HTTPException(status_code=404, detail="funded escrow not found")
        return decode_escrow(account, address).as_dict()
    except (RpcError, ValueError) as exc:
        raise HTTPException(status_code=502, detail="upstream Solana data unavailable") from exc
