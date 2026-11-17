"""Anchor account codec matching `programs/proofdock/src/lib.rs` field order."""

import hashlib
import struct
from dataclasses import dataclass

from .base58 import b58encode

# Anchor account discriminator is SHA256("account:<TypeName>")[:8].
ESCROW_DISCRIMINATOR = hashlib.sha256(b"account:Escrow").digest()[:8]
# id, client, contractor, amount, deadline, metadata_hash, bump
ESCROW_LAYOUT = struct.Struct("<Q32s32sQq32sB")
ESCROW_ACCOUNT_SIZE = 8 + ESCROW_LAYOUT.size
CLIENT_MEMCMP_OFFSET = 8 + 8  # discriminator + u64 id


@dataclass(frozen=True, slots=True)
class EscrowRecord:
    address: str
    milestone_id: int
    client: str
    contractor: str
    amount_lamports: int
    deadline_unix: int
    metadata_sha256: str
    bump: int

    def as_dict(self) -> dict:
        return {
            "address": self.address,
            "milestone_id": self.milestone_id,
            "client": self.client,
            "contractor": self.contractor,
            "amount_lamports": self.amount_lamports,
            "deadline_unix": self.deadline_unix,
            "metadata_sha256": self.metadata_sha256,
            "bump": self.bump,
            "status": "funded",
        }


def decode_escrow(data: bytes, address: str) -> EscrowRecord:
    """Decode a *funded* escrow. Settled escrows are closed on-chain."""
    if len(data) != ESCROW_ACCOUNT_SIZE:
        raise ValueError(f"expected {ESCROW_ACCOUNT_SIZE} bytes; received {len(data)}")
    if data[:8] != ESCROW_DISCRIMINATOR:
        raise ValueError("not a ProofDock Escrow account")
    milestone_id, client, contractor, amount, deadline, metadata, bump = (
        ESCROW_LAYOUT.unpack_from(data, 8)
    )
    return EscrowRecord(
        address=address,
        milestone_id=milestone_id,
        client=b58encode(client),
        contractor=b58encode(contractor),
        amount_lamports=amount,
        deadline_unix=deadline,
        metadata_sha256=metadata.hex(),
        bump=bump,
    )
