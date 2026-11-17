"""Strict base58 handling for 32-byte Solana addresses (no wallet dependency)."""

ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
INDEX = {char: idx for idx, char in enumerate(ALPHABET)}


def b58encode(data: bytes) -> str:
    """Encode bytes as unpadded Bitcoin base58, keeping zero-byte prefixes."""
    number = int.from_bytes(data, "big")
    encoded = ""
    while number:
        number, remainder = divmod(number, 58)
        encoded = ALPHABET[remainder] + encoded
    leading_zeroes = len(data) - len(data.lstrip(b"\x00"))
    return "1" * leading_zeroes + encoded


def b58decode(value: str) -> bytes:
    """Decode base58, rejecting invalid alphabet characters."""
    if not isinstance(value, str) or not value:
        raise ValueError("base58 value is required")
    number = 0
    for character in value:
        if character not in INDEX:
            raise ValueError("invalid base58 character")
        number = number * 58 + INDEX[character]
    length = (number.bit_length() + 7) // 8
    result = number.to_bytes(length, "big") if length else b""
    return b"\x00" * (len(value) - len(value.lstrip("1"))) + result


def validate_pubkey(value: str) -> str:
    """Validate the canonical textual representation of a Solana 32-byte key."""
    try:
        raw = b58decode(value)
    except ValueError as exc:
        raise ValueError("invalid Solana public key") from exc
    if len(raw) != 32 or b58encode(raw) != value:
        raise ValueError("Solana public keys must be canonical 32-byte base58")
    return value
