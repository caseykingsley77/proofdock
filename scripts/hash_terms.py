#!/usr/bin/env python3
"""Print a canonical JSON SHA-256 commitment used as ProofDock metadata_hash.

Usage:
    python scripts/hash_terms.py agreement.json
"""

import argparse
import hashlib
import json
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("json_path", type=Path, help="JSON terms agreed by both parties")
    args = parser.parse_args()

    with args.json_path.open(encoding="utf-8") as handle:
        document = json.load(handle)
    canonical = json.dumps(
        document, ensure_ascii=False, sort_keys=True,
        separators=(",", ":"), allow_nan=False,
    ).encode("utf-8")
    print(f"SHA-256: {hashlib.sha256(canonical).hexdigest()}")
    print(f"Canonical JSON bytes: {len(canonical)}")


if __name__ == "__main__":
    main()
