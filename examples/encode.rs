extern crate openh264_sys;

use openh264_sys::*;
use std::ptr::null_mut;

fn main() {
    let mut encoder = null_mut();
    let width = 32;
    let height = 32;
    let mut param = SEncParamBase::default();
    param.iUsageType = CAMERA_VIDEO_REAL_TIME;
    param.fMaxFrameRate = 1.0 / 30.0;
    param.iPicWidth = width;
    param.iPicHeight = height;
    param.iTargetBitrate = 5000000;
    unsafe {
        assert_eq!(WelsCreateSVCEncoder(&mut encoder), 0);
        assert!(!encoder.is_null());
        assert_eq!((**encoder).Initialize.unwrap()(encoder, &param), 0);
        assert_eq!((**encoder).Uninitialize.unwrap()(encoder), 0);
        WelsDestroySVCEncoder(encoder);
    }
}
