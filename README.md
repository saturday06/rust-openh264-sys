# openh264-sys

[![Build Status](https://travis-ci.org/saturday06/rust-openh264-sys.svg?branch=master)](https://travis-ci.org/saturday06/rust-openh264-sys)
[![Build Status](https://ci.appveyor.com/api/projects/status/github/saturday06/rust-openh264-sys?branch=master&svg=true)](https://ci.appveyor.com/project/saturday06/rust-openh264-sys)
[![crates.io](https://img.shields.io/crates/v/openh264-sys.svg)](https://crates.io/crates/openh264-sys)

Bindings to OpenH264.

## features

### ‘build’ feature
Download and build openh264 source.

### ‘static’ feature
Link static openh264 library.

## Specify custom openh264 installation prefix

Set environment variable `OPENH264_INCLUDE_PATH` and `OPENH264_LIBRARY_PATH`. Then `$OPENH264_INCLUDE_PATH/wels/codec_api.h` and `$OPENH264_LIBRARY_PATH/libopenh264.so` must be exist.
