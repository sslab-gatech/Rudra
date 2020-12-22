#!/bin/bash -e
source ci/env.sh
python test.py
if ! command -v cargo-download &> /dev/null
then
    cargo install cargo-download
fi
python ci/end_to_end_test.py
