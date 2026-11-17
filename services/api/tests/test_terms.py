import math

import pytest

from app.terms import canonical_terms_bytes, terms_sha256


def test_key_order_does_not_change_commitment():
    first = {"milestone": "API release", "amount": 100, "client": {"id": "A", "rev": 1}}
    second = {"client": {"rev": 1, "id": "A"}, "amount": 100, "milestone": "API release"}
    assert canonical_terms_bytes(first) == canonical_terms_bytes(second)
    assert terms_sha256(first) == terms_sha256(second)
    assert len(terms_sha256(first)) == 32


def test_changing_terms_changes_commitment():
    assert terms_sha256({"amount": 100}) != terms_sha256({"amount": 101})


def test_unicode_is_utf8_not_escaped():
    assert canonical_terms_bytes({"description": "Lagos → Abuja"}) == (
        '{"description":"Lagos → Abuja"}'.encode("utf-8")
    )


def test_non_finite_numbers_are_rejected():
    with pytest.raises(ValueError):
        canonical_terms_bytes({"amount": math.nan})
