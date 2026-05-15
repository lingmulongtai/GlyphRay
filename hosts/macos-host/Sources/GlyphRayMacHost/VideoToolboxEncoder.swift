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

    static let lowLatencyPreview = MacEncoderSettings(
        width: 1920,
        height: 1080,
        fps: 60,
        bitrate: 20_000_000,
        codec: .h264
    )
}

final class VideoToolboxEncoder {
    private let settings: MacEncoderSettings

    #if canImport(VideoToolbox)
    private var session: VTCompressionSession?
    #endif

    init(settings: MacEncoderSettings) {
        self.settings = settings
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
            outputCallback: nil,
            refcon: nil,
            compressionSessionOut: &createdSession
        )

        guard status == noErr, let createdSession else {
            throw MacHostError.encoderUnavailable(status)
        }

        VTSessionSetProperty(createdSession, key: kVTCompressionPropertyKey_RealTime, value: kCFBooleanTrue)
        VTSessionSetProperty(createdSession, key: kVTCompressionPropertyKey_AllowFrameReordering, value: kCFBooleanFalse)
        VTSessionSetProperty(createdSession, key: kVTCompressionPropertyKey_AverageBitRate, value: NSNumber(value: settings.bitrate))
        VTSessionSetProperty(createdSession, key: kVTCompressionPropertyKey_ExpectedFrameRate, value: NSNumber(value: settings.fps))
        VTSessionSetProperty(createdSession, key: kVTCompressionPropertyKey_MaxKeyFrameIntervalDuration, value: NSNumber(value: 1))
        VTCompressionSessionPrepareToEncodeFrames(createdSession)
        session = createdSession
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
}

#if canImport(VideoToolbox)
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
#endif

enum MacHostError: Error, CustomStringConvertible {
    case frameworkUnavailable(String)
    case encoderUnavailable(OSStatus)
    case captureUnavailable(String)

    var description: String {
        switch self {
        case .frameworkUnavailable(let name):
            return "\(name) is unavailable on this platform"
        case .encoderUnavailable(let status):
            return "VideoToolbox encoder unavailable: \(status)"
        case .captureUnavailable(let message):
            return "Screen capture unavailable: \(message)"
        }
    }
}
