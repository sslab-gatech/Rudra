#!/usr/bin/env python3
import os
import subprocess
import sys

from pathlib import Path

INDEX_URL = "https://github.com/Qwaz/crates.io-index-2020-07-04"

# This fork was created with the following revision
# There could be a slight mismatch between the db dump and the index,
# but it should be fine if the index is newer
BRANCH = "snapshot-2020-08-04"
COMMIT = "d6e73a1202079ae5945a7984572995e89ced729c"

if len(sys.argv) < 2:
    print(f"Usage: {sys.argv[0]} <path>", file=sys.stderr)
    exit(1)

rudra_home_path = Path(sys.argv[1])

# Sanity check
if rudra_home_path.exists():
    print(f"Error: {rudra_home_path} already exists", file=sys.stderr)
    exit(1)

# match directory names with the Rudra runner
rudra_home_path.mkdir()

cargo_home_path = rudra_home_path / "cargo_home"
cargo_home_path.mkdir()

with open(cargo_home_path / "config.toml", "w") as f:
    f.write(f"""[source]
[source.crates-io]
replace-with = "crates-io-forked"

[source.crates-io-forked]
registry = "{INDEX_URL}"
""")

sccache_home_path = rudra_home_path / "sccache_home"
sccache_home_path.mkdir()

rudra_cache_path = rudra_home_path / "rudra_cache"
rudra_cache_path.mkdir()

campaign_path = rudra_home_path / "campaign"
campaign_path.mkdir()
