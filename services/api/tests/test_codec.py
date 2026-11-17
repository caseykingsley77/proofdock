import pytest

from app.base58 import b58decode, b58encode, validate_pubkey
from app.codec import (
    CLIENT_MEMCMP_OFFSET,
    ESCROW_ACCOUNT_SIZE,
    ESCROW_DISCRIMINATOR,
    decode_escrow,
)


def test_expected_escrow_account_layout():
    assert ESCROW_ACCOUNT_SIZE == 129
    assert CLIENT_MEMCMP_OFFSET == 16
    assert len(ESCROW_DISCRIMINATOR) == 8


@pytest.mark.parametrize("payload", [bytes(32), bytes(range(32)), bytes([255] * 32)])
def test_base58_round_trip(payload):
    assert b58decode(b58encode(payload)) == payload


def test_base58_zero_prefix():
    assert b58decode("1112") == b"\x00\x00\x00\x01"


@pytest.mark.parametrize("input_value", ["", "0", "O0OO", "I", "solana", "111", "1" * 33])
def test_rejects_bad_pubkeys(input_value):
    with pytest.raises(ValueError):
        validate_pubkey(input_value)


def test_decodes_anchor_escrow(account_data):
    record = decode_escrow(account_data, "EscrowAccount")
    assert record.milestone_id == 77
    assert record.amount_lamports == 5_000_000
    assert record.deadline_unix == 1_805_000_000
    assert record.metadata_sha256 == "a7" * 32
    assert record.bump == 251
    assert record.client == b58encode(bytes(range(1, 33)))
    assert record.contractor == b58encode(bytes(range(33, 65)))
    assert record.as_dict()["status"] == "funded"


def test_rejects_wrong_discriminator(account_data):
    with pytest.raises(ValueError, match="not a ProofDock"):
        decode_escrow(b"wrong!!!" + account_data[8:], "EscrowAccount")


def test_rejects_truncated_data(account_data):
    with pytest.raises(ValueError, match="expected 129 bytes"):
        decode_escrow(account_data[:-1], "EscrowAccount")
