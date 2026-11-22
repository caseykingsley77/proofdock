"""Prevent the on-chain program and off-chain API from silently drifting apart."""

import tomllib
from pathlib import Path

import pytest

from app.main import DEV_PROGRAM_ID, program_id


ROOT = Path(__file__).resolve().parents[3]


def test_default_program_id_matches_anchor_and_rust():
    anchor = tomllib.loads((ROOT / "Anchor.toml").read_text(encoding="utf-8"))
    rust = (ROOT / "programs/proofdock/src/lib.rs").read_text(encoding="utf-8")

    assert DEV_PROGRAM_ID == anchor["programs"]["localnet"]["proofdock"]
    assert f'declare_id!("{DEV_PROGRAM_ID}");' in rust


def test_program_id_can_be_overridden_for_a_deployment(monkeypatch):
    deployed_program_id = "11111111111111111111111111111111"
    monkeypatch.setenv("PROOFDOCK_PROGRAM_ID", deployed_program_id)
    program_id.cache_clear()
    try:
        assert program_id() == deployed_program_id
    finally:
        program_id.cache_clear()


def test_invalid_program_id_is_rejected(monkeypatch):
    monkeypatch.setenv("PROOFDOCK_PROGRAM_ID", "not-a-public-key")
    program_id.cache_clear()
    try:
        with pytest.raises(ValueError):
            program_id()
    finally:
        program_id.cache_clear()
