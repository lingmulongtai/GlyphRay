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

struct MacLiveUdpSendProbeResult: Equatable {
    let displayID: UInt32
    let width: Int
    let height: Int
    let capturedFrames: Int
    let encodedFrames: Int
    let encodedBytes: Int
    let packetizedDatagrams: Int
    let packetizedBytes: Int
    let sentDatagrams: Int
    let sentBytes: Int
    let target: MacUdpSendTarget
}

struct MacLiveUdpStreamResult: Equatable {
    let displayID: UInt32
    let width: Int
    let height: Int
    let capturedFrames: Int
    let encodedFrames: Int
    let encodedBytes: Int
    let scheduledDatagrams: Int
    let scheduledBytes: Int
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
    private var pendingVideoDatagrams: [MacVideoTransportDatagram] = []
    private var activeDisplay: MacDisplayDescriptor?
    private var activeEncoder: VideoToolboxEncoder?
    private var activePublisher: MacUdpVideoPublisher?

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

    func startFirstDisplayUdpSendProbe(
        to target: MacUdpSendTarget
    ) async throws -> MacLiveUdpSendProbeResult {
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
        pendingVideoDatagrams.removeAll(keepingCapacity: true)

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
                    self.pendingVideoDatagrams.append(contentsOf: report.datagrams)
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

        let datagrams = pendingVideoDatagrams
        pendingVideoDatagrams.removeAll(keepingCapacity: true)
        let sendReport = try await MacUdpDatagramSender().send(datagrams: datagrams, to: target)

        return MacLiveUdpSendProbeResult(
            displayID: descriptor.id,
            width: descriptor.width,
            height: descriptor.height,
            capturedFrames: frameCount,
            encodedFrames: encodedFrameCount,
            encodedBytes: encodedBytes,
            packetizedDatagrams: videoDatagramCount,
            packetizedBytes: transportBytes,
            sentDatagrams: sendReport.datagrams,
            sentBytes: sendReport.bytes,
            target: sendReport.target
        )
        #else
        throw MacHostError.frameworkUnavailable("ScreenCaptureKit")
        #endif
    }

    func startFirstDisplayUdpStream(
        to target: MacUdpSendTarget
    ) async throws -> MacLiveUdpStreamResult {
        #if canImport(ScreenCaptureKit)
        guard stream == nil else {
            throw MacHostError.captureUnavailable("A capture stream is already running")
        }

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
        pendingVideoDatagrams.removeAll(keepingCapacity: true)

        let publisher = try MacUdpVideoPublisher(target: target)
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
                    for datagram in report.datagrams {
                        publisher.publish(datagram)
                    }
                }
            }
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

        do {
            try encoder.start()
            activeEncoder = encoder
            activePublisher = publisher
            activeDisplay = descriptor
            self.stream = stream
            try await stream.startCapture()
        } catch {
            encoder.stop()
            _ = publisher.stop()
            activeEncoder = nil
            activePublisher = nil
            activeDisplay = nil
            self.stream = nil
            throw error
        }

        return udpStreamResult(
            display: descriptor,
            publisher: publisher,
            running: true
        )
        #else
        throw MacHostError.frameworkUnavailable("ScreenCaptureKit")
        #endif
    }

    func stopUdpStream() async throws -> MacLiveUdpStreamResult {
        guard let display = activeDisplay, let publisher = activePublisher else {
            throw MacHostError.transportUnavailable("No active UDP video stream")
        }

        try await stop()
        activeEncoder?.stop()
        activeEncoder = nil
        activePublisher = nil
        let result = udpStreamResult(
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
        display: MacDisplayDescriptor,
        publisher: MacUdpVideoPublisher,
        running: Bool
    ) -> MacLiveUdpStreamResult {
        let snapshot = publisher.snapshot()
        return MacLiveUdpStreamResult(
            displayID: display.id,
            width: display.width,
            height: display.height,
            capturedFrames: frameCount,
            encodedFrames: encodedFrameCount,
            encodedBytes: encodedBytes,
            scheduledDatagrams: snapshot.scheduledDatagrams,
            scheduledBytes: snapshot.scheduledBytes,
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
