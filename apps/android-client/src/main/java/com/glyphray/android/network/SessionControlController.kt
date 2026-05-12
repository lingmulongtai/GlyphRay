package com.glyphray.android.network

import android.os.Build
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.io.Closeable
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetSocketAddress
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors

data class SessionControlState(
    val isConnected: Boolean = false,
    val connectedHostName: String? = null,
    val packetsSent: Long = 0,
    val lastAction: String = "Idle",
    val lastError: String? = null,
) {
    val statusLabel: String
        get() = if (isConnected) "Control channel ready" else "Disconnected"
}

class SessionControlController : Closeable {
    private val executor: ExecutorService = Executors.newSingleThreadExecutor()
    private var client: ControlUdpClient? = null

    var state by mutableStateOf(SessionControlState())
        private set

    fun connect(host: DiscoveredHost) {
        executor.execute {
            runCatching {
                client?.close()
                client = ControlUdpClient().also { it.connect(host) }
                state = state.copy(
                    isConnected = true,
                    connectedHostName = host.hostName,
                    lastAction = "Connected to ${host.hostName}",
                    lastError = null,
                )
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

    fun disconnect() {
        executor.execute {
            client?.close()
            client = null
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

    override fun close() {
        client?.close()
        client = null
        executor.shutdownNow()
    }
}

private class ControlUdpClient : Closeable {
    private val socket = DatagramSocket()
    private var remote: InetSocketAddress? = null
    private var nextTransportSequence = 1L
    private var nextFrameSequence = 1L

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

    override fun close() {
        socket.close()
    }
}

private fun defaultDeviceName(): String = listOfNotNull(
    "GlyphRay Android",
    Build.MANUFACTURER.takeUnless { it.isNullOrBlank() },
    Build.MODEL.takeUnless { it.isNullOrBlank() },
).joinToString(" ")
