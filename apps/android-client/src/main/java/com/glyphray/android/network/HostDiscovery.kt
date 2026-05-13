package com.glyphray.android.network

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.io.Closeable
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.SocketTimeoutException
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.charset.StandardCharsets
import java.util.concurrent.atomic.AtomicBoolean

private const val defaultDiscoveryPort = 44_998
private const val discoveryHeaderLength = 33

data class DiscoveredHost(
    val hostId: String,
    val hostName: String,
    val address: InetAddress,
    val protocolVersion: Int,
    val controlPort: Int,
    val videoPort: Int,
    val supportsWindowsInk: Boolean,
    val supportsH264: Boolean,
    val pairingRequired: Boolean,
    val loadPercent: Int,
    val lastSeenElapsedMs: Long,
) {
    val endpoint: InetSocketAddress
        get() = InetSocketAddress(address, controlPort)

    val capabilitiesLabel: String
        get() = listOfNotNull(
            if (supportsWindowsInk) "Ink ready" else null,
            if (supportsH264) "H.264" else null,
            if (pairingRequired) "Pairing" else "Trusted",
        ).joinToString(" / ")
}

data class HostDiscoveryState(
    val hosts: List<DiscoveredHost> = emptyList(),
    val isScanning: Boolean = false,
    val lastError: String? = null,
    val lastScanLabel: String = "not scanned",
)

data class ManualHostEndpoint(
    val addressText: String,
    val controlPort: Int = 44_999,
    val videoPort: Int = 45_000,
)

interface ManualHostStore {
    fun load(): List<ManualHostEndpoint>
    fun save(endpoints: List<ManualHostEndpoint>)
}

class AndroidManualHostStore(context: Context) : ManualHostStore {
    private val preferences = context.applicationContext.getSharedPreferences("glyphray_hosts", Context.MODE_PRIVATE)

    override fun load(): List<ManualHostEndpoint> {
        return preferences
            .getStringSet(savedHostsKey, emptySet())
            .orEmpty()
            .mapNotNull(::decodeEndpoint)
            .sortedBy { it.addressText }
    }

    override fun save(endpoints: List<ManualHostEndpoint>) {
        val encoded = endpoints
            .distinctBy { "${it.addressText.lowercase()}:${it.controlPort}:${it.videoPort}" }
            .map(::encodeEndpoint)
            .toSet()
        preferences.edit().putStringSet(savedHostsKey, encoded).apply()
    }

    private fun encodeEndpoint(endpoint: ManualHostEndpoint): String =
        "${endpoint.addressText}|${endpoint.controlPort}|${endpoint.videoPort}"

    private fun decodeEndpoint(value: String): ManualHostEndpoint? {
        val parts = value.split('|')
        if (parts.size != 3) {
            return null
        }
        val address = parts[0].trim().takeIf { it.isNotBlank() } ?: return null
        return ManualHostEndpoint(
            addressText = address,
            controlPort = parts[1].toIntOrNull() ?: return null,
            videoPort = parts[2].toIntOrNull() ?: return null,
        )
    }

    private companion object {
        const val savedHostsKey = "manual_hosts"
    }
}

object NoopManualHostStore : ManualHostStore {
    override fun load(): List<ManualHostEndpoint> = emptyList()
    override fun save(endpoints: List<ManualHostEndpoint>) = Unit
}

class HostDiscoveryController(
    private val client: LanHostDiscoveryClient = LanHostDiscoveryClient(),
    private val manualHostStore: ManualHostStore = NoopManualHostStore,
) : Closeable {
    private val mainHandler = Handler(Looper.getMainLooper())
    private val running = AtomicBoolean(false)
    private var worker: Thread? = null

    var state by mutableStateOf(HostDiscoveryState())
        private set

    fun startContinuousScan() {
        if (!running.compareAndSet(false, true)) {
            return
        }

        loadSavedManualHosts()
        postState { it.copy(isScanning = true, lastError = null) }
        worker = Thread {
            while (running.get()) {
                runCatching {
                    client.receiveAnnouncements(windowMs = 1_500)
                }.onSuccess(::mergeHosts)
                    .onFailure { error ->
                        postState {
                            it.copy(
                                isScanning = false,
                                lastError = error.message ?: error.javaClass.simpleName,
                            )
                        }
                    }
            }
        }.apply {
            name = "GlyphRayHostDiscovery"
            isDaemon = true
            start()
        }
    }

    fun refreshOnce() {
        if (running.get()) {
            postState { it.copy(isScanning = true, lastError = null) }
            return
        }

        Thread {
            postState { it.copy(isScanning = true, lastError = null) }
            runCatching {
                client.receiveAnnouncements(windowMs = 1_500)
            }.onSuccess(::mergeHosts)
                .onFailure { error ->
                    postState {
                        it.copy(
                            isScanning = false,
                            lastError = error.message ?: error.javaClass.simpleName,
                        )
                    }
                }
        }.apply {
            name = "GlyphRayHostDiscoveryRefresh"
            isDaemon = true
            start()
        }
    }

    fun addManualHost(addressText: String, controlPort: Int = 44_999, videoPort: Int = 45_000) {
        Thread {
            runCatching {
                val endpoint = ManualHostEndpoint(
                    addressText = addressText.trim(),
                    controlPort = controlPort,
                    videoPort = videoPort,
                )
                val host = endpoint.toDiscoveredHost()
                saveManualEndpoint(endpoint)
                host
            }.onSuccess { host ->
                mergeHosts(listOf(host))
            }.onFailure { error ->
                postState {
                    it.copy(
                        isScanning = false,
                        lastError = error.message ?: error.javaClass.simpleName,
                    )
                }
            }
        }.apply {
            name = "GlyphRayManualHostAdd"
            isDaemon = true
            start()
        }
    }

    private fun loadSavedManualHosts() {
        Thread {
            runCatching {
                manualHostStore.load().mapNotNull { endpoint ->
                    runCatching { endpoint.toDiscoveredHost() }.getOrNull()
                }
            }.onSuccess { hosts ->
                mergeHosts(hosts)
            }.onFailure { error ->
                postState {
                    it.copy(lastError = error.message ?: error.javaClass.simpleName)
                }
            }
        }.apply {
            name = "GlyphRayManualHostLoad"
            isDaemon = true
            start()
        }
    }

    private fun saveManualEndpoint(endpoint: ManualHostEndpoint) {
        val saved = manualHostStore.load()
            .filterNot {
                it.addressText.equals(endpoint.addressText, ignoreCase = true) &&
                    it.controlPort == endpoint.controlPort &&
                    it.videoPort == endpoint.videoPort
            } + endpoint
        manualHostStore.save(saved.takeLast(24))
    }

    override fun close() {
        running.set(false)
        client.close()
        worker?.interrupt()
        worker = null
    }

    private fun mergeHosts(newHosts: List<DiscoveredHost>) {
        postState { current ->
            val merged = LinkedHashMap<String, DiscoveredHost>()
            current.hosts.forEach { host -> merged[host.hostId] = host }
            newHosts.forEach { host -> merged[host.hostId] = host }
            current.copy(
                hosts = merged.values
                    .sortedByDescending { it.lastSeenElapsedMs }
                    .take(12),
                isScanning = running.get(),
                lastError = null,
                lastScanLabel = "just now",
            )
        }
    }

    private fun postState(update: (HostDiscoveryState) -> HostDiscoveryState) {
        mainHandler.post {
            state = update(state)
        }
    }
}

private fun ManualHostEndpoint.toDiscoveredHost(): DiscoveredHost {
    val address = InetAddress.getByName(addressText)
    return DiscoveredHost(
        hostId = "manual-${address.hostAddress}:$controlPort",
        hostName = "Manual $addressText",
        address = address,
        protocolVersion = 1,
        controlPort = controlPort,
        videoPort = videoPort,
        supportsWindowsInk = true,
        supportsH264 = true,
        pairingRequired = true,
        loadPercent = 0,
        lastSeenElapsedMs = SystemClock.elapsedRealtime(),
    )
}

class LanHostDiscoveryClient(
    private val discoveryPort: Int = defaultDiscoveryPort,
) : Closeable {
    private val socketHolder = lazy {
        DatagramSocket(null).apply {
            reuseAddress = true
            broadcast = true
            soTimeout = 250
            bind(InetSocketAddress(discoveryPort))
        }
    }
    private val socket: DatagramSocket by socketHolder

    fun receiveAnnouncements(windowMs: Long): List<DiscoveredHost> {
        val found = LinkedHashMap<String, DiscoveredHost>()
        val deadline = SystemClock.elapsedRealtime() + windowMs.coerceAtLeast(1)
        val buffer = ByteArray(512)

        while (SystemClock.elapsedRealtime() < deadline) {
            val packet = DatagramPacket(buffer, buffer.size)
            try {
                socket.receive(packet)
                val host = GlyphRayDiscoveryCodec.decode(
                    bytes = packet.data,
                    length = packet.length,
                    address = packet.address,
                )
                if (host != null) {
                    found[host.hostId] = host
                }
            } catch (_: SocketTimeoutException) {
                // Keep the scan window short without turning normal idle time into an error.
            }
        }

        return found.values.toList()
    }

    override fun close() {
        if (socketHolder.isInitialized()) {
            socket.close()
        }
    }
}

object GlyphRayDiscoveryCodec {
    fun decode(bytes: ByteArray, length: Int, address: InetAddress): DiscoveredHost? {
        if (length < discoveryHeaderLength) {
            return null
        }
        if (bytes[0] != 'G'.code.toByte() ||
            bytes[1] != 'L'.code.toByte() ||
            bytes[2] != 'Y'.code.toByte() ||
            bytes[3] != 'D'.code.toByte()
        ) {
            return null
        }

        val buffer = ByteBuffer
            .wrap(bytes, 0, length)
            .order(ByteOrder.LITTLE_ENDIAN)
        buffer.position(4)

        val version = buffer.short.toUShortInt()
        if (version != 1) {
            return null
        }

        val hostIdBytes = ByteArray(16)
        buffer.get(hostIdBytes)
        val protocolVersion = buffer.short.toUShortInt()
        val controlPort = buffer.short.toUShortInt()
        val videoPort = buffer.short.toUShortInt()
        val flags = buffer.get().toUByteInt()
        val loadPercent = buffer.get().toUByteInt().coerceAtMost(100)
        val hostNameLength = buffer.get().toUByteInt()
        buffer.position(buffer.position() + 2)

        if (length != discoveryHeaderLength + hostNameLength) {
            return null
        }

        val hostNameBytes = ByteArray(hostNameLength)
        buffer.get(hostNameBytes)
        val hostName = String(hostNameBytes, StandardCharsets.UTF_8)

        return DiscoveredHost(
            hostId = hostIdBytes.toHexString(),
            hostName = hostName,
            address = address,
            protocolVersion = protocolVersion,
            controlPort = controlPort,
            videoPort = videoPort,
            supportsWindowsInk = (flags and 0b0000_0001) != 0,
            supportsH264 = (flags and 0b0000_0010) != 0,
            pairingRequired = (flags and 0b0000_0100) != 0,
            loadPercent = loadPercent,
            lastSeenElapsedMs = SystemClock.elapsedRealtime(),
        )
    }
}

private fun Short.toUShortInt(): Int = toInt() and 0xffff

private fun Byte.toUByteInt(): Int = toInt() and 0xff

private fun ByteArray.toHexString(): String =
    joinToString(separator = "") { byte -> "%02x".format(byte.toUByteInt()) }
