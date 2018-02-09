use super::*;
use std::os::raw::{c_int, c_void};
use std::ptr::null_mut;
use std::slice::from_raw_parts;

#[test]
fn encode() {
    let mut encoder = null_mut();
    let width = 32;
    let height = 32;
    unsafe {
        assert_eq!(WelsCreateSVCEncoder(&mut encoder), 0);
        assert!(!encoder.is_null());

        let mut param = SEncParamExt::default();
        assert_eq!(
            (**encoder).GetDefaultParams.unwrap()(encoder, &mut param),
            0
        );

        let fps = 60.0;
        let bitrate = 1000_000;
        param.iUsageType = CAMERA_VIDEO_REAL_TIME;
        param.fMaxFrameRate = fps;
        param.iMaxBitrate = UNSPECIFIED_BIT_RATE as i32;
        param.iSpatialLayerNum = 1;
        param.sSpatialLayers[0].uiProfileIdc = PRO_BASELINE;
        param.sSpatialLayers[0].iVideoWidth = width as i32;
        param.sSpatialLayers[0].iVideoHeight = height as i32;
        param.sSpatialLayers[0].fFrameRate = fps;
        param.sSpatialLayers[0].iSpatialBitrate = bitrate;
        param.sSpatialLayers[0].iMaxSpatialBitrate = UNSPECIFIED_BIT_RATE as i32;
        param.sSpatialLayers[0].sSliceArgument.uiSliceMode = SM_FIXEDSLCNUM_SLICE;
        param.sSpatialLayers[0].sSliceArgument.uiSliceNum = 4;
        param.iPicWidth = width as i32;
        param.iPicHeight = height as i32;
        param.iTargetBitrate = bitrate;

        assert_eq!((**encoder).InitializeExt.unwrap()(encoder, &mut param), 0);

        let mut video_format = videoFormatI420 as c_int;
        assert_eq!(
            (**encoder).SetOption.unwrap()(
                encoder,
                ENCODER_OPTION_DATAFORMAT,
                &mut video_format as *mut c_int as *mut c_void,
            ),
            0
        );

        let mut rc_frame_skip = 0 as c_int;
        assert_eq!(
            (**encoder).SetOption.unwrap()(
                encoder,
                ENCODER_OPTION_RC_FRAME_SKIP,
                &mut rc_frame_skip as *mut c_int as *mut c_void,
            ),
            0
        );

        let mut out = Vec::new();
        let mut y_input = Vec::new();
        y_input.resize(width * height, 0);
        let mut u_input = Vec::new();
        u_input.resize((width / 2) * (height / 2), 0);
        let mut v_input = Vec::new();
        v_input.resize((width / 2) * (height / 2), 0);

        let mut info = SFrameBSInfo::default();
        let mut pic = SSourcePicture::default();
        pic.iPicWidth = width as i32;
        pic.iPicHeight = height as i32;
        pic.iColorFormat = videoFormatI420 as i32;
        pic.iStride[0] = pic.iPicWidth;
        pic.iStride[1] = pic.iPicWidth / 2;
        pic.iStride[2] = pic.iPicWidth / 2;
        pic.pData[0] = y_input.as_mut_ptr();
        pic.pData[1] = u_input.as_mut_ptr();
        pic.pData[2] = v_input.as_mut_ptr();

        if true {
            assert_eq!((**encoder).ForceIntraFrame.unwrap()(encoder, true), 0);
        }

        assert_eq!(
            (**encoder).EncodeFrame.unwrap()(encoder, &mut pic, &mut info),
            0
        );

        if info.eFrameType == videoFrameTypeSkip {
            panic!("Unexpected videoFrameTypeSkip")
        } else if info.eFrameType == videoFrameTypeInvalid {
            panic!("Unexpected videoFrameTypeInvalid")
        } else if info.eFrameType == videoFrameTypeIDR || info.eFrameType == videoFrameTypeI
            || info.eFrameType == videoFrameTypeP
            || info.eFrameType == videoFrameTypeIPMixed
        {
        } else {
            panic!("Unexpected frame: {:?}", info.eFrameType)
        }

        for spatial_id in 0..1 {
            for layer in 0..info.iLayerNum {
                if info.sLayerInfo[layer as usize].uiSpatialId != spatial_id {
                    continue;
                }

                let mut size = 0;
                for i in 0..info.sLayerInfo[layer as usize].iNalCount {
                    size += *info.sLayerInfo[layer as usize]
                        .pNalLengthInByte
                        .offset(i as isize)
                }
                if size > 0 {
                    out.extend_from_slice(from_raw_parts(
                        info.sLayerInfo[layer as usize].pBsBuf,
                        size as usize,
                    ))
                }
            }
        }

        assert!(out.len() > 0);
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
