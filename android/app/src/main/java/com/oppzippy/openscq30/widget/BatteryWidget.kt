package com.oppzippy.openscq30.widget

import android.content.Context
import androidx.compose.runtime.Composable
import androidx.compose.ui.unit.dp
import androidx.glance.GlanceModifier
import androidx.glance.background
import androidx.glance.layout.Column
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.padding
import androidx.glance.unit.ColorProvider

/** The 3×1 battery widget: the left earbud, the right earbud and the case in one row. */
class BatteryWidget : DeviceWidget() {
    @Composable
    override fun DeviceContent(context: Context, state: DeviceWidgetState?, palette: WidgetPalette) {
        Column(
            GlanceModifier
                .fillMaxSize()
                .background(ColorProvider(palette.background))
                .padding(7.dp),
            verticalAlignment = androidx.glance.layout.Alignment.CenterVertically,
        ) {
            val cells = (state as? DeviceWidgetState.Connected)?.batteryCells ?: disconnectedBatteryCells()
            WidgetBatteryCells(context, cells, palette)
        }
    }
}

/** A disconnected battery widget keeps the three positions and shows no reading. */
private fun disconnectedBatteryCells(): List<WidgetBattery> = listOf(
    WidgetBattery(WidgetBatteryCell.Left, WIDGET_UNKNOWN_VALUE),
    WidgetBattery(WidgetBatteryCell.Right, WIDGET_UNKNOWN_VALUE),
    WidgetBattery(WidgetBatteryCell.Case, WIDGET_UNKNOWN_VALUE),
)

class BatteryWidgetReceiver : DeviceWidgetReceiver(BatteryWidget())
