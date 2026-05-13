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
            isConnected -> "Streaming input"
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
    private val touchTranslator = TouchModeTranslator()

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

    fun onMotionEvent(
        event: MotionEvent,
        inputSettings: ClientInputSettings = ClientInputSettings(),
        displayId: Int = 0,
    ): Boolean {
        val directInputKind = event.directInputKind()
        if (directInputKind == DirectInputKind.Mouse && !inputSettings.bluetoothMouseEnabled) {
            return false
        }
        if (directInputKind != DirectInputKind.Stylus) {
            sendDirectInput(event, directInputKind, inputSettings, displayId)
            return true
        }

        val packet = runCatching {
            streamController.onMotionEvent(event, displayId)
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

    private fun sendDirectInput(
        event: MotionEvent,
        kind: DirectInputKind,
        inputSettings: ClientInputSettings,
        displayId: Int,
    ) {
        execute {
            val activeSender = sender
            if (activeSender == null) {
                postState { it.copy(lastError = "No host selected for input stream") }
                return@execute
            }

            runCatching {
                when (kind) {
                    DirectInputKind.Touch -> sendTouchByMode(activeSender, event, inputSettings.touchMode, displayId)
                    DirectInputKind.Mouse -> activeSender.sendMouse(event, displayId)
                    DirectInputKind.Stylus -> 0
                }
            }.onSuccess { bytes ->
                postState {
                    it.copy(
                        packetsSent = it.packetsSent + 1,
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
    }

    private fun sendTouchByMode(
        activeSender: StylusUdpSender,
        event: MotionEvent,
        touchMode: ClientTouchMode,
        displayId: Int,
    ): Int {
        return when (touchMode) {
            ClientTouchMode.Direct -> {
                touchTranslator.resetIfFinished(event)
                activeSender.sendTouch(event, displayId)
            }
            ClientTouchMode.Trackpad -> {
                val mouseEvents = touchTranslator.trackpadMouse(event)
                mouseEvents.sumOf { mouse ->
                    activeSender.sendMouse(
                        displayId = displayId,
                        x = mouse.x,
                        y = mouse.y,
                        buttonFlags = mouse.buttonFlags,
                        timestampUs = mouse.timestampUs,
                    )
                }
            }
            ClientTouchMode.Gesture -> {
                val wheel = touchTranslator.gestureWheel(event)
                if (wheel != null) {
                    activeSender.sendMouse(
                        displayId = displayId,
                        x = wheel.x,
                        y = wheel.y,
                        wheelDeltaX = wheel.wheelDeltaX,
                        wheelDeltaY = wheel.wheelDeltaY,
                        timestampUs = wheel.timestampUs,
                    )
                } else {
                    activeSender.sendTouch(event, displayId)
                }
            }
        }
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

private enum class DirectInputKind {
    Stylus,
    Touch,
    Mouse,
}

private fun MotionEvent.directInputKind(): DirectInputKind {
    val hasMouse = (0 until pointerCount).any { getToolType(it) == MotionEvent.TOOL_TYPE_MOUSE }
    if (hasMouse) {
        return DirectInputKind.Mouse
    }
    val hasFinger = (0 until pointerCount).any { getToolType(it) == MotionEvent.TOOL_TYPE_FINGER }
    return if (hasFinger) DirectInputKind.Touch else DirectInputKind.Stylus
}

private data class RemoteMouseGesture(
    val x: Float,
    val y: Float,
    val wheelDeltaX: Float = 0f,
    val wheelDeltaY: Float = 0f,
    val buttonFlags: Int = 0,
    val timestampUs: Long,
)

private class TouchModeTranslator {
    private var lastX: Float? = null
    private var lastY: Float? = null
    private var downX: Float = 0f
    private var downY: Float = 0f
    private var movedSinceDown: Boolean = false
    private var virtualX: Float = 960f
    private var virtualY: Float = 540f

    fun trackpadMouse(event: MotionEvent): List<RemoteMouseGesture> {
        if (event.pointerCount == 0) {
            return emptyList()
        }
        if (event.actionMasked == MotionEvent.ACTION_UP || event.actionMasked == MotionEvent.ACTION_CANCEL) {
            lastX = null
            lastY = null
            val tapped = !movedSinceDown &&
                kotlin.math.abs(event.getX(0) - downX) <= tapSlopPx &&
                kotlin.math.abs(event.getY(0) - downY) <= tapSlopPx
            movedSinceDown = false
            return if (event.actionMasked == MotionEvent.ACTION_UP && tapped) {
                listOf(
                    RemoteMouseGesture(
                        x = virtualX,
                        y = virtualY,
                        buttonFlags = primaryButtonFlag,
                        timestampUs = event.eventTime * 1_000L,
                    ),
                    RemoteMouseGesture(
                        x = virtualX,
                        y = virtualY,
                        buttonFlags = 0,
                        timestampUs = event.eventTime * 1_000L,
                    ),
                )
            } else {
                listOf(
                    RemoteMouseGesture(
                        x = virtualX,
                        y = virtualY,
                        buttonFlags = 0,
                        timestampUs = event.eventTime * 1_000L,
                    ),
                )
            }
        }

        val x = event.getX(0)
        val y = event.getY(0)
        if (event.actionMasked == MotionEvent.ACTION_DOWN) {
            downX = x
            downY = y
            movedSinceDown = false
        }
        val previousX = lastX
        val previousY = lastY
        lastX = x
        lastY = y
        if (previousX == null || previousY == null) {
            virtualX = x
            virtualY = y
        } else {
            val dx = x - previousX
            val dy = y - previousY
            movedSinceDown = movedSinceDown ||
                kotlin.math.abs(x - downX) > tapSlopPx ||
                kotlin.math.abs(y - downY) > tapSlopPx
            virtualX = (virtualX + dx * trackpadGain).coerceAtLeast(0f)
            virtualY = (virtualY + dy * trackpadGain).coerceAtLeast(0f)
        }
        return listOf(
            RemoteMouseGesture(
                x = virtualX,
                y = virtualY,
                buttonFlags = 0,
                timestampUs = event.eventTime * 1_000L,
            ),
        )
    }

    fun gestureWheel(event: MotionEvent): RemoteMouseGesture? {
        if (event.pointerCount < 2 || event.actionMasked == MotionEvent.ACTION_POINTER_UP) {
            return null
        }
        val centerX = (event.getX(0) + event.getX(1)) * 0.5f
        val centerY = (event.getY(0) + event.getY(1)) * 0.5f
        val previousX = lastX
        val previousY = lastY
        lastX = centerX
        lastY = centerY
        if (previousX == null || previousY == null) {
            return null
        }
        return RemoteMouseGesture(
            x = centerX,
            y = centerY,
            wheelDeltaX = ((centerX - previousX) / 72f).coerceIn(-3f, 3f),
            wheelDeltaY = ((previousY - centerY) / 72f).coerceIn(-3f, 3f),
            timestampUs = event.eventTime * 1_000L,
        )
    }

    fun resetIfFinished(event: MotionEvent) {
        if (event.actionMasked == MotionEvent.ACTION_UP || event.actionMasked == MotionEvent.ACTION_CANCEL) {
            lastX = null
            lastY = null
        }
    }

    private companion object {
        const val primaryButtonFlag = 1
        const val tapSlopPx = 18f
        const val trackpadGain = 1.65f
    }
}
