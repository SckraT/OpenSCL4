package com.oppzippy.openscq30.widget

import android.content.Context
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.glance.ColorFilter
import androidx.glance.GlanceModifier
import androidx.glance.Image
import androidx.glance.ImageProvider
import androidx.glance.action.Action
import androidx.glance.action.clickable
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

/** The 2×2 switcher: the whole tile moves between noise canceling and transparency. */
class ModeToggleWidget : DeviceWidget() {
    @Composable
    override fun DeviceContent(context: Context, state: DeviceWidgetState?, palette: WidgetPalette) {
        val connected = state as? DeviceWidgetState.Connected
        val modes = connected?.soundModes
        val target = modes?.toggleTarget
        val caption = switchCaption(context, state, modes)
        val hint = switchHint(context, state, target)
        val description = if (target != null) {
            context.getString(R.string.widget_switch_description, caption, hint)
        } else {
            context.getString(R.string.widget_switch_description_unavailable, caption, hint)
        }
        val action: Action? = when {
            connected != null && target != null -> setSoundModeAction(context, target)
            state is DeviceWidgetState.Lost -> retryConnectionAction(context)
            state is DeviceWidgetState.Recovering -> null
            else -> openDeviceAction(context)
        }
        val icon = when {
            connected != null -> modeIcon(modes?.current)
            state is DeviceWidgetState.Recovering -> R.drawable.bluetooth_24px
            else -> R.drawable.baseline_headset_off_24
        }
        var modifier = GlanceModifier
            .fillMaxSize()
        if (action != null) {
            modifier = modifier.clickable(action)
        }
        modifier = modifier
            .background(ColorProvider(palette.lavender))
            .semantics { contentDescription = description }
            .padding(13.dp)
        Column(modifier, verticalAlignment = Alignment.CenterVertically) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                WidgetLogo(22.dp)
                Spacer(GlanceModifier.width(7.dp))
                Text(
                    fallbackDeviceName(context, state),
                    style = widgetStyle(palette, palette.onLavender, 12f, FontWeight.Bold),
                    maxLines = 1,
                )
            }
            Spacer(GlanceModifier.height(12.dp))
            Box(
                modifier = GlanceModifier
                    .size(49.dp)
                    .background(ColorProvider(palette.primary))
                    .cornerRadius(17.dp),
                contentAlignment = Alignment.Center,
            ) {
                Image(
                    provider = ImageProvider(icon),
                    contentDescription = null,
                    modifier = GlanceModifier.size(25.dp),
                    colorFilter = ColorFilter.tint(ColorProvider(palette.onPrimary)),
                )
            }
            Spacer(GlanceModifier.height(8.dp))
            Text(
                caption,
                style = widgetStyle(palette, palette.onLavender, 15f, FontWeight.Bold),
                maxLines = 2,
            )
            Spacer(GlanceModifier.height(3.dp))
            Text(hint, style = widgetStyle(palette, palette.onLavender, 11f), maxLines = 1)
        }
    }
}

private fun switchCaption(context: Context, state: DeviceWidgetState?, modes: WidgetSoundModes?): String = when {
    modes != null -> modes.modes.firstOrNull { it.id == modes.current }?.label
        ?: fallbackDeviceName(context, state)

    state is DeviceWidgetState.Recovering -> context.getString(R.string.sound_overview_reconnecting)

    state is DeviceWidgetState.Lost -> context.getString(R.string.sound_overview_disconnected)

    state is DeviceWidgetState.Connecting -> context.getString(R.string.sound_device_connecting)

    else -> context.getString(R.string.disconnected)
}

private fun switchHint(context: Context, state: DeviceWidgetState?, target: String?): String = when {
    target == WIDGET_MODE_NOISE_CANCELING -> context.getString(R.string.widget_next_noise_canceling)
    target == WIDGET_MODE_TRANSPARENCY -> context.getString(R.string.widget_next_transparency)
    state is DeviceWidgetState.Recovering -> context.getString(R.string.widget_connection_waiting)
    state is DeviceWidgetState.Lost -> context.getString(R.string.sound_overview_retry)
    state is DeviceWidgetState.Connecting -> context.getString(R.string.widget_connection_waiting)
    else -> context.getString(R.string.widget_connection_no_device)
}

class ModeToggleWidgetReceiver : DeviceWidgetReceiver(ModeToggleWidget())
