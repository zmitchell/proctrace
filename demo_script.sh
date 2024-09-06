#!/usr/bin/env bash

set -euo pipefail

"$(command -v printf)" "Hello, World!\n"
sleep 0.25
curl_status="$(curl -s -X GET "example.com" -o /dev/null -w "%{http_code}")"
"$(command -v printf)" "example.com status: %s\n" "$curl_status"
