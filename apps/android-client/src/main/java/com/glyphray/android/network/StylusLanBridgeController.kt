package com.glyphray.android.network

import android.os.Handler
import android.os.Looper
import android.view.MotionEvent
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import com.glyphray.android.input.StylusStreamController
import java.io.Closeable
import java.util.concurrent.RejectedExecutionException
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean

data class StylusLanBridgeState(
    val connectedHostName: String? = null,
    val isConnected: Boolean = false,
    val packetsSent: Long = 0,
    val samplesSent: Long = 0,
    val bytesSent: Long = 0,
    val lastError: String? = null,
) {
    val statusLabel: String
        get() = when {
            lastError != null -> "Input error"
            isConnected -> "Streaming stylus"
            else -> "Input idle"
        }
}

class StylusLanBridgeController(
    private val streamController: StylusStreamController = StylusStreamController(),
) : Closeable {
    private val mainHandler = Handler(Looper.getMainLooper())
    private val executor = Executors.newSingleThreadExecutor { runnable ->
        Thread(runnable, "GlyphRayStylusLanBridge").apply {
            isDaemon = true
        }
    }
    private val closed = AtomicBoolean(false)
    private var sender: StylusUdpSender? = null

    var state by mutableStateOf(StylusLanBridgeState())
        private set

    fun connect(host: DiscoveredHost) {
        execute {
            runCatching {
                val nextSender = StylusUdpSender().apply { connect(host) }
                val previous = sender
                sender = nextSender
                previous?.close()
                postState {
                    StylusLanBridgeState(
                        connectedHostName = host.hostName,
                        isConnected = true,
                    )
                }
            }.onFailure { error ->
                postState {
                    it.copy(
                        isConnected = false,
                        lastError = error.message ?: error.javaClass.simpleName,
                    )
                }
            }
        }
    }

    fun disconnect() {
        execute {
            sender?.close()
            sender = null
            postState {
                it.copy(
                    connectedHostName = null,
                    isConnected = false,
                )
            }
        }
    }

    fun onMotionEvent(event: MotionEvent): Boolean {
        val packet = runCatching {
            streamController.onMotionEvent(event)
        }.getOrElse { error ->
            postState {
                it.copy(lastError = error.message ?: error.javaClass.simpleName)
            }
            return true
        }
        if (packet.sampleCount == 0) {
            return true
        }

        execute {
            val activeSender = sender
            if (activeSender == null) {
                postState { it.copy(lastError = "No host selected for stylus stream") }
                return@execute
            }

            runCatching {
                activeSender.send(packet)
            }.onSuccess { bytes ->
                postState {
                    it.copy(
                        packetsSent = it.packetsSent + 1,
                        samplesSent = it.samplesSent + packet.sampleCount,
                        bytesSent = it.bytesSent + bytes,
                        lastError = null,
                    )
                }
            }.onFailure { error ->
                postState {
                    it.copy(
                        isConnected = false,
                        lastError = error.message ?: error.javaClass.simpleName,
                    )
                }
            }
        }
        return true
    }

    override fun close() {
        if (!closed.compareAndSet(false, true)) {
            return
        }
        try {
            executor.execute {
                sender?.close()
                sender = null
            }
        } catch (_: RejectedExecutionException) {
            sender?.close()
            sender = null
        }
        executor.shutdown()
    }

    private fun execute(action: () -> Unit) {
        if (closed.get()) {
            return
        }
        try {
            executor.execute { action() }
        } catch (_: RejectedExecutionException) {
            postState {
                it.copy(
                    isConnected = false,
                    lastError = "Stylus stream worker is closed",
                )
            }
        }
    }

    private fun postState(update: (StylusLanBridgeState) -> StylusLanBridgeState) {
        mainHandler.post {
            state = update(state)
        }
    }
}
