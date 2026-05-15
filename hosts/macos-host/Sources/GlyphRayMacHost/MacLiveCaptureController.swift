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

final class MacLiveCaptureController: NSObject {
    private let sampleQueue = DispatchQueue(label: "com.glyphray.mac.live-capture.samples")
    private var frameCount = 0
    private var activeDisplay: MacDisplayDescriptor?

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

    func stop() async throws {
        #if canImport(ScreenCaptureKit)
        if let stream {
            try await stream.stopCapture()
            self.stream = nil
        }
        activeDisplay = nil
        #endif
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
    }
}
#endif
