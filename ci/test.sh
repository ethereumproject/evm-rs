#!/bin/bash
set -e
set -x

source $HOME/.cargo/env
cargo $@ test --all
