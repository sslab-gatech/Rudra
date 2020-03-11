#!/usr/bin/env python
import os
import re
import sys

if len(sys.argv) < 3:
    print(f"Usage: {sys.argv[0]} library version")
    exit(1)

lib = sys.argv[1]
version = sys.argv[2]

# find next poc number
poc_dir_pattern = re.compile(r"poc-(\d{3})-.+")
poc_id_set = set()

for name in os.listdir('./'):
    if os.path.isdir(name):
        match = poc_dir_pattern.match(name)
        poc_id_set.add(match.group(1))

for poc_id_num in range(1000):
    poc_id_str = str(poc_id_num).rjust(3, '0')
    if poc_id_str not in poc_id_set:
        break

assert poc_id_str not in poc_id_set
poc_dir_name = f"poc-{poc_id_str}-{lib}"

os.system(f"cargo init {poc_dir_name}")

manifest_path = os.path.join(poc_dir_name, "Cargo.toml")

with open(manifest_path, "a") as f:
    f.write(f"{lib} = \"={version}\"\n")

print(f"Created `{poc_dir_name}` with version {version}")
