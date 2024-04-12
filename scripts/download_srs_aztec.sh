#!/usr/bin/env bash

set -euo pipefail

# Check if the .env file exists
if [ -f .env ]; then
    # Load variables from .env into the environment
    source .env
    echo "Environment variables loaded from .env"
else
    echo ".env file not found"
fi

if [ -f "$AZTEC_SRS_PATH" ]; then
    echo "SRS file $AZTEC_SRS_PATH exists"
else
    echo "SRS file $AZTEC_SRS_PATH does not exist, downloading ..."
    # Perform the curl request and capture the HTTP response code
    url="https://github.com/alxiong/ark-srs/releases/v0.2.0/$(basename $AZTEC_SRS_PATH)"
    http_code=$(curl -s -o /dev/null -w "%{http_code}" "$url")
    # Check if the HTTP response code is not 404 (Not Found)
    if [ "$http_code" -ne 404 ]; then
        mkdir -p data/aztec20
        curl -LH "Accept: application/octet-stream" -o "$AZTEC_SRS_PATH" "$url"
    else
        echo "URL $url is not found"
        exit 1
    fi
fi
