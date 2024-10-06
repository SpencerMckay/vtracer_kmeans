#!/bin/bash

set -e

LIB_NAME=libvtracer_kmeans.a
IOS_LIB_DIR=ios_lib

# Create output directory for the compiled libraries
mkdir -p $IOS_LIB_DIR

# Build for iOS simulator (x86_64)
cargo build --target x86_64-apple-ios --release
cp target/x86_64-apple-ios/release/$LIB_NAME $IOS_LIB_DIR/$LIB_NAME.x86_64.a

# Build for iOS simulator (ARM64 for Apple Silicon)
cargo build --target aarch64-apple-ios-sim --release
cp target/aarch64-apple-ios-sim/release/$LIB_NAME $IOS_LIB_DIR/$LIB_NAME.arm64.sim.a

# Build for iOS device (ARM64)
cargo build --target aarch64-apple-ios --release
cp target/aarch64-apple-ios/release/$LIB_NAME $IOS_LIB_DIR/$LIB_NAME.arm64.a

# Combine x86_64 and arm64 simulator architectures into a single library
lipo -create -output $IOS_LIB_DIR/$LIB_NAME.simulator.a $IOS_LIB_DIR/$LIB_NAME.x86_64.a $IOS_LIB_DIR/$LIB_NAME.arm64.sim.a

# Create xcframework using the properly named libraries
xcodebuild -create-xcframework \
    -library $IOS_LIB_DIR/$LIB_NAME.arm64.a \
    -headers include \
    -library $IOS_LIB_DIR/$LIB_NAME.simulator.a \
    -headers include \
    -output $IOS_LIB_DIR/png_to_svg_rust.xcframework

echo "XCFramework created at $IOS_LIB_DIR/png_to_svg_rust.xcframework"