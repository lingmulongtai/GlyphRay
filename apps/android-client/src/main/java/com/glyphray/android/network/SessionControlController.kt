package com.glyphray.android.network

import android.os.Build
import android.view.KeyEvent
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.io.Closeable
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetSocketAddress
import java.net.SocketTimeoutException
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors

data class SessionControlState(
    val isConnected: Boolean = false,
    val connectedHostName: String? = null,
    val packetsSent: Long = 0,
    val responsesReceived: Long = 0,
    val lastPairingAccepted: Boolean? = null,
    val trustedDeviceId: String? = null,
    val lastRoundTripMs: Long? = null,
    val displays: List<RemoteDisplayDescriptor> = emptyList(),
    val videoSettings: ClientVideoSettings = ClientVideoSettings(),
    val inputSettings: ClientInputSettings = ClientInputSettings(),
    val lastAction: String = "Idle",
    val lastError: String? = null,
) {
    val statusLabel: String
        get() = if (isConnected) "Control channel ready" else "Disconnected"

    val primaryDisplay: RemoteDisplayDescriptor?
        get() = displays.firstOrNull { it.primary } ?: displays.firstOrNull()
}

class SessionControlController : Closeable {
    private val executor: ExecutorService = Executors.newSingleThreadExecutor()
    private val receiverExecutor: ExecutorService = Executors.newSingleThreadExecutor()
    private var client: ControlUdpClient? = null
    @Volatile private var receiving = false

    var state by mutableStateOf(SessionControlState())
        private set

    fun connect(host: DiscoveredHost) {
        executor.execute {
            runCatching {
                client?.close()
                val nextClient = ControlUdpClient().also { it.connect(host) }
                client = nextClient
                state = state.copy(
                    isConnected = true,
                    connectedHostName = host.hostName,
                    lastAction = "Connected to ${host.hostName}",
                    lastError = null,
                )
                startReceiver(nextClient)
            }.onFailure { error ->
                state = state.copy(
                    isConnected = false,
                    lastAction = "Connect failed",
                    lastError = error.message ?: error.javaClass.simpleName,
                )
            }
        }
    }

    fun sendPairingRequest(deviceName: String = defaultDeviceName()) {
        executor.execute {
            sendControl("Pairing request") { client ->
                client.sendPairingRequest(deviceName)
            }
        }
    }

    fun sendLatencyPing() {
        executor.execute {
            sendControl("Latency ping") { client ->
                client.sendLatencyPing()
            }
        }
    }

    fun updateVideoSettings(settings: ClientVideoSettings) {
        state = state.copy(videoSettings = settings)
    }

    fun updateInputSettings(settings: ClientInputSettings) {
        state = state.copy(inputSettings = settings)
    }

    fun sendEncoderConfig() {
        executor.execute {
            val settings = state.videoSettings
            sendControl("Encoder config") { client ->
                client.sendEncoderConfig(settings)
            }
        }
    }

    fun onKeyEvent(event: KeyEvent): Boolean {
        if (!state.inputSettings.bluetoothKeyboardEnabled) {
            return false
        }
        if (event.action != KeyEvent.ACTION_DOWN && event.action != KeyEvent.ACTION_UP) {
            return false
        }
        if (RemoteKeyMapper.toWindowsVirtualKey(event.keyCode) == null) {
            return false
        }
        executor.execute {
            sendControl("Keyboard input") { client ->
                client.sendKeyboardInput(event)
            }
        }
        return true
    }

    fun sendSpecialKey(key: SpecialRemoteKey) {
        executor.execute {
            sendControl("${key.label} key") { client ->
                client.sendSpecialKey(key)
            }
        }
    }

    fun disconnect() {
        executor.execute {
            client?.close()
            client = null
            receiving = false
            state = state.copy(isConnected = false, connectedHostName = null, lastAction = "Disconnected")
        }
    }

    private fun sendControl(label: String, block: (ControlUdpClient) -> Int) {
        val activeClient = client
        if (activeClient == null) {
            state = state.copy(lastAction = "$label skipped", lastError = "Control channel is not connected")
            return
        }

        runCatching {
            block(activeClient)
        }.onSuccess {
            state = state.copy(
                packetsSent = state.packetsSent + 1,
                lastAction = "$label sent",
                lastError = null,
            )
        }.onFailure { error ->
            state = state.copy(
                lastAction = "$label failed",
                lastError = error.message ?: error.javaClass.simpleName,
            )
        }
    }

    private fun startReceiver(activeClient: ControlUdpClient) {
        receiving = true
        receiverExecutor.execute {
            while (receiving && activeClient.isOpen) {
                val response = runCatching {
                    activeClient.receiveControlResponse(timeoutMs = 250)
                }.onFailure { error ->
                    if (activeClient.isOpen) {
                        state = state.copy(lastAction = "Control receive failed", lastError = error.message)
                    }
                }.getOrNull()

                if (response != null) {
                    handleControlMessage(response)
                }
            }
        }
    }

    private fun handleControlMessage(message: ControlProtocolMessage) {
        when (message) {
            is ControlProtocolMessage.PairingResult -> {
                state = state.copy(
                    responsesReceived = state.responsesReceived + 1,
                    lastPairingAccepted = message.accepted,
                    trustedDeviceId = message.trustedDeviceId,
                    lastAction = if (message.accepted) "Pairing accepted" else "Pairing rejected",
                    lastError = message.reason,
                )
                if (message.accepted) {
                    sendEncoderConfig()
                }
            }
            is ControlProtocolMessage.LatencyPong -> {
                val nowUs = android.os.SystemClock.elapsedRealtimeNanos() / 1_000L
                val rttMs = ((nowUs - message.clientSendTimestampUs).coerceAtLeast(0L)) / 1_000L
                state = state.copy(
                    responsesReceived = state.responsesReceived + 1,
                    lastRoundTripMs = rttMs,
                    lastAction = "Latency pong received",
                    lastError = null,
                )
            }
            is ControlProtocolMessage.DisplayInfo -> {
                state = state.copy(
                    responsesReceived = state.responsesReceived + 1,
                    displays = message.displays,
                    lastAction = "Display info received",
                    lastError = null,
                )
            }
        }
    }

    override fun close() {
        receiving = false
        client?.close()
        client = null
        executor.shutdownNow()
        receiverExecutor.shutdownNow()
    }
}

private class ControlUdpClient : Closeable {
    private val socket = DatagramSocket()
    private var remote: InetSocketAddress? = null
    private var nextTransportSequence = 1L
    private var nextFrameSequence = 1L
    @Volatile var isOpen: Boolean = true
        private set

    fun connect(host: DiscoveredHost) {
        remote = host.endpoint
        socket.connect(host.endpoint)
    }

    fun sendPairingRequest(deviceName: String): Int {
        val frame = ProtocolFrameCodec.encodePairingRequest(
            sequence = nextFrameSequence++,
            deviceName = deviceName,
        )
        return sendControl(TransportMessageKind.pairingRequest, frame)
    }

    fun sendLatencyPing(): Int {
        val frame = ProtocolFrameCodec.encodeLatencyPing(sequence = nextFrameSequence++)
        return sendControl(TransportMessageKind.latencyPing, frame)
    }

    fun sendEncoderConfig(settings: ClientVideoSettings): Int {
        val frame = ProtocolFrameCodec.encodeEncoderConfig(
            sequence = nextFrameSequence++,
            settings = settings,
        )
        return sendControl(TransportMessageKind.encoderConfig, frame)
    }

    fun sendKeyboardInput(event: KeyEvent): Int {
        val frame = ProtocolFrameCodec.encodeKeyboardInput(
            sequence = nextFrameSequence++,
            event = event,
        ) ?: return 0
        return sendInput(TransportMessageKind.keyboardInput, frame)
    }

    fun sendSpecialKey(key: SpecialRemoteKey): Int {
        val down = ProtocolFrameCodec.encodeKeyboardInput(
            sequence = nextFrameSequence++,
            scanCode = key.scanCode,
            virtualKey = key.virtualKey,
            pressed = true,
            modifiers = 0,
        )
        val up = ProtocolFrameCodec.encodeKeyboardInput(
            sequence = nextFrameSequence++,
            scanCode = key.scanCode,
            virtualKey = key.virtualKey,
            pressed = false,
            modifiers = 0,
        )
        return sendInput(TransportMessageKind.keyboardInput, down) +
            sendInput(TransportMessageKind.keyboardInput, up)
    }

    private fun sendControl(messageKind: Int, frame: ByteArray): Int {
        val target = remote ?: error("ControlUdpClient is not connected to a host")
        val datagram = TransportPacketCodec.encodeControl(
            sequence = nextTransportSequence++,
            messageKind = messageKind,
            payload = frame,
        )
        socket.send(DatagramPacket(datagram, datagram.size, target))
        return datagram.size
    }

    private fun sendInput(messageKind: Int, frame: ByteArray): Int {
        val target = remote ?: error("ControlUdpClient is not connected to a host")
        val datagram = TransportPacketCodec.encode(
            channel = TransportChannel.Input,
            messageKind = messageKind,
            sequence = nextTransportSequence++,
            timestampUs = android.os.SystemClock.elapsedRealtimeNanos() / 1_000L,
            payload = frame,
        )
        socket.send(DatagramPacket(datagram, datagram.size, target))
        return datagram.size
    }

    fun receiveControlResponse(timeoutMs: Int): ControlProtocolMessage? {
        socket.soTimeout = timeoutMs
        val buffer = ByteArray(65_536)
        val packet = DatagramPacket(buffer, buffer.size)
        return try {
            socket.receive(packet)
            val transportPacket = TransportPacketCodec.decode(buffer, packet.length)
            if (transportPacket.channel != TransportChannel.Control) {
                return null
            }
            ProtocolFrameCodec.decodeFrame(transportPacket.payload).message
        } catch (_: SocketTimeoutException) {
            null
        }
    }

    override fun close() {
        isOpen = false
        socket.close()
    }
}

private fun defaultDeviceName(): String = listOfNotNull(
    "GlyphRay Android",
    Build.MANUFACTURER.takeUnless { it.isNullOrBlank() },
    Build.MODEL.takeUnless { it.isNullOrBlank() },
).joinToString(" ")
