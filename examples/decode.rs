extern crate openh264_sys;

use openh264_sys::*;
use std::ptr::null_mut;

fn main() {
    let mut decoder = null_mut();
    let param = SDecodingParam::default();
    unsafe {
        assert_eq!(WelsCreateDecoder(&mut decoder), 0);
        assert!(!decoder.is_null());
        assert_eq!((**decoder).Initialize.unwrap()(decoder, &param), 0);
        assert_eq!((**decoder).Uninitialize.unwrap()(decoder), 0);
        WelsDestroyDecoder(decoder);
    }
}
