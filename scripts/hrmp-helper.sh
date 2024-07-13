#!/bin/bash

# Helper script to open HRMP channels between Muse and Asset Hub.
# This script is meant to be run after the relay chain and parachains are spawned.

function ensure_polkadot_js_api() {
    echo "*** Checking for required polkadot-js-api"
    if ! which polkadot-js-api &>/dev/null; then
        echo ''
        echo 'Required command `polkadot-js-api` not in PATH, please, install, e.g.:'
        echo "npm install -g @polkadot/api-cli"
        echo "      or"
        echo "yarn global add @polkadot/api-cli"
        echo ''
        exit 1
    fi
}

function open_hrmp_channels() {
    local relay_url=$1
    local relay_chain_seed=$2
    local sender_para_id=$3
    local recipient_para_id=$4
    local max_capacity=$5
    local max_message_size=$6
    echo "  calling open_hrmp_channels:"
    echo "      relay_url: ${relay_url}"
    echo "      relay_chain_seed: ${relay_chain_seed}"
    echo "      sender_para_id: ${sender_para_id}"
    echo "      recipient_para_id: ${recipient_para_id}"
    echo "      max_capacity: ${max_capacity}"
    echo "      max_message_size: ${max_message_size}"
    echo "      params:"
    echo "--------------------------------------------------"
    polkadot-js-api \
        --ws "${relay_url?}" \
        --seed "${relay_chain_seed?}" \
        --sudo \
        tx.hrmp.forceOpenHrmpChannel \
        ${sender_para_id} \
        ${recipient_para_id} \
        ${max_capacity} \
        ${max_message_size}
}

# Check for polkadot-js-api cli
ensure_polkadot_js_api

# HRMP: Myth - Asset Hub
open_hrmp_channels \
    "ws://127.0.0.1:9900" \
    "//Alice" \
    3369 1000 8 1048576

# HRMP: Asset Hub - Myth
open_hrmp_channels \
    "ws://127.0.0.1:9900" \
    "//Alice" \
    1000 3369 8 1048576
