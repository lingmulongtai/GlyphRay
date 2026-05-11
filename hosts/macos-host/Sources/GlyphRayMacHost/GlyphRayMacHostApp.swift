import SwiftUI

@main
struct GlyphRayMacHostApp: App {
    @StateObject private var model = HostStatusModel()

    var body: some Scene {
        WindowGroup {
            ContentView(model: model)
        }
    }
}

final class HostStatusModel: ObservableObject {
    @Published var status: String = "Idle"
    @Published var captureStatus: String = "Capture idle"
    @Published var encoderStatus: String = "Encoder idle"
}

struct ContentView: View {
    @ObservedObject var model: HostStatusModel

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("GlyphRay Host")
                .font(.largeTitle)
                .fontWeight(.semibold)
            Text(model.status)
                .foregroundStyle(.secondary)
            Divider()
            Label(model.captureStatus, systemImage: "display")
            Label(model.encoderStatus, systemImage: "video")
            Label("Mouse and keyboard input first; Windows Ink-style pen injection is Windows-specific.", systemImage: "hand.draw")
        }
        .padding(24)
        .frame(minWidth: 520, minHeight: 260)
    }
}
