#!/bin/bash
source "$HOME/.cargo/env"
cd "$(dirname "$0")/.."
tauri dev
