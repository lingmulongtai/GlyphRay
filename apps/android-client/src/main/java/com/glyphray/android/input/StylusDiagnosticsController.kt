package com.glyphray.android.input

import android.view.MotionEvent
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

class StylusDiagnosticsController {
    var state by mutableStateOf(StylusDiagnosticsState())
        private set

    fun onMotionEvent(event: MotionEvent): Boolean {
        val samples = event.toStylusSamples()
        if (samples.isEmpty()) {
            return true
        }

        val latest = samples.last()
        state = state.copy(
            latest = latest,
            totalSamples = state.totalSamples + samples.size,
            historicalSamples = state.historicalSamples + samples.count { it.isHistorical },
            hoverSamples = state.hoverSamples + samples.count { it.isHover },
            contactSamples = state.contactSamples + samples.count { !it.isHover },
            lastBatchSize = samples.size,
        )
        return true
    }
}

