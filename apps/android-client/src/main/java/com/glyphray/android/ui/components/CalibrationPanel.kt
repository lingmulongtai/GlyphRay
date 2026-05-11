package com.glyphray.android.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp

enum class CalibrationStep {
    TopLeft,
    BottomRight,
    Complete,
}

@Composable
fun CalibrationPanel(
    step: CalibrationStep,
    modifier: Modifier = Modifier,
) {
    Box(
        modifier = modifier
            .fillMaxWidth()
            .height(220.dp)
            .background(Color(0xFF070A0E))
            .border(1.dp, MaterialTheme.colorScheme.outline),
    ) {
        val label = when (step) {
            CalibrationStep.TopLeft -> "Tap top-left target"
            CalibrationStep.BottomRight -> "Tap bottom-right target"
            CalibrationStep.Complete -> "Calibration profile captured"
        }
        Text(
            text = label,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.align(Alignment.Center),
        )
        CalibrationDot(
            modifier = Modifier
                .align(
                    when (step) {
                        CalibrationStep.TopLeft -> Alignment.TopStart
                        CalibrationStep.BottomRight -> Alignment.BottomEnd
                        CalibrationStep.Complete -> Alignment.Center
                    },
                )
                .offset {
                    when (step) {
                        CalibrationStep.TopLeft -> IntOffset(16, 16)
                        CalibrationStep.BottomRight -> IntOffset(-16, -16)
                        CalibrationStep.Complete -> IntOffset.Zero
                    }
                },
        )
    }
}

@Composable
private fun CalibrationDot(modifier: Modifier = Modifier) {
    Box(
        modifier = modifier
            .clip(CircleShape)
            .background(MaterialTheme.colorScheme.primary)
            .border(2.dp, MaterialTheme.colorScheme.secondary, CircleShape)
            .size(22.dp),
    )
}
