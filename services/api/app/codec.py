"""Anchor account codec matching `programs/proofdock/src/lib.rs` field order."""

import hashlib
import struct
from dataclasses import dataclass

from .base58 import b58encode

# Anchor account discriminator is SHA256("account:<TypeName>")[:8].
ESCROW_DISCRIMINATOR = hashlib.sha256(b"account:Escrow").digest()[:8]
# id, client, contractor, arbitrator, amount, deadline, metadata_hash, disputed, bump
ESCROW_LAYOUT = struct.Struct("<Q32s32s32sQq32s?B")
ESCROW_ACCOUNT_SIZE = 8 + ESCROW_LAYOUT.size
CLIENT_MEMCMP_OFFSET = 8 + 8  # discriminator + u64 id


@dataclass(frozen=True, slots=True)
class EscrowRecord:
    address: str
    milestone_id: int
    client: str
    contractor: str
    arbitrator: str
    amount_lamports: int
    deadline_unix: int
    metadata_sha256: str
    disputed: bool
    bump: int

    def as_dict(self) -> dict:
        return {
            "address": self.address,
            "milestone_id": self.milestone_id,
            "client": self.client,
            "contractor": self.contractor,
            "arbitrator": self.arbitrator,
            "amount_lamports": self.amount_lamports,
            "deadline_unix": self.deadline_unix,
            "metadata_sha256": self.metadata_sha256,
            "bump": self.bump,
            "disputed": self.disputed,
            "status": "disputed" if self.disputed else "funded",
        }


def decode_escrow(data: bytes, address: str) -> EscrowRecord:
    """Decode a *funded* escrow. Settled escrows are closed on-chain."""
    if len(data) != ESCROW_ACCOUNT_SIZE:
        raise ValueError(f"expected {ESCROW_ACCOUNT_SIZE} bytes; received {len(data)}")
    if data[:8] != ESCROW_DISCRIMINATOR:
        raise ValueError("not a ProofDock Escrow account")
    milestone_id, client, contractor, arbitrator, amount, deadline, metadata, disputed, bump = (
        ESCROW_LAYOUT.unpack_from(data, 8)
    )
    return EscrowRecord(
        address=address,
        milestone_id=milestone_id,
        client=b58encode(client),
        contractor=b58encode(contractor),
        arbitrator=b58encode(arbitrator),
        amount_lamports=amount,
        deadline_unix=deadline,
        metadata_sha256=metadata.hex(),
        disputed=disputed,
        bump=bump,
    )
