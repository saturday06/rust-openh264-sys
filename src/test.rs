use super::*;
use std::ptr::null_mut;

#[test]
fn encode() {
    let mut encoder = null_mut();
    let width = 32;
    let height = 32;
    unsafe {
        assert_eq!(WelsCreateSVCEncoder(&mut encoder), 0);
        assert!(!encoder.is_null());
        let mut param = SEncParamBase::default();
        param.iUsageType = CAMERA_VIDEO_REAL_TIME;
        param.fMaxFrameRate = 1.0 / 30.0;
        param.iPicWidth = width;
        param.iPicHeight = height;
        param.iTargetBitrate = 5000000;
        assert_eq!((**encoder).Initialize.unwrap()(encoder, &param), 0);
        assert_eq!((**encoder).Uninitialize.unwrap()(encoder), 0);
        WelsDestroySVCEncoder(encoder);
    }
}

#[test]
fn decode() {
    let mut decoder = null_mut();
    unsafe {
        assert_eq!(WelsCreateDecoder(&mut decoder), 0);
        assert!(!decoder.is_null());
        let param = SDecodingParam::default();
        assert_eq!((**decoder).Initialize.unwrap()(decoder, &param), 0);
        assert_eq!((**decoder).Uninitialize.unwrap()(decoder), 0);
        WelsDestroyDecoder(decoder);
    }
}
