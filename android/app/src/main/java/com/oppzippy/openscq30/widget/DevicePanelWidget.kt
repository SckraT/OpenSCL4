package com.oppzippy.openscq30.widget

import android.content.Context
import androidx.compose.runtime.Composable
import androidx.compose.ui.unit.dp
import androidx.glance.ColorFilter
import androidx.glance.GlanceModifier
import androidx.glance.Image
import androidx.glance.ImageProvider
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
import androidx.glance.text.FontWeight
import androidx.glance.text.Text
import androidx.glance.unit.ColorProvider
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.lib.bindings.translateDeviceModel
import com.oppzippy.openscq30.ui.deviceselection.splitBrand

/** The 4×4 device panel: the device, its charge, the sound modes and the quick presets. */
class DevicePanelWidget : DeviceWidget() {
    @Composable
    override fun DeviceContent(context: Context, state: DeviceWidgetState?, palette: WidgetPalette) {
        Column(
            GlanceModifier
                .fillMaxSize()
                .background(ColorProvider(palette.background))
                .padding(7.dp),
        ) {
            when (state) {
                is DeviceWidgetState.Connected -> {
                    PanelHero(context, state, palette)
                    WidgetPanelControls(context, state, palette)
                }

                else -> {
                    PanelHero(context, state, palette)
                    WidgetConnectionPanel(
                        context = context,
                        state = state ?: DeviceWidgetState.Disconnected(),
                        palette = palette,
                        modifier = GlanceModifier.height(209.dp).padding(horizontal = 10.dp),
                    )
                }
            }
        }
    }
}

@Composable
private fun PanelHero(context: Context, state: DeviceWidgetState?, palette: WidgetPalette) {
    val connected = state as? DeviceWidgetState.Connected
    Column(
        GlanceModifier
            .fillMaxWidth()
            .background(ColorProvider(palette.sage))
            .cornerRadius(23.dp),
    ) {
        Row(GlanceModifier.fillMaxWidth().height(68.dp).padding(start = 6.dp, top = 8.dp, end = 8.dp)) {
            Column(GlanceModifier.defaultWeight(), verticalAlignment = Alignment.CenterVertically) {
                val model = connected?.deviceModel.orEmpty()
                val brand = if (model.isEmpty()) "" else splitBrand(translateDeviceModel(model)).first.orEmpty()
                if (brand.isNotEmpty()) {
                    Text(
                        brand.uppercase(),
                        style = widgetStyle(palette, palette.onSage, 9.5f, FontWeight.Bold),
                        maxLines = 1,
                    )
                }
                Text(
                    connected?.deviceName?.takeIf { it.isNotEmpty() }
                        ?: fallbackDeviceName(context, state),
                    style = widgetStyle(palette, palette.onSage, 21f, FontWeight.Bold),
                    maxLines = 1,
                )
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Box(
                        GlanceModifier
                            .size(6.dp)
                            .background(ColorProvider(if (connected != null) palette.leaf else palette.muted))
                            .cornerRadius(3.dp),
                    ) {}
                    Spacer(GlanceModifier.width(5.dp))
                    Text(
                        statusLabel(context, state),
                        style = widgetStyle(palette, palette.onSage, 11f, FontWeight.Medium),
                        maxLines = 1,
                    )
                }
            }
            WidgetDeviceArt(
                model = connected?.deviceModel.orEmpty(),
                modifier = GlanceModifier.width(77.dp).height(68.dp),
                darkTheme = palette.isDark,
            )
            WidgetOpenButton(context, palette.onSage)
        }
        if (connected != null) {
            Spacer(GlanceModifier.height(5.dp))
            Box(GlanceModifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 0.dp)) {
                WidgetBatteryCells(context, connected.batteryCells, palette)
            }
        }
    }
}

@Composable
private fun WidgetPanelControls(context: Context, state: DeviceWidgetState.Connected, palette: WidgetPalette) {
    Column(GlanceModifier.fillMaxWidth().padding(start = 4.dp, end = 4.dp, bottom = 4.dp)) {
        WidgetSectionLabel(context.getString(R.string.sound_overview_sound_mode), palette)
        val modes = state.soundModes.modes
        Column(GlanceModifier.fillMaxWidth()) {
            modes.chunked(2).forEach { rowModes ->
                Row(GlanceModifier.fillMaxWidth()) {
                    rowModes.forEachIndexed { index, mode ->
                        if (index > 0) Spacer(GlanceModifier.width(5.dp))
                        WidgetModeTile(
                            context = context,
                            mode = mode,
                            selected = state.soundModes.isSelected(mode.id),
                            palette = palette,
                            onClick = setSoundModeAction(context, mode.id),
                            modifier = GlanceModifier.defaultWeight(),
                        )
                    }
                    if (rowModes.size == 1) {
                        Spacer(GlanceModifier.width(5.dp))
                        Spacer(GlanceModifier.defaultWeight())
                    }
                }
                Spacer(GlanceModifier.height(5.dp))
            }
        }
        PresetSection(context, state, palette)
    }
}

@Composable
private fun PresetSection(context: Context, state: DeviceWidgetState.Connected, palette: WidgetPalette) {
    Row(GlanceModifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text(
            context.getString(R.string.quick_presets),
            modifier = GlanceModifier.defaultWeight().padding(start = 2.dp, top = 10.dp, bottom = 5.dp),
            style = widgetStyle(palette, palette.text, 12f, FontWeight.Bold),
            maxLines = 1,
        )
        val lastApplied = state.presets.firstOrNull { it.lastApplied }
        if (lastApplied != null) {
            Text(
                context.getString(R.string.widget_last_preset, lastApplied.name),
                style = widgetStyle(palette, palette.muted, 10f, FontWeight.Medium),
                maxLines = 1,
            )
        }
    }
    if (state.presets.isEmpty()) {
        Row(GlanceModifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Text(
                context.getString(R.string.widget_presets_empty),
                modifier = GlanceModifier.defaultWeight(),
                style = widgetStyle(palette, palette.muted, 11f),
                maxLines = 2,
            )
            WidgetButton(
                text = context.getString(R.string.widget_presets_configure),
                palette = palette,
                background = palette.lavender,
                textColor = palette.onLavender,
                onClick = configurePresetsAction(context),
            )
        }
        return
    }
    Column(GlanceModifier.fillMaxWidth()) {
        state.presets.chunked(2).forEach { rowPresets ->
            Row(GlanceModifier.fillMaxWidth()) {
                rowPresets.forEachIndexed { index, preset ->
                    if (index > 0) Spacer(GlanceModifier.width(5.dp))
                    val slotIndex = state.presets.indexOf(preset)
                    val action = requireNotNull(activatePresetAction(context, state.presets, slotIndex))
                    Box(GlanceModifier.defaultWeight()) {
                        WidgetPresetTile(context, preset, palette, action)
                    }
                }
                if (rowPresets.size == 1) {
                    Spacer(GlanceModifier.width(5.dp))
                    Spacer(GlanceModifier.defaultWeight())
                }
            }
            Spacer(GlanceModifier.height(5.dp))
        }
    }
}

private fun statusLabel(context: Context, state: DeviceWidgetState?): String = when (state) {
    is DeviceWidgetState.Connected -> context.getString(R.string.connected)
    is DeviceWidgetState.Connecting -> context.getString(R.string.sound_device_connecting)
    is DeviceWidgetState.Recovering -> context.getString(R.string.sound_overview_reconnecting)
    is DeviceWidgetState.Lost -> context.getString(R.string.sound_overview_disconnected)
    else -> context.getString(R.string.disconnected)
}

class DevicePanelWidgetReceiver : DeviceWidgetReceiver(DevicePanelWidget())
