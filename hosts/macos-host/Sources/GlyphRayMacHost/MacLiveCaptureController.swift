import Foundation

#if canImport(CoreMedia)
import CoreMedia
#endif

#if canImport(ScreenCaptureKit)
import ScreenCaptureKit
#endif

struct MacLiveCaptureProbeResult: Equatable {
    let displayID: UInt32
    let width: Int
    let height: Int
    let frameCount: Int
}

struct MacLiveEncodeProbeResult: Equatable {
    let displayID: UInt32
    let width: Int
    let height: Int
    let capturedFrames: Int
    let encodedFrames: Int
    let encodedBytes: Int
}

struct MacLiveTransportProbeResult: Equatable {
    let displayID: UInt32
    let width: Int
    let height: Int
    let capturedFrames: Int
    let encodedFrames: Int
    let encodedBytes: Int
    let videoDatagrams: Int
    let transportBytes: Int
}

struct MacLiveUdpStreamResult: Equatable {
    let streamID: UUID
    let displayID: UInt32
    let width: Int
    let height: Int
    let capturedFrames: Int
    let encodedFrames: Int
    let encodedBytes: Int
    let scheduledDatagrams: Int
    let scheduledBytes: Int
    let sentDatagrams: Int
    let sentBytes: Int
    let droppedDatagrams: Int
    let droppedBytes: Int
    let inFlightDatagrams: Int
    let highWatermarkDatagrams: Int
    let publisherError: String?
    let backpressureLimited: Bool
    let reconnectCount: Int
    let running: Bool
    let target: MacUdpSendTarget
}

final class MacLiveCaptureController: NSObject {
    private let sampleQueue = DispatchQueue(label: "com.glyphray.mac.live-capture.samples")
    private var frameCount = 0
    private var encodedFrameCount = 0
    private var encodedBytes = 0
    private var videoDatagramCount = 0
    private var transportBytes = 0
    private var activeDisplay: MacDisplayDescriptor?
    private var activeEncoder: VideoToolboxEncoder?
    private var activePublisher: MacUdpVideoPublisher?
    private var activeStreamID: UUID?
    private var activeStreamTarget: MacUdpSendTarget?
    private var reconnectCount = 0

    #if canImport(ScreenCaptureKit)
    private var stream: SCStream?
    #endif

    func startFirstDisplayProbe() async throws -> MacLiveCaptureProbeResult {
        #if canImport(ScreenCaptureKit)
        let content = try await SCShareableContent.current
        guard let display = content.displays.first else {
            throw MacHostError.captureUnavailable("No capture displays are available")
        }

        let descriptor = MacDisplayDescriptor(
            id: display.displayID,
            width: display.width,
            height: display.height,
            originX: Int(display.frame.origin.x),
            originY: Int(display.frame.origin.y)
        )
        let configuration = SCStreamConfiguration()
        configuration.width = display.width
        configuration.height = display.height
        configuration.queueDepth = 3
        configuration.showsCursor = true
        configuration.minimumFrameInterval = CMTime(value: 1, timescale: 60)

        let filter = SCContentFilter(display: display, excludingWindows: [])
        let stream = SCStream(filter: filter, configuration: configuration, delegate: nil)
        try stream.addStreamOutput(self, type: .screen, sampleHandlerQueue: sampleQueue)

        frameCount = 0
        activeDisplay = descriptor
        self.stream = stream
        try await stream.startCapture()

        try await Task.sleep(nanoseconds: 500_000_000)
        let result = MacLiveCaptureProbeResult(
            displayID: descriptor.id,
            width: descriptor.width,
            height: descriptor.height,
            frameCount: frameCount
        )
        try await stop()
        return result
        #else
        throw MacHostError.frameworkUnavailable("ScreenCaptureKit")
        #endif
    }

    func startFirstDisplayEncodeProbe() async throws -> MacLiveEncodeProbeResult {
        #if canImport(ScreenCaptureKit)
        let content = try await SCShareableContent.current
        guard let display = content.displays.first else {
            throw MacHostError.captureUnavailable("No capture displays are available")
        }

        let descriptor = MacDisplayDescriptor(
            id: display.displayID,
            width: display.width,
            height: display.height,
            originX: Int(display.frame.origin.x),
            originY: Int(display.frame.origin.y)
        )
        frameCount = 0
        encodedFrameCount = 0
        encodedBytes = 0

        let encoder = VideoToolboxEncoder(
            settings: MacEncoderSettings(
                width: Int32(display.width),
                height: Int32(display.height),
                fps: 60,
                bitrate: 20_000_000,
                codec: .h264
            ),
            onFrame: { [weak self] frame in
                self?.encodedFrameCount += 1
                self?.encodedBytes += frame.byteCount
            }
        )
        try encoder.start()
        activeEncoder = encoder

        let configuration = SCStreamConfiguration()
        configuration.width = display.width
        configuration.height = display.height
        configuration.queueDepth = 3
        configuration.showsCursor = true
        configuration.minimumFrameInterval = CMTime(value: 1, timescale: 60)

        let filter = SCContentFilter(display: display, excludingWindows: [])
        let stream = SCStream(filter: filter, configuration: configuration, delegate: nil)
        try stream.addStreamOutput(self, type: .screen, sampleHandlerQueue: sampleQueue)

        activeDisplay = descriptor
        self.stream = stream
        try await stream.startCapture()
        try await Task.sleep(nanoseconds: 900_000_000)

        try await stop()
        encoder.stop()
        activeEncoder = nil

        return MacLiveEncodeProbeResult(
            displayID: descriptor.id,
            width: descriptor.width,
            height: descriptor.height,
            capturedFrames: frameCount,
            encodedFrames: encodedFrameCount,
            encodedBytes: encodedBytes
        )
        #else
        throw MacHostError.frameworkUnavailable("ScreenCaptureKit")
        #endif
    }

    func startFirstDisplayTransportProbe() async throws -> MacLiveTransportProbeResult {
        #if canImport(ScreenCaptureKit)
        let content = try await SCShareableContent.current
        guard let display = content.displays.first else {
            throw MacHostError.captureUnavailable("No capture displays are available")
        }

        let descriptor = MacDisplayDescriptor(
            id: display.displayID,
            width: display.width,
            height: display.height,
            originX: Int(display.frame.origin.x),
            originY: Int(display.frame.origin.y)
        )
        frameCount = 0
        encodedFrameCount = 0
        encodedBytes = 0
        videoDatagramCount = 0
        transportBytes = 0

        let packetizer = MacVideoTransportPacketizer()
        let encoder = VideoToolboxEncoder(
            settings: MacEncoderSettings(
                width: Int32(display.width),
                height: Int32(display.height),
                fps: 60,
                bitrate: 20_000_000,
                codec: .h264
            ),
            onFrame: { [weak self] frame in
                guard let self else {
                    return
                }
                self.encodedFrameCount += 1
                self.encodedBytes += frame.byteCount
                if let report = try? packetizer.packetize(frame: frame) {
                    self.videoDatagramCount += report.datagramCount
                    self.transportBytes += report.byteCount
                }
            }
        )
        try encoder.start()
        activeEncoder = encoder

        let configuration = SCStreamConfiguration()
        configuration.width = display.width
        configuration.height = display.height
        configuration.queueDepth = 3
        configuration.showsCursor = true
        configuration.minimumFrameInterval = CMTime(value: 1, timescale: 60)

        let filter = SCContentFilter(display: display, excludingWindows: [])
        let stream = SCStream(filter: filter, configuration: configuration, delegate: nil)
        try stream.addStreamOutput(self, type: .screen, sampleHandlerQueue: sampleQueue)

        activeDisplay = descriptor
        self.stream = stream
        try await stream.startCapture()
        try await Task.sleep(nanoseconds: 900_000_000)

        try await stop()
        encoder.stop()
        activeEncoder = nil

        return MacLiveTransportProbeResult(
            displayID: descriptor.id,
            width: descriptor.width,
            height: descriptor.height,
            capturedFrames: frameCount,
            encodedFrames: encodedFrameCount,
            encodedBytes: encodedBytes,
            videoDatagrams: videoDatagramCount,
            transportBytes: transportBytes
        )
        #else
        throw MacHostError.frameworkUnavailable("ScreenCaptureKit")
        #endif
    }

    func startFirstDisplayUdpStream(
        to target: MacUdpSendTarget,
        preference: MacClientVideoPreference?,
        transformDatagram: @escaping (Data) throws -> Data
    ) async throws -> MacLiveUdpStreamResult {
        #if canImport(ScreenCaptureKit)
        if stream != nil {
            if activeStreamTarget == target {
                throw MacHostError.captureUnavailable("A capture stream is already running for \(target.host):\(target.port)")
            }
            _ = try await stopUdpStream()
            reconnectCount += 1
        }

        let content = try await SCShareableContent.current
        let requestedDisplay = preference.flatMap { requested in
            content.displays.first { $0.displayID == requested.displayID }
        }
        guard let display = requestedDisplay ?? content.displays.first else {
            throw MacHostError.captureUnavailable("No capture displays are available")
        }
        if let preference, preference.codec != 0 {
            throw MacHostError.unsupportedCodec("macOS live sessions currently support H.264 only")
        }
        let outputWidth = evenDimension(
            preference.map { min(Int($0.width), display.width) } ?? display.width
        )
        let outputHeight = evenDimension(
            preference.map { min(Int($0.height), display.height) } ?? display.height
        )
        let outputFPS = Int32(min(max(Int(preference?.maxFPS ?? 60), 1), 120))
        let bitrateKbps = min(max(Int(preference?.targetBitrateKbps ?? 20_000), 1_000), 100_000)
        let keyframeIntervalMs = min(
            max(Int(preference?.keyframeIntervalMs ?? 1_000), 250),
            10_000
        )

        let descriptor = MacDisplayDescriptor(
            id: display.displayID,
            width: outputWidth,
            height: outputHeight,
            originX: Int(display.frame.origin.x),
            originY: Int(display.frame.origin.y)
        )
        frameCount = 0
        encodedFrameCount = 0
        encodedBytes = 0
        videoDatagramCount = 0
        transportBytes = 0
        let streamID = UUID()

        let publisher = try MacUdpVideoPublisher(
            target: target,
            transformDatagram: transformDatagram
        )
        let packetizer = MacVideoTransportPacketizer()
        let encoder = VideoToolboxEncoder(
            settings: MacEncoderSettings(
                width: Int32(outputWidth),
                height: Int32(outputHeight),
                fps: outputFPS,
                bitrate: bitrateKbps * 1_000,
                codec: .h264,
                keyframeIntervalMs: keyframeIntervalMs
            ),
            onFrame: { [weak self] frame in
                guard let self else {
                    return
                }
                self.encodedFrameCount += 1
                self.encodedBytes += frame.byteCount
                if let report = try? packetizer.packetize(frame: frame) {
                    self.videoDatagramCount += report.datagramCount
                    self.transportBytes += report.byteCount
                    for datagram in report.datagrams {
                        publisher.publish(datagram)
                    }
                }
            }
        )

        let configuration = SCStreamConfiguration()
        configuration.width = outputWidth
        configuration.height = outputHeight
        configuration.queueDepth = 3
        configuration.showsCursor = true
        configuration.minimumFrameInterval = CMTime(value: 1, timescale: outputFPS)

        let filter = SCContentFilter(display: display, excludingWindows: [])
        let stream = SCStream(filter: filter, configuration: configuration, delegate: nil)
        try stream.addStreamOutput(self, type: .screen, sampleHandlerQueue: sampleQueue)

        do {
            try encoder.start()
            activeEncoder = encoder
            activePublisher = publisher
            activeStreamID = streamID
            activeStreamTarget = target
            activeDisplay = descriptor
            self.stream = stream
            try await stream.startCapture()
        } catch {
            encoder.stop()
            _ = publisher.stop()
            activeEncoder = nil
            activePublisher = nil
            activeStreamID = nil
            activeStreamTarget = nil
            activeDisplay = nil
            self.stream = nil
            throw error
        }

        return udpStreamResult(
            streamID: streamID,
            display: descriptor,
            publisher: publisher,
            running: true
        )
        #else
        throw MacHostError.frameworkUnavailable("ScreenCaptureKit")
        #endif
    }

    private func evenDimension(_ value: Int) -> Int {
        let bounded = max(2, value)
        return bounded - (bounded % 2)
    }

    func stopUdpStream() async throws -> MacLiveUdpStreamResult {
        guard let display = activeDisplay, let publisher = activePublisher else {
            throw MacHostError.transportUnavailable("No active UDP video stream")
        }
        let streamID = activeStreamID ?? UUID()

        try await stop()
        activeEncoder?.stop()
        activeEncoder = nil
        activePublisher = nil
        activeStreamID = nil
        activeStreamTarget = nil
        let result = udpStreamResult(
            streamID: streamID,
            display: display,
            publisher: publisher,
            running: false
        )
        _ = publisher.stop()
        return result
    }

    func stop() async throws {
        #if canImport(ScreenCaptureKit)
        if let stream {
            try await stream.stopCapture()
            self.stream = nil
        }
        activeDisplay = nil
        #endif
    }

    private func udpStreamResult(
        streamID: UUID,
        display: MacDisplayDescriptor,
        publisher: MacUdpVideoPublisher,
        running: Bool
    ) -> MacLiveUdpStreamResult {
        let snapshot = publisher.snapshot()
        return MacLiveUdpStreamResult(
            streamID: streamID,
            displayID: display.id,
            width: display.width,
            height: display.height,
            capturedFrames: frameCount,
            encodedFrames: encodedFrameCount,
            encodedBytes: encodedBytes,
            scheduledDatagrams: snapshot.scheduledDatagrams,
            scheduledBytes: snapshot.scheduledBytes,
            sentDatagrams: snapshot.sentDatagrams,
            sentBytes: snapshot.sentBytes,
            droppedDatagrams: snapshot.droppedDatagrams,
            droppedBytes: snapshot.droppedBytes,
            inFlightDatagrams: snapshot.inFlightDatagrams,
            highWatermarkDatagrams: snapshot.highWatermarkDatagrams,
            publisherError: snapshot.lastError,
            backpressureLimited: snapshot.inFlightDatagrams >= snapshot.maxInFlightDatagrams
                || snapshot.lastError?.contains("backlog") == true,
            reconnectCount: reconnectCount,
            running: running,
            target: snapshot.target
        )
    }
}

#if canImport(ScreenCaptureKit)
extension MacLiveCaptureController: SCStreamOutput {
    func stream(
        _ stream: SCStream,
        didOutputSampleBuffer sampleBuffer: CMSampleBuffer,
        of type: SCStreamOutputType
    ) {
        guard type == .screen, CMSampleBufferIsValid(sampleBuffer) else {
            return
        }
        frameCount += 1
        do {
            try activeEncoder?.encode(sampleBuffer: sampleBuffer)
        } catch {
            activeEncoder = nil
        }
    }
}
#endif
