package com.glyphray.android.network

import android.os.Build
import android.view.KeyEvent
import android.view.MotionEvent
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
import com.glyphray.android.audio.RemoteAudioStreamController
import com.glyphray.android.security.AndroidDeviceKeys
import com.glyphray.android.security.TrustedHostIdentityStore
import com.glyphray.android.input.StylusStreamPacket
import com.glyphray.android.video.RemoteVideoStreamController

interface SessionRealtimeInputSender {
    val isInputTransportReady: Boolean

    fun sendStylus(packet: StylusStreamPacket): Int

    fun sendEncodedInput(messageKind: Int, frame: ByteArray): Int
}

data class SessionControlState(
    val isConnected: Boolean = false,
    val connectedHostName: String? = null,
    val packetsSent: Long = 0,
    val responsesReceived: Long = 0,
    val lastPairingAccepted: Boolean? = null,
    val pairingChallenge: ControlProtocolMessage.PairingChallenge? = null,
    val trustedDeviceId: String? = null,
    val lastRoundTripMs: Long? = null,
    val displays: List<RemoteDisplayDescriptor> = emptyList(),
    val videoPacketsReceived: Long = 0,
    val videoFramesCompleted: Long = 0,
    val videoFramesQueuedToDecoder: Long = 0,
    val lastVideoSequence: Long? = null,
    val audioPacketsReceived: Long = 0,
    val audioBytesQueued: Long = 0,
    val lastAudioSequence: Long? = null,
    val secureSession: Boolean = false,
    val hostIdentityFingerprint: String? = null,
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

class SessionControlController(
    initialVideoSettings: ClientVideoSettings = ClientVideoSettings(),
    initialInputSettings: ClientInputSettings = ClientInputSettings(),
    private val trustedHostIdentityStore: TrustedHostIdentityStore? = null,
) : Closeable, SessionRealtimeInputSender {
    private val executor: ExecutorService = Executors.newSingleThreadExecutor()
    private val receiverExecutor: ExecutorService = Executors.newSingleThreadExecutor()
    private var client: ControlUdpClient? = null
    private var gamepadButtons: Int = 0
    private val audioStreamController = RemoteAudioStreamController()
    @Volatile private var receiving = false
    @Volatile private var videoStreamController: RemoteVideoStreamController? = null

    var state by mutableStateOf(
        SessionControlState(
            videoSettings = initialVideoSettings,
            inputSettings = initialInputSettings,
        ),
    )
        private set

    fun connect(host: DiscoveredHost) {
        executor.execute {
            runCatching {
                client?.close()
                val nextClient = ControlUdpClient(trustedHostIdentityStore).also { it.connect(host) }
                client = nextClient
                state = state.copy(
                    isConnected = true,
                    connectedHostName = host.hostName,
                    secureSession = false,
                    pairingChallenge = null,
                    hostIdentityFingerprint = null,
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

    fun submitPairingCode(code: String, deviceName: String = defaultDeviceName()) {
        executor.execute {
            val challenge = state.pairingChallenge
            if (challenge == null) {
                state = state.copy(lastAction = "Pairing code unavailable", lastError = "Request a new pairing code first")
                return@execute
            }
            if (System.currentTimeMillis() > challenge.expiresAtUnixMs) {
                state = state.copy(pairingChallenge = null, lastAction = "Pairing code expired", lastError = "Request a new pairing code")
                return@execute
            }
            sendControl("Pairing code proof") { client ->
                client.sendPairingRequest(
                    deviceName = deviceName,
                    pairingCodeHash = PairingCodeProof.create(code, challenge.salt),
                )
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
        if (RemoteGamepadMapper.isGamepadEvent(event.source)) {
            return onGamepadKeyEvent(event)
        }
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

    fun onGenericMotionEvent(event: MotionEvent): Boolean {
        if (!RemoteGamepadMapper.isGamepadEvent(event.source)) {
            return false
        }
        if (!state.inputSettings.gameControllerEnabled) {
            return false
        }
        executor.execute {
            sendControl("Gamepad input") { client ->
                client.sendGamepadInput(
                    controllerId = event.deviceId,
                    buttons = gamepadButtons,
                    leftTrigger = RemoteGamepadMapper.axis(event, MotionEvent.AXIS_LTRIGGER, MotionEvent.AXIS_BRAKE),
                    rightTrigger = RemoteGamepadMapper.axis(event, MotionEvent.AXIS_RTRIGGER, MotionEvent.AXIS_GAS),
                    leftStickX = RemoteGamepadMapper.axis(event, MotionEvent.AXIS_X),
                    leftStickY = RemoteGamepadMapper.axis(event, MotionEvent.AXIS_Y),
                    rightStickX = RemoteGamepadMapper.axis(event, MotionEvent.AXIS_Z, MotionEvent.AXIS_RX),
                    rightStickY = RemoteGamepadMapper.axis(event, MotionEvent.AXIS_RZ, MotionEvent.AXIS_RY),
                )
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

    fun attachVideoStreamController(controller: RemoteVideoStreamController?) {
        videoStreamController = controller
    }

    override val isInputTransportReady: Boolean
        get() = client?.isSecure == true

    override fun sendStylus(packet: StylusStreamPacket): Int {
        return client?.sendStylus(packet)
            ?: error("Realtime session transport is not connected")
    }

    override fun sendEncodedInput(messageKind: Int, frame: ByteArray): Int {
        return client?.sendEncodedInput(messageKind, frame)
            ?: error("Realtime session transport is not connected")
    }

    private fun onGamepadKeyEvent(event: KeyEvent): Boolean {
        if (!state.inputSettings.gameControllerEnabled) {
            return false
        }
        val bit = RemoteGamepadMapper.buttonBit(event.keyCode) ?: return false
        if (event.action == KeyEvent.ACTION_DOWN) {
            gamepadButtons = gamepadButtons or bit
        } else if (event.action == KeyEvent.ACTION_UP) {
            gamepadButtons = gamepadButtons and bit.inv()
        } else {
            return false
        }
        executor.execute {
            sendControl("Gamepad button") { client ->
                client.sendGamepadInput(
                    controllerId = event.deviceId,
                    buttons = gamepadButtons,
                    leftTrigger = 0f,
                    rightTrigger = 0f,
                    leftStickX = 0f,
                    leftStickY = 0f,
                    rightStickX = 0f,
                    rightStickY = 0f,
                )
            }
        }
        return true
    }

    fun disconnect() {
        executor.execute {
            client?.close()
            client = null
            receiving = false
            audioStreamController.release()
            state = state.copy(
                isConnected = false,
                connectedHostName = null,
                secureSession = false,
                hostIdentityFingerprint = null,
                displays = emptyList(),
                audioPacketsReceived = 0,
                audioBytesQueued = 0,
                lastAudioSequence = null,
                lastAction = "Disconnected",
                lastError = null,
            )
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
                val packet = runCatching {
                    activeClient.receiveTransportPacket(timeoutMs = 250)
                }.onFailure { error ->
                    if (activeClient.isOpen) {
                        state = state.copy(lastAction = "Control receive failed", lastError = error.message)
                    }
                }.getOrNull()

                if (packet != null) {
                    handleTransportPacket(packet)
                }
            }
        }
    }

    private fun handleTransportPacket(packet: DecodedTransportPacket) {
        when (packet.channel) {
            TransportChannel.Control -> {
                if (packet.messageKind == TransportMessageKind.sessionKeyExchange) {
                    handleSessionKeyExchange(packet.payload)
                } else {
                    handleControlMessage(ProtocolFrameCodec.decodeFrame(packet.payload).message)
                }
            }
            TransportChannel.Video -> {
                if (packet.messageKind != TransportMessageKind.videoFrame) {
                    return
                }
                val result = runCatching {
                    videoStreamController?.onVideoFragment(packet.payload)
                }.getOrElse { error ->
                    state = state.copy(lastAction = "Video receive failed", lastError = error.message)
                    return
                }
                state = state.copy(
                    videoPacketsReceived = state.videoPacketsReceived + 1,
                    videoFramesCompleted = state.videoFramesCompleted + if (result?.completedFrame == true) 1 else 0,
                    videoFramesQueuedToDecoder = state.videoFramesQueuedToDecoder + if (result?.queuedToDecoder == true) 1 else 0,
                    lastVideoSequence = result?.frameSequence ?: state.lastVideoSequence,
                    lastAction = if (result?.completedFrame == true) "Video frame received" else "Video fragment received",
                    lastError = null,
                )
            }
            TransportChannel.Audio -> {
                if (packet.messageKind != TransportMessageKind.audioFrame) {
                    return
                }
                val result = audioStreamController.onAudioFrame(packet.payload)
                state = state.copy(
                    audioPacketsReceived = state.audioPacketsReceived + 1,
                    audioBytesQueued = state.audioBytesQueued + result.queuedBytes,
                    lastAudioSequence = result.frameSequence ?: state.lastAudioSequence,
                    lastAction = if (result.accepted) "Audio frame queued" else "Audio frame skipped",
                    lastError = result.reason,
                )
            }
            TransportChannel.Input -> Unit
        }
    }

    private fun handleSessionKeyExchange(payload: ByteArray) {
        executor.execute {
            sendControl("Secure session") { client ->
                val fingerprint = client.establishSecureSession(payload)
                state = state.copy(
                    secureSession = true,
                    hostIdentityFingerprint = fingerprint,
                    responsesReceived = state.responsesReceived + 1,
                    lastAction = "Encrypted session established",
                    lastError = null,
                )
                client.sendEncoderConfig(state.videoSettings)
            }
        }
    }

    private fun handleControlMessage(message: ControlProtocolMessage) {
        when (message) {
            is ControlProtocolMessage.PairingChallenge -> {
                state = state.copy(
                    responsesReceived = state.responsesReceived + 1,
                    pairingChallenge = message,
                    lastPairingAccepted = null,
                    lastAction = "Enter the code shown on the host",
                    lastError = null,
                )
            }
            is ControlProtocolMessage.AuthChallenge -> {
                state = state.copy(
                    responsesReceived = state.responsesReceived + 1,
                    lastAction = "Trusted auth challenge received",
                    lastError = null,
                )
                executor.execute {
                    sendControl("Trusted auth response") { client ->
                        client.sendAuthResponse(message)
                    }
                }
            }
            is ControlProtocolMessage.PairingResult -> {
                state = state.copy(
                    responsesReceived = state.responsesReceived + 1,
                    lastPairingAccepted = message.accepted,
                    pairingChallenge = null,
                    trustedDeviceId = message.trustedDeviceId,
                    lastAction = if (message.accepted) "Pairing accepted" else "Pairing rejected",
                    lastError = message.reason,
                )
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
            is ControlProtocolMessage.AudioFrame -> {
                state = state.copy(
                    lastAction = "Audio frame ignored on control channel",
                    lastError = "Audio frames must arrive on the audio transport channel",
                )
            }
        }
    }

    override fun close() {
        receiving = false
        client?.close()
        client = null
        audioStreamController.release()
        executor.shutdownNow()
        receiverExecutor.shutdownNow()
    }
}

private class ControlUdpClient(
    private val trustedHostIdentityStore: TrustedHostIdentityStore?,
) : Closeable {
    private val socket = DatagramSocket()
    private val deviceKeys = AndroidDeviceKeys()
    private var remote: InetSocketAddress? = null
    private var nextTransportSequence = 1L
    private var nextFrameSequence = 1L
    private var connectedHostId: String? = null
    @Volatile private var secureCodec: SecureDatagramCodec? = null
    @Volatile var isOpen: Boolean = true
        private set
    val isSecure: Boolean
        get() = secureCodec != null

    fun connect(host: DiscoveredHost) {
        remote = host.endpoint
        connectedHostId = host.hostId
        secureCodec = null
        socket.connect(host.endpoint)
    }

    @Synchronized
    fun establishSecureSession(encodedExchange: ByteArray): String {
        val target = remote ?: error("ControlUdpClient is not connected to a host")
        val proposal = AndroidSessionKeyHandshake.begin(
            encodedServerExchange = encodedExchange,
            deviceId = deviceKeys.trustedDeviceId(),
            signClientPayload = deviceKeys::signSessionPayload,
        )
        connectedHostId?.let { hostId ->
            trustedHostIdentityStore?.verifyOrTrust(hostId, proposal.hostIdentityFingerprint)
        }
        val confirmDatagram = TransportPacketCodec.encodeControl(
            sequence = nextTransportSequence++,
            messageKind = TransportMessageKind.sessionKeyConfirm,
            payload = proposal.encodedClientConfirm,
        )
        socket.send(DatagramPacket(confirmDatagram, confirmDatagram.size, target))
        secureCodec = proposal.codec
        return proposal.hostIdentityFingerprint
    }

    @Synchronized
    fun sendPairingRequest(
        deviceName: String,
        pairingCodeHash: ByteArray = ByteArray(0),
    ): Int {
        val frame = ProtocolFrameCodec.encodePairingRequest(
            sequence = nextFrameSequence++,
            deviceName = deviceName,
            pairingCodeHash = pairingCodeHash,
            oneTimePublicKey = runCatching { deviceKeys.publicKeyBytes() }.getOrDefault(ByteArray(0)),
        )
        return sendControl(TransportMessageKind.pairingRequest, frame)
    }

    @Synchronized
    fun sendAuthResponse(challenge: ControlProtocolMessage.AuthChallenge): Int {
        val trustedDeviceId = deviceKeys.trustedDeviceId()
        val signature = deviceKeys.signTrustedChallenge(
            challengeId = challenge.challengeId,
            nonce = challenge.nonce,
            trustedDeviceId = trustedDeviceId,
        )
        val frame = ProtocolFrameCodec.encodeAuthResponse(
            sequence = nextFrameSequence++,
            challengeId = challenge.challengeId,
            deviceId = trustedDeviceId,
            signature = signature,
        )
        return sendControl(TransportMessageKind.authResponse, frame)
    }

    @Synchronized
    fun sendLatencyPing(): Int {
        val frame = ProtocolFrameCodec.encodeLatencyPing(sequence = nextFrameSequence++)
        return sendControl(TransportMessageKind.latencyPing, frame)
    }

    @Synchronized
    fun sendEncoderConfig(settings: ClientVideoSettings): Int {
        val frame = ProtocolFrameCodec.encodeEncoderConfig(
            sequence = nextFrameSequence++,
            settings = settings,
        )
        return sendControl(TransportMessageKind.encoderConfig, frame)
    }

    @Synchronized
    fun sendKeyboardInput(event: KeyEvent): Int {
        val frame = ProtocolFrameCodec.encodeKeyboardInput(
            sequence = nextFrameSequence++,
            event = event,
        ) ?: return 0
        return sendInput(TransportMessageKind.keyboardInput, frame)
    }

    @Synchronized
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

    @Synchronized
    fun sendGamepadInput(
        controllerId: Int,
        buttons: Int,
        leftTrigger: Float,
        rightTrigger: Float,
        leftStickX: Float,
        leftStickY: Float,
        rightStickX: Float,
        rightStickY: Float,
    ): Int {
        val frame = ProtocolFrameCodec.encodeGamepadInput(
            sequence = nextFrameSequence++,
            controllerId = controllerId,
            buttons = buttons,
            leftTrigger = leftTrigger,
            rightTrigger = rightTrigger,
            leftStickX = leftStickX,
            leftStickY = leftStickY,
            rightStickX = rightStickX,
            rightStickY = rightStickY,
        )
        return sendInput(TransportMessageKind.gamepadInput, frame)
    }

    @Synchronized
    fun sendStylus(packet: StylusStreamPacket): Int {
        val target = remote ?: error("ControlUdpClient is not connected to a host")
        val datagram = TransportPacketCodec.encodeStylusInput(
            sequence = nextTransportSequence++,
            packet = packet,
        )
        return sendEncryptedSessionDatagram(target, datagram)
    }

    @Synchronized
    fun sendEncodedInput(messageKind: Int, frame: ByteArray): Int {
        return sendInput(messageKind, frame)
    }

    private fun sendControl(messageKind: Int, frame: ByteArray): Int {
        val target = remote ?: error("ControlUdpClient is not connected to a host")
        val datagram = TransportPacketCodec.encodeControl(
            sequence = nextTransportSequence++,
            messageKind = messageKind,
            payload = frame,
        )
        return sendSessionDatagram(target, datagram)
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
        return sendEncryptedSessionDatagram(target, datagram)
    }

    private fun sendSessionDatagram(target: InetSocketAddress, plaintext: ByteArray): Int {
        val datagram = secureCodec?.seal(plaintext) ?: plaintext
        socket.send(DatagramPacket(datagram, datagram.size, target))
        return datagram.size
    }

    private fun sendEncryptedSessionDatagram(target: InetSocketAddress, plaintext: ByteArray): Int {
        val codec = secureCodec ?: error("Realtime input requires an encrypted session")
        val datagram = codec.seal(plaintext)
        socket.send(DatagramPacket(datagram, datagram.size, target))
        return datagram.size
    }

    fun receiveTransportPacket(timeoutMs: Int): DecodedTransportPacket? {
        socket.soTimeout = timeoutMs
        val buffer = ByteArray(65_536)
        val packet = DatagramPacket(buffer, buffer.size)
        return try {
            socket.receive(packet)
            val datagram = buffer.copyOf(packet.length)
            val plaintext = if (datagram.startsWithSecureMagic()) {
                val codec = secureCodec ?: error("Encrypted datagram arrived before key exchange")
                codec.open(datagram)
            } else {
                check(secureCodec == null) {
                    "Plaintext datagram rejected after secure-session establishment"
                }
                datagram
            }
            TransportPacketCodec.decode(plaintext)
        } catch (_: SocketTimeoutException) {
            null
        }
    }

    override fun close() {
        isOpen = false
        socket.close()
    }
}

private fun ByteArray.startsWithSecureMagic(): Boolean {
    return size >= 4 &&
        this[0] == 'G'.code.toByte() &&
        this[1] == 'L'.code.toByte() &&
        this[2] == 'Y'.code.toByte() &&
        this[3] == 'E'.code.toByte()
}

private fun defaultDeviceName(): String = listOfNotNull(
    "GlyphRay Android",
    Build.MANUFACTURER.takeUnless { it.isNullOrBlank() },
    Build.MODEL.takeUnless { it.isNullOrBlank() },
).joinToString(" ")
