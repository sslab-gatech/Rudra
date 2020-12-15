#!/bin/bash -e
source ci/env.sh
python test.py
(cargo install cargo-download || exit 0)
python ci/end_to_end_test.py
