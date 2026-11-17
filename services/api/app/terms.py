"""Deterministic SHA-256 commitment for UTF-8 JSON milestone terms."""

import hashlib
import json
from typing import Any


def canonical_terms_bytes(document: Any) -> bytes:
    """Serialize a JSON document without whitespace/key-order ambiguity.

    Parties must agree to this exact serialization convention; a hash only
    commits to bytes, not to the truth or enforceability of a contract.
    """
    return json.dumps(
        document,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")


def terms_sha256(document: Any) -> bytes:
    return hashlib.sha256(canonical_terms_bytes(document)).digest()
