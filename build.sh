#!/bin/bash

# --- Configuration ---
APP_NAME="Pulse"
APP_IDENTIFIER="com.canmi.pulse"
EXE_NAME="pulse"

# --- Script ---
set -e

echo "Fetching version from Cargo.toml..."
VERSION=$(grep '^version' Cargo.toml | sed 's/version = "\(.*\)"/\1/')
if [ -z "$VERSION" ]; then
    echo "Error: Could not find version in Cargo.toml"
    exit 1
fi
echo "Version: ${VERSION}"

echo "Building release binary..."
cargo build --release

if [ $? -ne 0 ]; then
    echo "Cargo build failed. Aborting."
    exit 1
fi

APP_BUNDLE_PATH="target/release/${APP_NAME}.app"

echo "Creating .app bundle structure at ${APP_BUNDLE_PATH}"
# Clean up previous bundle if it exists
rm -rf "${APP_BUNDLE_PATH}"
mkdir -p "${APP_BUNDLE_PATH}/Contents/MacOS"
mkdir -p "${APP_BUNDLE_PATH}/Contents/Resources"

echo "Copying binary..."
cp "target/release/${EXE_NAME}" "${APP_BUNDLE_PATH}/Contents/MacOS/"

echo "Creating Info.plist..."
cat > "${APP_BUNDLE_PATH}/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>${EXE_NAME}</string>
    <key>CFBundleIdentifier</key>
    <string>${APP_IDENTIFIER}</string>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSUIElement</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
    <key>CFBundleIconFile</key>
    <string>icon.icns</string>
</dict>
</plist>
EOF

echo "----------------------------------------"
echo "Build complete!"
echo "Application bundle is at: ${APP_BUNDLE_PATH}"
echo "----------------------------------------"
