use super::{EncodedVideoFrame, EncoderBackend, EncoderError, EncoderSettings, VideoEncoder};
use crate::capture::CapturedFrame;
use glyphray_protocol::VideoCodec;
use std::collections::VecDeque;
use std::mem::ManuallyDrop;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use windows::core::{Interface, PWSTR, VARIANT};
use windows::Win32::Media::MediaFoundation::{
    eAVEncCommonRateControlMode_CBR, eAVEncCommonRateControlMode_LowDelayVBR,
    eAVEncH264VProfile_Base, CLSID_MSH264EncoderMFT, CODECAPI_AVEncCommonQualityVsSpeed,
    CODECAPI_AVEncCommonRateControlMode, CODECAPI_AVEncMPVDefaultBPictureCount,
    CODECAPI_AVEncMPVGOPSize, CODECAPI_AVEncVideoForceKeyFrame, CODECAPI_AVLowLatencyMode,
    ICodecAPI, IMFActivate, IMFMediaBuffer, IMFMediaEventGenerator, IMFSample, IMFTransform,
    METransformHaveOutput, METransformNeedInput, MFCreateMediaType, MFCreateMemoryBuffer,
    MFCreateSample, MFMediaType_Video, MFSampleExtension_CleanPoint, MFShutdown, MFStartup,
    MFTEnumEx, MFT_FRIENDLY_NAME_Attribute, MFVideoFormat_H264, MFVideoFormat_NV12,
    MFVideoInterlace_Progressive, MFSTARTUP_LITE, MFT_CATEGORY_VIDEO_ENCODER,
    MFT_ENUM_FLAG_HARDWARE, MFT_ENUM_FLAG_SORTANDFILTER, MFT_MESSAGE_NOTIFY_BEGIN_STREAMING,
    MFT_MESSAGE_NOTIFY_END_OF_STREAM, MFT_MESSAGE_NOTIFY_END_STREAMING,
    MFT_MESSAGE_NOTIFY_START_OF_STREAM, MFT_OUTPUT_DATA_BUFFER, MFT_OUTPUT_STREAM_PROVIDES_SAMPLES,
    MFT_REGISTER_TYPE_INFO, MF_EVENT_FLAG_NO_WAIT, MF_E_NO_EVENTS_AVAILABLE,
    MF_E_TRANSFORM_NEED_MORE_INPUT, MF_MT_ALL_SAMPLES_INDEPENDENT, MF_MT_AVG_BITRATE,
    MF_MT_FIXED_SIZE_SAMPLES, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE, MF_MT_INTERLACE_MODE,
    MF_MT_MAJOR_TYPE, MF_MT_MPEG2_PROFILE, MF_MT_PIXEL_ASPECT_RATIO, MF_MT_SAMPLE_SIZE,
    MF_MT_SUBTYPE, MF_TRANSFORM_ASYNC, MF_TRANSFORM_ASYNC_UNLOCK, MF_VERSION,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_MULTITHREADED,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardwareEncoderInfo {
    pub backend: EncoderBackend,
    pub friendly_name: String,
}

struct HardwareEncoderCandidate {
    activate: IMFActivate,
    info: HardwareEncoderInfo,
}

pub fn available_h264_hardware_encoders() -> Result<Vec<HardwareEncoderInfo>, EncoderError> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED)
            .ok()
            .map_err(|error| backend_error("CoInitializeEx", error))?;
        if let Err(error) = MFStartup(MF_VERSION, MFSTARTUP_LITE) {
            CoUninitialize();
            return Err(backend_error("MFStartup", error));
        }
        let result = enumerate_hardware_encoders().map(|candidates| {
            candidates
                .into_iter()
                .map(|candidate| candidate.info)
                .collect()
        });
        let _ = MFShutdown();
        CoUninitialize();
        result
    }
}

pub struct MediaFoundationH264Encoder {
    settings: EncoderSettings,
    requested_backend: EncoderBackend,
    backend_name: String,
    transform: Option<IMFTransform>,
    event_generator: Option<IMFMediaEventGenerator>,
    next_sequence: u64,
    pending_frames: VecDeque<(u64, u64)>,
    force_keyframe: bool,
    frame_duration_hns: i64,
    output_buffer_size: u32,
    nv12: Vec<u8>,
    media_foundation_started: bool,
    com_initialized: bool,
}

impl MediaFoundationH264Encoder {
    pub fn new(settings: EncoderSettings) -> Self {
        let requested_backend = settings.backend;
        let frame_duration_hns = 10_000_000_i64 / i64::from(settings.fps.max(1));
        let pixels = settings.width.saturating_mul(settings.height);
        let output_buffer_size = pixels.saturating_mul(2).max(1_048_576);
        Self {
            settings,
            requested_backend,
            backend_name: "not started".to_string(),
            transform: None,
            event_generator: None,
            next_sequence: 1,
            pending_frames: VecDeque::new(),
            force_keyframe: true,
            frame_duration_hns,
            output_buffer_size,
            nv12: Vec::new(),
            media_foundation_started: false,
            com_initialized: false,
        }
    }

    fn validate_settings(&self) -> Result<(), EncoderError> {
        if self.settings.codec != VideoCodec::H264 {
            return Err(EncoderError::InvalidSettings(
                "Media Foundation fallback currently supports H.264 only".to_string(),
            ));
        }
        if self.settings.width == 0
            || self.settings.height == 0
            || self.settings.width % 2 != 0
            || self.settings.height % 2 != 0
        {
            return Err(EncoderError::InvalidSettings(
                "NV12 encoding requires non-zero even dimensions".to_string(),
            ));
        }
        Ok(())
    }

    unsafe fn create_transform(&mut self) -> Result<IMFTransform, EncoderError> {
        let result = CoInitializeEx(None, COINIT_MULTITHREADED);
        result
            .ok()
            .map_err(|error| backend_error("CoInitializeEx", error))?;
        self.com_initialized = true;

        MFStartup(MF_VERSION, MFSTARTUP_LITE).map_err(|error| backend_error("MFStartup", error))?;
        self.media_foundation_started = true;

        if self.requested_backend != EncoderBackend::Software {
            let candidates = match enumerate_hardware_encoders() {
                Ok(candidates) => candidates,
                Err(_) if self.requested_backend == EncoderBackend::Auto => Vec::new(),
                Err(error) => return Err(error),
            };
            let mut failures = Vec::new();
            for candidate in candidates
                .into_iter()
                .filter(|candidate| backend_matches(self.requested_backend, candidate.info.backend))
            {
                let transform = match candidate.activate.ActivateObject::<IMFTransform>() {
                    Ok(transform) => transform,
                    Err(error) => {
                        failures.push(format!(
                            "{} activation: {error}",
                            candidate.info.friendly_name
                        ));
                        continue;
                    }
                };
                let is_async = transform
                    .GetAttributes()
                    .ok()
                    .map(|attributes| {
                        let is_async = attributes
                            .GetUINT32(&MF_TRANSFORM_ASYNC)
                            .unwrap_or_default()
                            != 0;
                        if is_async {
                            let _ = attributes.SetUINT32(&MF_TRANSFORM_ASYNC_UNLOCK, 1);
                        }
                        is_async
                    })
                    .unwrap_or(false);
                match self.configure_transform(&transform) {
                    Ok(()) => {
                        if is_async {
                            match transform.cast::<IMFMediaEventGenerator>() {
                                Ok(generator) => self.event_generator = Some(generator),
                                Err(error) => {
                                    failures.push(format!(
                                        "{} event interface: {error}",
                                        candidate.info.friendly_name
                                    ));
                                    continue;
                                }
                            }
                        }
                        self.settings.backend = candidate.info.backend;
                        self.backend_name = candidate.info.friendly_name;
                        return Ok(transform);
                    }
                    Err(error) => failures.push(format!(
                        "{} configuration: {error}",
                        candidate.info.friendly_name
                    )),
                }
            }

            if self.requested_backend != EncoderBackend::Auto {
                if failures.is_empty() {
                    return Err(EncoderError::BackendUnavailable(self.requested_backend));
                }
                return Err(EncoderError::Backend(format!(
                    "requested {:?} encoder could not start: {}",
                    self.requested_backend,
                    failures.join("; ")
                )));
            }
        }

        let transform: IMFTransform =
            CoCreateInstance(&CLSID_MSH264EncoderMFT, None, CLSCTX_INPROC_SERVER)
                .map_err(|error| backend_error("create Microsoft H.264 encoder MFT", error))?;
        self.configure_transform(&transform)?;
        self.event_generator = None;
        self.settings.backend = EncoderBackend::Software;
        self.backend_name = "Microsoft H.264 Video Encoder MFT".to_string();
        Ok(transform)
    }

    unsafe fn configure_transform(&self, transform: &IMFTransform) -> Result<(), EncoderError> {
        let output_type = MFCreateMediaType()
            .map_err(|error| backend_error("MFCreateMediaType(output)", error))?;
        output_type
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .and_then(|_| output_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264))
            .and_then(|_| {
                output_type.SetUINT32(
                    &MF_MT_AVG_BITRATE,
                    self.settings.target_bitrate_kbps.saturating_mul(1_000),
                )
            })
            .and_then(|_| {
                output_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
            })
            .and_then(|_| {
                output_type.SetUINT32(&MF_MT_MPEG2_PROFILE, eAVEncH264VProfile_Base.0 as u32)
            })
            .and_then(|_| {
                output_type.SetUINT64(
                    &MF_MT_FRAME_SIZE,
                    pack_ratio(self.settings.width, self.settings.height),
                )
            })
            .and_then(|_| {
                output_type.SetUINT64(
                    &MF_MT_FRAME_RATE,
                    pack_ratio(u32::from(self.settings.fps), 1),
                )
            })
            .and_then(|_| output_type.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack_ratio(1, 1)))
            .map_err(|error| backend_error("configure H.264 output media type", error))?;
        transform
            .SetOutputType(0, &output_type, 0)
            .map_err(|error| backend_error("IMFTransform::SetOutputType", error))?;

        let input_size = nv12_len(self.settings.width, self.settings.height)? as u32;
        let input_type = MFCreateMediaType()
            .map_err(|error| backend_error("MFCreateMediaType(input)", error))?;
        input_type
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .and_then(|_| input_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12))
            .and_then(|_| {
                input_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
            })
            .and_then(|_| input_type.SetUINT32(&MF_MT_FIXED_SIZE_SAMPLES, 1))
            .and_then(|_| input_type.SetUINT32(&MF_MT_ALL_SAMPLES_INDEPENDENT, 1))
            .and_then(|_| input_type.SetUINT32(&MF_MT_SAMPLE_SIZE, input_size))
            .and_then(|_| {
                input_type.SetUINT64(
                    &MF_MT_FRAME_SIZE,
                    pack_ratio(self.settings.width, self.settings.height),
                )
            })
            .and_then(|_| {
                input_type.SetUINT64(
                    &MF_MT_FRAME_RATE,
                    pack_ratio(u32::from(self.settings.fps), 1),
                )
            })
            .and_then(|_| input_type.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack_ratio(1, 1)))
            .map_err(|error| backend_error("configure NV12 input media type", error))?;
        transform
            .SetInputType(0, &input_type, 0)
            .map_err(|error| backend_error("IMFTransform::SetInputType", error))?;

        configure_codec_api(transform, &self.settings)?;

        transform
            .ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)
            .and_then(|_| transform.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0))
            .map_err(|error| backend_error("start H.264 transform stream", error))?;

        Ok(())
    }

    unsafe fn switch_to_software_encoder(&mut self) -> Result<(), EncoderError> {
        self.event_generator = None;
        if let Some(transform) = self.transform.take() {
            let _ = transform.ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0);
            let _ = transform.ProcessMessage(MFT_MESSAGE_NOTIFY_END_STREAMING, 0);
        }
        self.pending_frames.clear();
        self.force_keyframe = true;

        let transform: IMFTransform =
            CoCreateInstance(&CLSID_MSH264EncoderMFT, None, CLSCTX_INPROC_SERVER).map_err(
                |error| backend_error("create fallback Microsoft H.264 encoder MFT", error),
            )?;
        self.configure_transform(&transform)?;
        self.settings.backend = EncoderBackend::Software;
        self.backend_name = "Microsoft H.264 Video Encoder MFT (runtime fallback)".to_string();
        self.transform = Some(transform);
        Ok(())
    }

    fn encode_once(&mut self, frame: &CapturedFrame) -> Result<EncodedVideoFrame, EncoderError> {
        let transform = self.transform.clone().ok_or(EncoderError::NotStarted)?;
        bgra_to_nv12(frame, &mut self.nv12)?;

        let sequence = self.next_sequence;
        let sample_time_hns =
            (sequence.saturating_sub(1) as i64).saturating_mul(self.frame_duration_hns);
        let input_sample = unsafe { self.make_input_sample(sample_time_hns)? };

        if std::mem::take(&mut self.force_keyframe) {
            unsafe { force_keyframe(&transform)? };
        }
        unsafe {
            if let Some(generator) = &self.event_generator {
                wait_for_transform_event(generator, METransformNeedInput.0 as u32)?;
            }
            transform
                .ProcessInput(0, &input_sample, 0)
                .map_err(|error| backend_error("IMFTransform::ProcessInput", error))?;
            if let Some(generator) = &self.event_generator {
                wait_for_transform_event(generator, METransformHaveOutput.0 as u32)?;
            }
        }
        self.pending_frames
            .push_back((sequence, frame.capture_timestamp_us));
        self.next_sequence = self.next_sequence.saturating_add(1);
        let (payload, is_keyframe) = unsafe { self.take_output(&transform)? };
        let (output_sequence, capture_timestamp_us) = self
            .pending_frames
            .pop_front()
            .ok_or(EncoderError::OutputUnavailable)?;

        Ok(EncodedVideoFrame {
            sequence: output_sequence,
            codec: VideoCodec::H264,
            capture_timestamp_us,
            encode_done_timestamp_us: now_us(),
            is_keyframe,
            payload,
        })
    }

    unsafe fn make_input_sample(&self, sample_time_hns: i64) -> Result<IMFSample, EncoderError> {
        let buffer = MFCreateMemoryBuffer(self.nv12.len() as u32)
            .map_err(|error| backend_error("MFCreateMemoryBuffer(input)", error))?;
        copy_to_media_buffer(&buffer, &self.nv12)?;

        let sample =
            MFCreateSample().map_err(|error| backend_error("MFCreateSample(input)", error))?;
        sample
            .AddBuffer(&buffer)
            .and_then(|_| sample.SetSampleTime(sample_time_hns))
            .and_then(|_| sample.SetSampleDuration(self.frame_duration_hns))
            .map_err(|error| backend_error("prepare H.264 input sample", error))?;
        Ok(sample)
    }

    unsafe fn take_output(
        &self,
        transform: &IMFTransform,
    ) -> Result<(Vec<u8>, bool), EncoderError> {
        let stream_info = transform
            .GetOutputStreamInfo(0)
            .map_err(|error| backend_error("IMFTransform::GetOutputStreamInfo", error))?;
        let transform_provides_sample =
            stream_info.dwFlags & MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 as u32 != 0;

        let supplied_sample = if transform_provides_sample {
            None
        } else {
            let sample =
                MFCreateSample().map_err(|error| backend_error("MFCreateSample(output)", error))?;
            let buffer_size = stream_info.cbSize.max(self.output_buffer_size);
            let buffer = MFCreateMemoryBuffer(buffer_size)
                .map_err(|error| backend_error("MFCreateMemoryBuffer(output)", error))?;
            sample
                .AddBuffer(&buffer)
                .map_err(|error| backend_error("IMFSample::AddBuffer(output)", error))?;
            Some(sample)
        };

        let mut output = MFT_OUTPUT_DATA_BUFFER {
            dwStreamID: 0,
            pSample: ManuallyDrop::new(supplied_sample.clone()),
            dwStatus: 0,
            pEvents: ManuallyDrop::new(None),
        };
        let mut status = 0_u32;
        let process_result =
            transform.ProcessOutput(0, std::slice::from_mut(&mut output), &mut status);
        let returned_sample = ManuallyDrop::take(&mut output.pSample);
        let returned_events = ManuallyDrop::take(&mut output.pEvents);
        drop(returned_events);

        if let Err(error) = process_result {
            if error.code() == MF_E_TRANSFORM_NEED_MORE_INPUT {
                return Err(EncoderError::OutputUnavailable);
            }
            return Err(backend_error("IMFTransform::ProcessOutput", error));
        }

        let sample = returned_sample
            .or(supplied_sample)
            .ok_or(EncoderError::OutputUnavailable)?;
        let clean_point = sample
            .GetUINT32(&MFSampleExtension_CleanPoint)
            .unwrap_or_default()
            != 0;
        let buffer = sample
            .ConvertToContiguousBuffer()
            .map_err(|error| backend_error("IMFSample::ConvertToContiguousBuffer", error))?;
        let payload = read_media_buffer(&buffer)?;
        if payload.is_empty() {
            return Err(EncoderError::OutputUnavailable);
        }
        let payload = normalize_h264_access_unit(payload);
        let keyframe = clean_point || annex_b_contains_idr(&payload);
        Ok((payload, keyframe))
    }
}

unsafe fn enumerate_hardware_encoders() -> Result<Vec<HardwareEncoderCandidate>, EncoderError> {
    let input_type = MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Video,
        guidSubtype: MFVideoFormat_NV12,
    };
    let output_type = MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Video,
        guidSubtype: MFVideoFormat_H264,
    };
    let mut raw_activates: *mut Option<IMFActivate> = std::ptr::null_mut();
    let mut count = 0_u32;
    MFTEnumEx(
        MFT_CATEGORY_VIDEO_ENCODER,
        MFT_ENUM_FLAG_HARDWARE | MFT_ENUM_FLAG_SORTANDFILTER,
        Some(&input_type),
        Some(&output_type),
        &mut raw_activates,
        &mut count,
    )
    .map_err(|error| backend_error("enumerate hardware H.264 encoder MFTs", error))?;

    if raw_activates.is_null() || count == 0 {
        if !raw_activates.is_null() {
            CoTaskMemFree(Some(raw_activates.cast()));
        }
        return Ok(Vec::new());
    }

    let mut candidates = Vec::with_capacity(count as usize);
    let activates = std::slice::from_raw_parts_mut(raw_activates, count as usize);
    for activate in activates.iter_mut().filter_map(Option::take) {
        let friendly_name = activation_friendly_name(&activate)
            .unwrap_or_else(|_| "Unknown hardware H.264 encoder".to_string());
        candidates.push(HardwareEncoderCandidate {
            activate,
            info: HardwareEncoderInfo {
                backend: classify_hardware_backend(&friendly_name),
                friendly_name,
            },
        });
    }
    CoTaskMemFree(Some(raw_activates.cast()));
    Ok(candidates)
}

unsafe fn activation_friendly_name(activate: &IMFActivate) -> Result<String, EncoderError> {
    let mut value = PWSTR(std::ptr::null_mut());
    let mut length = 0_u32;
    activate
        .GetAllocatedString(&MFT_FRIENDLY_NAME_Attribute, &mut value, &mut length)
        .map_err(|error| backend_error("read hardware encoder friendly name", error))?;
    if value.is_null() {
        return Ok("Unknown hardware H.264 encoder".to_string());
    }
    let name = String::from_utf16_lossy(std::slice::from_raw_parts(value.0, length as usize));
    CoTaskMemFree(Some(value.0.cast()));
    Ok(name)
}

fn classify_hardware_backend(friendly_name: &str) -> EncoderBackend {
    let name = friendly_name.to_ascii_lowercase();
    if name.contains("intel") || name.contains("quick sync") {
        EncoderBackend::IntelQuickSync
    } else if name.contains("nvidia") || name.contains("nvenc") {
        EncoderBackend::NvidiaNvenc
    } else if name.contains("amd")
        || name.contains("advanced micro devices")
        || name.contains("amf")
    {
        EncoderBackend::AmdAmf
    } else {
        EncoderBackend::Hardware
    }
}

fn backend_matches(requested: EncoderBackend, candidate: EncoderBackend) -> bool {
    matches!(requested, EncoderBackend::Auto | EncoderBackend::Hardware) || requested == candidate
}

fn should_runtime_fallback(
    requested: EncoderBackend,
    selected: EncoderBackend,
    error: &EncoderError,
) -> bool {
    requested == EncoderBackend::Auto
        && selected != EncoderBackend::Software
        && matches!(error, EncoderError::Backend(_))
}

impl VideoEncoder for MediaFoundationH264Encoder {
    fn settings(&self) -> &EncoderSettings {
        &self.settings
    }

    fn backend_name(&self) -> &str {
        &self.backend_name
    }

    fn start(&mut self) -> Result<(), EncoderError> {
        self.validate_settings()?;
        if self.transform.is_some() {
            return Ok(());
        }
        self.nv12
            .resize(nv12_len(self.settings.width, self.settings.height)?, 0);
        self.transform = Some(unsafe { self.create_transform()? });
        Ok(())
    }

    fn encode(&mut self, frame: &CapturedFrame) -> Result<EncodedVideoFrame, EncoderError> {
        if frame.width != self.settings.width || frame.height != self.settings.height {
            return Err(EncoderError::DimensionMismatch);
        }
        match self.encode_once(frame) {
            Err(error)
                if should_runtime_fallback(
                    self.requested_backend,
                    self.settings.backend,
                    &error,
                ) =>
            {
                unsafe { self.switch_to_software_encoder()? };
                self.encode_once(frame)
            }
            result => result,
        }
    }

    fn request_keyframe(&mut self) -> Result<(), EncoderError> {
        self.force_keyframe = true;
        Ok(())
    }
}

impl Drop for MediaFoundationH264Encoder {
    fn drop(&mut self) {
        unsafe {
            self.event_generator = None;
            if let Some(transform) = self.transform.take() {
                let _ = transform.ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0);
                let _ = transform.ProcessMessage(MFT_MESSAGE_NOTIFY_END_STREAMING, 0);
            }
            if self.media_foundation_started {
                let _ = MFShutdown();
            }
            if self.com_initialized {
                CoUninitialize();
            }
        }
    }
}

unsafe fn wait_for_transform_event(
    generator: &IMFMediaEventGenerator,
    expected_type: u32,
) -> Result<(), EncoderError> {
    let deadline = Instant::now() + Duration::from_millis(50);
    loop {
        match generator.GetEvent(MF_EVENT_FLAG_NO_WAIT) {
            Ok(event) => {
                let status = event
                    .GetStatus()
                    .map_err(|error| backend_error("read asynchronous encoder event", error))?;
                if status.is_err() {
                    return Err(EncoderError::Backend(format!(
                        "asynchronous encoder event failed: {status:?}"
                    )));
                }
                if event
                    .GetType()
                    .map_err(|error| backend_error("read asynchronous encoder event type", error))?
                    == expected_type
                {
                    return Ok(());
                }
            }
            Err(error) if error.code() == MF_E_NO_EVENTS_AVAILABLE => {
                if Instant::now() >= deadline {
                    return Err(EncoderError::Backend(
                        "asynchronous encoder event timed out after 50 ms".to_string(),
                    ));
                }
                std::thread::sleep(Duration::from_micros(100));
            }
            Err(error) => {
                return Err(backend_error(
                    "read asynchronous encoder event queue",
                    error,
                ));
            }
        }
    }
}

unsafe fn configure_codec_api(
    transform: &IMFTransform,
    settings: &EncoderSettings,
) -> Result<(), EncoderError> {
    let codec_api: ICodecAPI = transform
        .cast()
        .map_err(|error| backend_error("query ICodecAPI", error))?;
    codec_api
        .SetValue(&CODECAPI_AVLowLatencyMode, &VARIANT::from(true))
        .map_err(|error| backend_error("enable low-latency encoder mode", error))?;
    if codec_api
        .SetValue(
            &CODECAPI_AVEncCommonRateControlMode,
            &VARIANT::from(eAVEncCommonRateControlMode_LowDelayVBR.0 as u32),
        )
        .is_err()
    {
        codec_api
            .SetValue(
                &CODECAPI_AVEncCommonRateControlMode,
                &VARIANT::from(eAVEncCommonRateControlMode_CBR.0 as u32),
            )
            .map_err(|error| backend_error("set CBR rate control fallback", error))?;
    }
    // Some hardware MFTs reject the B-picture property even while honoring the
    // Baseline profile, which cannot contain B slices. Keep the explicit request
    // where supported without excluding those otherwise valid encoders.
    let _ = codec_api.SetValue(
        &CODECAPI_AVEncMPVDefaultBPictureCount,
        &VARIANT::from(0_u32),
    );
    let gop_frames = (u64::from(settings.fps) * u64::from(settings.keyframe_interval_ms) / 1_000)
        .clamp(1, u64::from(u32::MAX)) as u32;
    let _ = codec_api.SetValue(&CODECAPI_AVEncMPVGOPSize, &VARIANT::from(gop_frames));
    let _ = codec_api.SetValue(&CODECAPI_AVEncCommonQualityVsSpeed, &VARIANT::from(100_u32));
    Ok(())
}

unsafe fn force_keyframe(transform: &IMFTransform) -> Result<(), EncoderError> {
    let codec_api: ICodecAPI = transform
        .cast()
        .map_err(|error| backend_error("query ICodecAPI", error))?;
    codec_api
        .SetValue(&CODECAPI_AVEncVideoForceKeyFrame, &VARIANT::from(1_u32))
        .map_err(|error| backend_error("request H.264 keyframe", error))
}

unsafe fn copy_to_media_buffer(buffer: &IMFMediaBuffer, bytes: &[u8]) -> Result<(), EncoderError> {
    let mut destination = std::ptr::null_mut();
    let mut max_length = 0_u32;
    buffer
        .Lock(&mut destination, Some(&mut max_length), None)
        .map_err(|error| backend_error("IMFMediaBuffer::Lock(input)", error))?;
    if destination.is_null() || max_length < bytes.len() as u32 {
        let _ = buffer.Unlock();
        return Err(EncoderError::Backend(
            "Media Foundation input buffer was smaller than the NV12 frame".to_string(),
        ));
    }
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), destination, bytes.len());
    buffer
        .Unlock()
        .and_then(|_| buffer.SetCurrentLength(bytes.len() as u32))
        .map_err(|error| backend_error("commit Media Foundation input buffer", error))
}

unsafe fn read_media_buffer(buffer: &IMFMediaBuffer) -> Result<Vec<u8>, EncoderError> {
    let mut source = std::ptr::null_mut();
    let mut current_length = 0_u32;
    buffer
        .Lock(&mut source, None, Some(&mut current_length))
        .map_err(|error| backend_error("IMFMediaBuffer::Lock(output)", error))?;
    if source.is_null() {
        let _ = buffer.Unlock();
        return Err(EncoderError::Backend(
            "Media Foundation returned a null output buffer".to_string(),
        ));
    }
    let payload = std::slice::from_raw_parts(source, current_length as usize).to_vec();
    buffer
        .Unlock()
        .map_err(|error| backend_error("IMFMediaBuffer::Unlock(output)", error))?;
    Ok(payload)
}

fn bgra_to_nv12(frame: &CapturedFrame, output: &mut [u8]) -> Result<(), EncoderError> {
    let width = frame.width as usize;
    let height = frame.height as usize;
    let expected_bgra = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| EncoderError::InvalidSettings("frame dimensions overflow".to_string()))?;
    let expected_nv12 = nv12_len(frame.width, frame.height)?;
    if frame.bgra.len() != expected_bgra || output.len() != expected_nv12 {
        return Err(EncoderError::DimensionMismatch);
    }

    let (y_plane, uv_plane) = output.split_at_mut(width * height);
    for y in 0..height {
        for x in 0..width {
            let offset = (y * width + x) * 4;
            let b = i32::from(frame.bgra[offset]);
            let g = i32::from(frame.bgra[offset + 1]);
            let r = i32::from(frame.bgra[offset + 2]);
            y_plane[y * width + x] = limited_y(r, g, b);
        }
    }

    for y in (0..height).step_by(2) {
        for x in (0..width).step_by(2) {
            let mut r = 0_i32;
            let mut g = 0_i32;
            let mut b = 0_i32;
            for dy in 0..2 {
                for dx in 0..2 {
                    let offset = ((y + dy) * width + x + dx) * 4;
                    b += i32::from(frame.bgra[offset]);
                    g += i32::from(frame.bgra[offset + 1]);
                    r += i32::from(frame.bgra[offset + 2]);
                }
            }
            r /= 4;
            g /= 4;
            b /= 4;
            let uv_offset = (y / 2) * width + x;
            uv_plane[uv_offset] = limited_u(r, g, b);
            uv_plane[uv_offset + 1] = limited_v(r, g, b);
        }
    }
    Ok(())
}

fn limited_y(r: i32, g: i32, b: i32) -> u8 {
    (16 + ((47 * r + 157 * g + 16 * b + 128) >> 8)).clamp(16, 235) as u8
}

fn limited_u(r: i32, g: i32, b: i32) -> u8 {
    (128 + ((-26 * r - 87 * g + 112 * b + 128) >> 8)).clamp(16, 240) as u8
}

fn limited_v(r: i32, g: i32, b: i32) -> u8 {
    (128 + ((112 * r - 102 * g - 10 * b + 128) >> 8)).clamp(16, 240) as u8
}

fn nv12_len(width: u32, height: u32) -> Result<usize, EncoderError> {
    let pixels = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| EncoderError::InvalidSettings("frame dimensions overflow".to_string()))?;
    pixels
        .checked_add(pixels / 2)
        .ok_or_else(|| EncoderError::InvalidSettings("NV12 buffer size overflow".to_string()))
}

fn pack_ratio(numerator: u32, denominator: u32) -> u64 {
    (u64::from(numerator) << 32) | u64::from(denominator)
}

fn normalize_h264_access_unit(payload: Vec<u8>) -> Vec<u8> {
    if payload.starts_with(&[0, 0, 1]) || payload.starts_with(&[0, 0, 0, 1]) {
        return payload;
    }

    let mut offset = 0_usize;
    let mut annex_b = Vec::with_capacity(payload.len() + 16);
    while offset + 4 <= payload.len() {
        let length = u32::from_be_bytes([
            payload[offset],
            payload[offset + 1],
            payload[offset + 2],
            payload[offset + 3],
        ]) as usize;
        offset += 4;
        if length == 0 || offset + length > payload.len() {
            return payload;
        }
        annex_b.extend_from_slice(&[0, 0, 0, 1]);
        annex_b.extend_from_slice(&payload[offset..offset + length]);
        offset += length;
    }
    if offset == payload.len() && !annex_b.is_empty() {
        annex_b
    } else {
        payload
    }
}

fn annex_b_contains_idr(payload: &[u8]) -> bool {
    let mut index = 0_usize;
    while index + 4 < payload.len() {
        let start_len = if payload[index..].starts_with(&[0, 0, 0, 1]) {
            4
        } else if payload[index..].starts_with(&[0, 0, 1]) {
            3
        } else {
            index += 1;
            continue;
        };
        let nal_index = index + start_len;
        if nal_index < payload.len() && payload[nal_index] & 0x1f == 5 {
            return true;
        }
        index = nal_index.saturating_add(1);
    }
    false
}

fn backend_error(context: &str, error: windows::core::Error) -> EncoderError {
    EncoderError::Backend(format!("{context}: {error}"))
}

fn now_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_micros() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bgra_converts_to_nv12_planes() {
        let frame = CapturedFrame {
            display_id: 0,
            width: 2,
            height: 2,
            capture_timestamp_us: 0,
            bgra: [0, 0, 255, 255].repeat(4),
        };
        let mut output = vec![0; 6];
        bgra_to_nv12(&frame, &mut output).expect("convert");
        assert!(output[..4].iter().all(|value| *value > 16));
        assert_eq!(output.len(), 6);
    }

    #[test]
    fn length_prefixed_h264_becomes_annex_b() {
        let converted = normalize_h264_access_unit(vec![0, 0, 0, 2, 0x65, 0x88]);
        assert_eq!(converted, vec![0, 0, 0, 1, 0x65, 0x88]);
        assert!(annex_b_contains_idr(&converted));
    }

    #[test]
    fn hardware_vendor_is_classified_from_friendly_name() {
        assert_eq!(
            classify_hardware_backend("Intel Quick Sync H.264 Encoder"),
            EncoderBackend::IntelQuickSync
        );
        assert_eq!(
            classify_hardware_backend("NVIDIA NVENC H264 Encoder MFT"),
            EncoderBackend::NvidiaNvenc
        );
        assert_eq!(
            classify_hardware_backend("AMD AMF Video Encoder"),
            EncoderBackend::AmdAmf
        );
        assert_eq!(
            classify_hardware_backend("Vendor GPU Encoder"),
            EncoderBackend::Hardware
        );
    }

    #[test]
    fn runtime_fallback_is_limited_to_auto_selected_hardware_failures() {
        let failure = EncoderError::Backend("driver reset".to_string());
        assert!(should_runtime_fallback(
            EncoderBackend::Auto,
            EncoderBackend::NvidiaNvenc,
            &failure
        ));
        assert!(!should_runtime_fallback(
            EncoderBackend::NvidiaNvenc,
            EncoderBackend::NvidiaNvenc,
            &failure
        ));
        assert!(!should_runtime_fallback(
            EncoderBackend::Auto,
            EncoderBackend::Software,
            &failure
        ));
        assert!(!should_runtime_fallback(
            EncoderBackend::Auto,
            EncoderBackend::NvidiaNvenc,
            &EncoderError::DimensionMismatch
        ));
    }
}
