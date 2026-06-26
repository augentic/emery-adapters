#!/usr/bin/env bash
# CLI-owned simulator build — destination is fixed here, not in the Makefile.
set -euo pipefail

DEST='generic/platform=iOS Simulator'
APP_NAME='__APP_NAME__'
IOS_DIR="$(cd "$(dirname "$0")/.." && pwd)"

cd "$IOS_DIR"

xcodebuild build \
	-project "${APP_NAME}.xcodeproj" \
	-scheme "${APP_NAME}" \
	-destination "$DEST" \
	-configuration Debug \
	CODE_SIGNING_ALLOWED=NO \
	2>&1 | xcbeautify
