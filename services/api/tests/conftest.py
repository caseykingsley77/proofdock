"""Fixtures construct the exact account layout produced by the Rust program."""

import base64

import pytest
from fastapi.testclient import TestClient

from app.codec import ESCROW_DISCRIMINATOR, ESCROW_LAYOUT
from app.main import DEV_PROGRAM_ID, app

CLIENT_BYTES = bytes(range(1, 33))
CONTRACTOR_BYTES = bytes(range(33, 65))
ARBITRATOR_BYTES = bytes(range(100, 132))
ACCOUNT_BYTES = bytes(range(65, 97))
PROGRAM_BYTES = bytes(range(100, 132))


@pytest.fixture
def account_data() -> bytes:
    return ESCROW_DISCRIMINATOR + ESCROW_LAYOUT.pack(
        77, CLIENT_BYTES, CONTRACTOR_BYTES, ARBITRATOR_BYTES,
        5_000_000, 1_805_000_000,
        bytes.fromhex("a7" * 32), False, 251,
    )


@pytest.fixture
def rpc_account(account_data):
    return {
        "data": [base64.b64encode(account_data).decode("ascii"), "base64"],
        "owner": DEV_PROGRAM_ID,
        "lamports": 5_001_000,
        "executable": False,
        "rentEpoch": 0,
    }


@pytest.fixture
def test_client():
    return TestClient(app)
