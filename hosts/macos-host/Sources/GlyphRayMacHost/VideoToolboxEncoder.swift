import Foundation

#if canImport(VideoToolbox)
import VideoToolbox
#endif

enum MacVideoCodec: String, CaseIterable, Identifiable {
    case h264 = "H.264"
    case hevc = "HEVC"

    var id: String { rawValue }
}

struct MacEncoderSettings {
    let width: Int32
    let height: Int32
    let fps: Int32
    let bitrate: Int
    let codec: MacVideoCodec
    let keyframeIntervalMs: Int = 1_000

    static let lowLatencyPreview = MacEncoderSettings(
        width: 1920,
        height: 1080,
        fps: 60,
        bitrate: 20_000_000,
        codec: .h264
    )
}

struct MacEncodedFrame: Equatable {
    let sequence: Int64
    let presentationTimeUs: Int64
    let isKeyframe: Bool
    let payload: Data

    var byteCount: Int {
        payload.count
    }
}

final class VideoToolboxEncoder {
    private let settings: MacEncoderSettings
    private let onFrame: ((MacEncodedFrame) -> Void)?
    private var nextFrameSequence: Int64 = 1

    #if canImport(VideoToolbox)
    private var session: VTCompressionSession?
    #endif

    init(settings: MacEncoderSettings, onFrame: ((MacEncodedFrame) -> Void)? = nil) {
        self.settings = settings
        self.onFrame = onFrame
    }

    func start() throws {
        #if canImport(VideoToolbox)
        var createdSession: VTCompressionSession?
        let status = VTCompressionSessionCreate(
            allocator: kCFAllocatorDefault,
            width: settings.width,
            height: settings.height,
            codecType: settings.videoCodecType,
            encoderSpecification: nil,
            imageBufferAttributes: nil,
            compressedDataAllocator: nil,
            outputCallback: videoToolboxOutputCallback,
            refcon: Unmanaged.passUnretained(self).toOpaque(),
            compressionSessionOut: &createdSession
        )

        guard status == noErr, let createdSession else {
            throw MacHostError.encoderUnavailable(status)
        }

        VTSessionSetProperty(createdSession, key: kVTCompressionPropertyKey_RealTime, value: kCFBooleanTrue)
        VTSessionSetProperty(createdSession, key: kVTCompressionPropertyKey_AllowFrameReordering, value: kCFBooleanFalse)
        VTSessionSetProperty(createdSession, key: kVTCompressionPropertyKey_AverageBitRate, value: NSNumber(value: settings.bitrate))
        VTSessionSetProperty(createdSession, key: kVTCompressionPropertyKey_ExpectedFrameRate, value: NSNumber(value: settings.fps))
        VTSessionSetProperty(
            createdSession,
            key: kVTCompressionPropertyKey_MaxKeyFrameIntervalDuration,
            value: NSNumber(value: Double(settings.keyframeIntervalMs) / 1_000.0)
        )
        VTCompressionSessionPrepareToEncodeFrames(createdSession)
        session = createdSession
        #else
        throw MacHostError.frameworkUnavailable("VideoToolbox")
        #endif
    }

    func encode(sampleBuffer: CMSampleBuffer) throws {
        #if canImport(VideoToolbox)
        guard let session else {
            throw MacHostError.encoderUnavailable(kVTInvalidSessionErr)
        }
        guard let imageBuffer = CMSampleBufferGetImageBuffer(sampleBuffer) else {
            throw MacHostError.captureUnavailable("ScreenCaptureKit sample did not contain an image buffer")
        }

        let frameRef = MacFrameRef(sequence: nextFrameSequence)
        nextFrameSequence += 1
        let status = VTCompressionSessionEncodeFrame(
            session,
            imageBuffer: imageBuffer,
            presentationTimeStamp: CMSampleBufferGetPresentationTimeStamp(sampleBuffer),
            duration: CMSampleBufferGetDuration(sampleBuffer),
            frameProperties: nil,
            sourceFrameRefcon: Unmanaged.passRetained(frameRef).toOpaque(),
            infoFlagsOut: nil
        )

        guard status == noErr else {
            throw MacHostError.encoderUnavailable(status)
        }
        #else
        throw MacHostError.frameworkUnavailable("VideoToolbox")
        #endif
    }

    func stop() {
        #if canImport(VideoToolbox)
        if let session {
            VTCompressionSessionCompleteFrames(session, untilPresentationTimeStamp: .invalid)
            VTCompressionSessionInvalidate(session)
            self.session = nil
        }
        #endif
    }

    #if canImport(VideoToolbox)
    fileprivate func handleEncodedFrame(
        status: OSStatus,
        sampleBuffer: CMSampleBuffer?,
        frameRef: MacFrameRef?
    ) {
        guard status == noErr, let sampleBuffer, let frameRef else {
            return
        }
        guard let payload = copyPayload(from: sampleBuffer, codec: settings.codec) else {
            return
        }

        let presentationTime = CMSampleBufferGetPresentationTimeStamp(sampleBuffer)
        let seconds = CMTimeGetSeconds(presentationTime)
        let presentationTimeUs = seconds.isFinite ? Int64((seconds * 1_000_000).rounded()) : 0
        onFrame?(
            MacEncodedFrame(
                sequence: frameRef.sequence,
                presentationTimeUs: presentationTimeUs,
                isKeyframe: sampleBuffer.isKeyframe,
                payload: payload
            )
        )
    }
    #endif
}

#if canImport(VideoToolbox)
private final class MacFrameRef {
    let sequence: Int64

    init(sequence: Int64) {
        self.sequence = sequence
    }
}

private let videoToolboxOutputCallback: VTCompressionOutputCallback = {
    outputCallbackRefCon,
    sourceFrameRefCon,
    status,
    _,
    sampleBuffer
    in
    guard let outputCallbackRefCon else {
        return
    }
    let encoder = Unmanaged<VideoToolboxEncoder>
        .fromOpaque(outputCallbackRefCon)
        .takeUnretainedValue()
    let frameRef = sourceFrameRefCon.map {
        Unmanaged<MacFrameRef>.fromOpaque($0).takeRetainedValue()
    }
    encoder.handleEncodedFrame(
        status: status,
        sampleBuffer: sampleBuffer,
        frameRef: frameRef
    )
}

private extension MacEncoderSettings {
    var videoCodecType: CMVideoCodecType {
        switch codec {
        case .h264:
            return kCMVideoCodecType_H264
        case .hevc:
            return kCMVideoCodecType_HEVC
        }
    }
}

private extension CMSampleBuffer {
    var isKeyframe: Bool {
        guard
            let attachments = CMSampleBufferGetSampleAttachmentsArray(
                self,
                createIfNecessary: false
            ) as? [[CFString: Any]],
            let first = attachments.first
        else {
            return true
        }
        return !(first[kCMSampleAttachmentKey_NotSync] as? Bool ?? false)
    }
}

private func copyPayload(from sampleBuffer: CMSampleBuffer, codec: MacVideoCodec) -> Data? {
    switch codec {
    case .h264:
        return copyH264AnnexBPayload(from: sampleBuffer)
    case .hevc:
        return copyLengthPrefixedPayload(from: sampleBuffer)
    }
}

private func copyH264AnnexBPayload(from sampleBuffer: CMSampleBuffer) -> Data? {
    guard let lengthPrefixedPayload = copyLengthPrefixedPayload(from: sampleBuffer) else {
        return nil
    }

    var out = Data()
    if sampleBuffer.isKeyframe {
        appendH264ParameterSets(from: sampleBuffer, to: &out)
    }
    guard appendAnnexBNalUnits(from: lengthPrefixedPayload, to: &out) else {
        return lengthPrefixedPayload
    }
    return out
}

private func copyLengthPrefixedPayload(from sampleBuffer: CMSampleBuffer) -> Data? {
    guard let blockBuffer = CMSampleBufferGetDataBuffer(sampleBuffer) else {
        return nil
    }
    let length = CMBlockBufferGetDataLength(blockBuffer)
    var data = Data(count: length)
    let status = data.withUnsafeMutableBytes { output in
        CMBlockBufferCopyDataBytes(
            blockBuffer,
            atOffset: 0,
            dataLength: length,
            destination: output.baseAddress!
        )
    }
    return status == noErr ? data : nil
}

private func appendH264ParameterSets(from sampleBuffer: CMSampleBuffer, to out: inout Data) {
    guard let formatDescription = CMSampleBufferGetFormatDescription(sampleBuffer) else {
        return
    }

    for index in 0..<2 {
        var parameterSet: UnsafePointer<UInt8>?
        var parameterSetSize = 0
        var parameterSetCount = 0
        var nalUnitHeaderLength = Int32(0)
        let status = CMVideoFormatDescriptionGetH264ParameterSetAtIndex(
            formatDescription,
            parameterSetIndex: index,
            parameterSetPointerOut: &parameterSet,
            parameterSetSizeOut: &parameterSetSize,
            parameterSetCountOut: &parameterSetCount,
            nalUnitHeaderLengthOut: &nalUnitHeaderLength
        )
        guard status == noErr, let parameterSet, parameterSetSize > 0 else {
            continue
        }
        appendAnnexBStartCode(to: &out)
        out.append(parameterSet, count: parameterSetSize)
    }
}

private func appendAnnexBNalUnits(from payload: Data, to out: inout Data) -> Bool {
    let bytes = [UInt8](payload)
    var offset = 0
    var appendedAnyUnit = false

    while offset + 4 <= bytes.count {
        let length = (Int(bytes[offset]) << 24)
            | (Int(bytes[offset + 1]) << 16)
            | (Int(bytes[offset + 2]) << 8)
            | Int(bytes[offset + 3])
        offset += 4

        guard length > 0, offset + length <= bytes.count else {
            return false
        }

        appendAnnexBStartCode(to: &out)
        out.append(contentsOf: bytes[offset..<(offset + length)])
        offset += length
        appendedAnyUnit = true
    }

    return appendedAnyUnit && offset == bytes.count
}

private func appendAnnexBStartCode(to out: inout Data) {
    out.append(contentsOf: [0x00, 0x00, 0x00, 0x01])
}
#endif

enum MacHostError: Error, CustomStringConvertible {
    case frameworkUnavailable(String)
    case encoderUnavailable(OSStatus)
    case captureUnavailable(String)
    case transportUnavailable(String)
    case unsupportedCodec(String)

    var description: String {
        switch self {
        case .frameworkUnavailable(let name):
            return "\(name) is unavailable on this platform"
        case .encoderUnavailable(let status):
            return "VideoToolbox encoder unavailable: \(status)"
        case .captureUnavailable(let message):
            return "Screen capture unavailable: \(message)"
        case .transportUnavailable(let message):
            return "Transport unavailable: \(message)"
        case .unsupportedCodec(let codec):
            return "Unsupported video codec: \(codec)"
        }
    }
}
