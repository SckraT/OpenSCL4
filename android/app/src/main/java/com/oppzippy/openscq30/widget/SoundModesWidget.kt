package com.oppzippy.openscq30.widget

import android.content.Context
import androidx.compose.runtime.Composable
import androidx.compose.ui.unit.dp
import androidx.glance.ColorFilter
import androidx.glance.GlanceModifier
import androidx.glance.Image
import androidx.glance.ImageProvider
import androidx.glance.action.clickable
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.cornerRadius
import androidx.glance.background
import androidx.glance.layout.Alignment
import androidx.glance.layout.Box
import androidx.glance.layout.Column
import androidx.glance.layout.Row
import androidx.glance.layout.Spacer
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.fillMaxWidth
import androidx.glance.layout.height
import androidx.glance.layout.padding
import androidx.glance.layout.size
import androidx.glance.layout.width
import androidx.glance.semantics.contentDescription
import androidx.glance.semantics.semantics
import androidx.glance.text.FontWeight
import androidx.glance.text.Text
import androidx.glance.unit.ColorProvider
import com.oppzippy.openscq30.R

/** The 4×2 widget: the device and its charge above one row of the three sound modes. */
class SoundModesWidget : DeviceWidget() {
    @Composable
    override fun DeviceContent(context: Context, state: DeviceWidgetState?, palette: WidgetPalette) {
        Column(
            GlanceModifier
                .fillMaxSize()
                .background(ColorProvider(palette.background))
                .padding(7.dp),
        ) {
            if (state is DeviceWidgetState.Connected) {
                MediumHero(context, state, palette)
                Spacer(GlanceModifier.height(5.dp))
                if (state.soundModes.actionModes.isEmpty()) {
                    WidgetConnectionPanel(context, state, palette)
                } else {
                    Row(GlanceModifier.fillMaxWidth()) {
                        val modes = state.soundModes.actionModes
                        modes.forEachIndexed { index, mode ->
                            if (index > 0) Spacer(GlanceModifier.width(4.dp))
                            ModePill(
                                context = context,
                                mode = mode,
                                selected = state.soundModes.isSelected(mode.id),
                                enabled = state.soundModes.canSwitch,
                                palette = palette,
                                onClick = setSoundModeAction(context, mode.id),
                                modifier = GlanceModifier.defaultWeight(),
                            )
                        }
                    }
                }
            } else {
                WidgetConnectionPanel(context, state ?: DeviceWidgetState.Disconnected(), palette)
            }
        }
    }
}

@Composable
private fun MediumHero(context: Context, state: DeviceWidgetState.Connected, palette: WidgetPalette) {
    Row(
        modifier = GlanceModifier
            .fillMaxWidth()
            .background(ColorProvider(palette.sage))
            .cornerRadius(28.dp)
            .padding(start = 10.dp, end = 4.dp, top = 3.dp, bottom = 3.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        WidgetDeviceArt(state.deviceModel, GlanceModifier.size(36.dp), darkTheme = palette.isDark)
        Spacer(GlanceModifier.width(9.dp))
        Column(GlanceModifier.defaultWeight()) {
            Text(
                state.deviceName,
                style = widgetStyle(palette, palette.onSage, 16f, FontWeight.Bold),
                maxLines = 1,
            )
            Text(
                state.batteryCells.joinToString(" · ") { it.value },
                style = widgetStyle(palette, palette.onSage, 12f, FontWeight.Medium),
                maxLines = 1,
            )
        }
        WidgetOpenButton(context, palette.onSage)
    }
}

@Composable
private fun ModePill(
    context: Context,
    mode: WidgetMode,
    selected: Boolean,
    enabled: Boolean,
    palette: WidgetPalette,
    onClick: androidx.glance.action.Action,
    modifier: GlanceModifier,
) {
    val textColor = if (selected) palette.onPrimary else palette.text
    var pillModifier = modifier.height(48.dp)
    if (enabled) {
        pillModifier = pillModifier.clickable(onClick)
    }
    pillModifier = pillModifier
        .semantics { contentDescription = mode.label }
        .background(ColorProvider(if (selected) palette.primary else palette.tile))
        .cornerRadius(24.dp)
        .padding(horizontal = 5.dp, vertical = 4.dp)
    Column(pillModifier, horizontalAlignment = Alignment.CenterHorizontally) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Image(
                provider = ImageProvider(modeIcon(mode.id)),
                contentDescription = null,
                modifier = GlanceModifier.size(17.dp),
                colorFilter = ColorFilter.tint(ColorProvider(textColor)),
            )
            if (selected) {
                Spacer(GlanceModifier.width(3.dp))
                Image(
                    provider = ImageProvider(R.drawable.check_24px),
                    contentDescription = null,
                    modifier = GlanceModifier.size(12.dp),
                    colorFilter = ColorFilter.tint(ColorProvider(textColor)),
                )
            }
        }
        Spacer(GlanceModifier.height(2.dp))
        Text(
            if (mode.id == WIDGET_MODE_NOISE_CANCELING) {
                context.getString(R.string.widget_mode_noise_canceling_short)
            } else {
                mode.label
            },
            style = widgetStyle(palette, textColor, 11f, FontWeight.Medium),
            maxLines = 1,
        )
    }
}

class SoundModesWidgetReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget: SoundModesWidget = SoundModesWidget()
}
