package com.oppzippy.openscq30.widget

import android.content.Context
import androidx.annotation.DrawableRes
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
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
import androidx.glance.layout.fillMaxHeight
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
import androidx.glance.text.TextStyle
import androidx.glance.unit.ColorProvider
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.features.preferences.EarbudsColor
import com.oppzippy.openscq30.lib.bindings.translateDeviceModel
import com.oppzippy.openscq30.ui.sound.deviceArt
import com.oppzippy.openscq30.ui.sound.deviceArtOnCard

/** A text style of the widget: the prototype's Onest look approximated with the system typeface. */
internal fun widgetStyle(
    palette: WidgetPalette,
    color: Color,
    size: Float,
    weight: FontWeight = FontWeight.Normal,
): TextStyle = TextStyle(color = ColorProvider(color), fontSize = size.sp, fontWeight = weight)

/** The rounded app mark the prototype shows in the switcher header. */
@Composable
internal fun WidgetLogo(size: androidx.compose.ui.unit.Dp) {
    Box(
        modifier = GlanceModifier
            .size(size)
            .background(ColorProvider(Color(0xFF403052)))
            .cornerRadius(size * 0.32f),
        contentAlignment = Alignment.Center,
    ) {
        Image(
            provider = ImageProvider(R.drawable.openscq30_logo_foreground),
            contentDescription = null,
            modifier = GlanceModifier.size(size * 0.62f),
            colorFilter = ColorFilter.tint(ColorProvider(Color(0xFFD4BAFF))),
        )
    }
}

/** The illustration of the device used on the sage hero cards. */
@Composable
internal fun WidgetDeviceArt(model: String, modifier: GlanceModifier, darkTheme: Boolean) {
    val art = deviceArt(model)
    if (art != null) {
        Image(
            provider = ImageProvider(
                deviceArtOnCard(
                    art,
                    EarbudsColor.Silver.takeIf {
                        darkTheme
                    },
                    darkHeader = darkTheme,
                ),
            ),
            contentDescription = null,
            modifier = modifier,
        )
    } else {
        Box(modifier, contentAlignment = Alignment.Center) {
            Image(
                provider = ImageProvider(R.drawable.headphones_24px),
                contentDescription = null,
                modifier = GlanceModifier.fillMaxSize().padding(6.dp),
                colorFilter = ColorFilter.tint(ColorProvider(Color(0xFF34462D))),
            )
        }
    }
}

/** The icon the prototype uses for a sound mode. */
@DrawableRes
internal fun modeIcon(modeId: String?): Int = when (modeId) {
    WIDGET_MODE_NOISE_CANCELING -> R.drawable.sound_noise
    WIDGET_MODE_TRANSPARENCY -> R.drawable.sound_transparency
    WIDGET_MODE_AIRPLANE -> R.drawable.sound_plane
    else -> R.drawable.headphones_24px
}

/** A thin battery bar: filled segments of the prototype's leaf colour over the track. */
@Composable
internal fun WidgetProgressBar(progress: Float?, palette: WidgetPalette, modifier: GlanceModifier = GlanceModifier) {
    if (progress == null) return
    val filledSegments = (progress.coerceIn(0f, 1f) * PROGRESS_SEGMENTS).toInt().coerceIn(0, PROGRESS_SEGMENTS)
    Box(modifier.fillMaxWidth().height(3.dp).background(ColorProvider(palette.line)).cornerRadius(3.dp)) {
        Row(GlanceModifier.fillMaxSize()) {
            repeat(PROGRESS_SEGMENTS) { index ->
                val color = if (index < filledSegments) palette.leaf else Color.Transparent
                Box(GlanceModifier.defaultWeight().fillMaxHeight().background(ColorProvider(color))) {}
            }
        }
    }
}

private const val PROGRESS_SEGMENTS = 16

/** The label resource of a battery cell. */
@androidx.annotation.StringRes
internal fun batteryLabelRes(cell: WidgetBatteryCell): Int = when (cell) {
    WidgetBatteryCell.Left -> R.string.sound_overview_left
    WidgetBatteryCell.Right -> R.string.sound_overview_right
    WidgetBatteryCell.Case -> R.string.sound_overview_case
    WidgetBatteryCell.Whole -> R.string.sound_overview_battery
}

/** One battery cell: label, reading and a thin bar; an unknown reading keeps its place without a bar. */
@Composable
internal fun WidgetBatteryCellView(context: Context, battery: WidgetBattery, palette: WidgetPalette) {
    val label = context.getString(batteryLabelRes(battery.cell))
    val description = if (battery.progress == null) {
        context.getString(R.string.widget_battery_unknown_cell, label)
    } else {
        context.getString(R.string.widget_battery_cell_description, label, battery.value)
    }
    Column(
        modifier = GlanceModifier
            .semantics { contentDescription = description }
            .background(ColorProvider(palette.battery))
            .cornerRadius(13.dp)
            .padding(horizontal = 8.dp, vertical = 6.dp),
    ) {
        Text(
            label,
            style = widgetStyle(palette, palette.muted, 11f, FontWeight.Medium),
            maxLines = 1,
        )
        Text(
            battery.value,
            style = widgetStyle(palette, palette.onSage, 16f, FontWeight.Bold),
            maxLines = 1,
        )
        Spacer(GlanceModifier.height(2.dp))
        WidgetProgressBar(battery.progress, palette)
    }
}

/** The battery row of a widget: one cell per reading, keeping the positions of missing ones. */
@Composable
internal fun WidgetBatteryCells(context: Context, cells: List<WidgetBattery>, palette: WidgetPalette) {
    Row(GlanceModifier.fillMaxWidth()) {
        cells.forEachIndexed { index, cell ->
            if (index > 0) Spacer(GlanceModifier.width(5.dp))
            Box(GlanceModifier.defaultWeight()) {
                WidgetBatteryCellView(context, cell, palette)
            }
        }
    }
}

/** A pill button of the widget. */
@Composable
internal fun WidgetButton(
    text: String,
    palette: WidgetPalette,
    background: Color,
    textColor: Color,
    onClick: Action,
    modifier: GlanceModifier = GlanceModifier,
) {
    Box(
        modifier = modifier
            .height(48.dp)
            .clickable(onClick)
            .background(ColorProvider(background))
            .cornerRadius(20.dp)
            .padding(horizontal = 15.dp, vertical = 7.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(text, style = widgetStyle(palette, textColor, 12f, FontWeight.Medium), maxLines = 1)
    }
}

/** The message and action a widget shows while it has no confirmed connection. */
@Composable
internal fun WidgetConnectionPanel(
    context: Context,
    state: DeviceWidgetState,
    palette: WidgetPalette,
    modifier: GlanceModifier = GlanceModifier,
) {
    val message = when (state) {
        is DeviceWidgetState.Recovering -> context.getString(R.string.widget_connection_waiting)
        is DeviceWidgetState.Lost -> context.getString(R.string.widget_connection_check)
        is DeviceWidgetState.Connecting -> context.getString(R.string.sound_device_connecting)
        else -> context.getString(R.string.widget_connection_no_device)
    }
    Column(
        modifier = modifier.fillMaxWidth().padding(horizontal = 9.dp, vertical = 9.dp),
        horizontalAlignment = Alignment.Start,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(message, style = widgetStyle(palette, palette.muted, 12f), maxLines = 3)
        Spacer(GlanceModifier.height(7.dp))
        when (state) {
            is DeviceWidgetState.Lost -> WidgetButton(
                text = context.getString(R.string.sound_overview_retry),
                palette = palette,
                background = palette.primary,
                textColor = palette.onPrimary,
                onClick = retryConnectionAction(context),
            )

            is DeviceWidgetState.Disconnected, is DeviceWidgetState.Connecting -> WidgetButton(
                text = context.getString(R.string.widget_open_app),
                palette = palette,
                background = palette.primary,
                textColor = palette.onPrimary,
                onClick = openDeviceAction(context),
            )

            else -> Unit
        }
    }
}

/** The sound-mode tile the 4×4 panel draws in two columns. */
@Composable
internal fun WidgetModeTile(
    context: Context,
    mode: WidgetMode,
    selected: Boolean,
    palette: WidgetPalette,
    onClick: Action,
    modifier: GlanceModifier = GlanceModifier,
) {
    Row(
        modifier = modifier
            .height(48.dp)
            .clickable(onClick)
            .semantics { contentDescription = mode.label }
            .background(ColorProvider(if (selected) palette.primary else palette.tile))
            .cornerRadius(15.dp)
            .padding(horizontal = 8.dp, vertical = 5.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Image(
            provider = ImageProvider(modeIcon(mode.id)),
            contentDescription = null,
            modifier = GlanceModifier.size(17.dp),
            colorFilter = ColorFilter.tint(ColorProvider(if (selected) palette.onPrimary else palette.text)),
        )
        Spacer(GlanceModifier.width(6.dp))
        Text(
            mode.label,
            modifier = GlanceModifier.defaultWeight(),
            style = widgetStyle(palette, if (selected) palette.onPrimary else palette.text, 11.5f, FontWeight.Medium),
            maxLines = 2,
        )
        if (selected) {
            Image(
                provider = ImageProvider(R.drawable.check_24px),
                contentDescription = null,
                modifier = GlanceModifier.size(13.dp),
                colorFilter = ColorFilter.tint(ColorProvider(palette.onPrimary)),
            )
        }
    }
}

/** The quick-preset tile of the 4×4 panel. */
@Composable
internal fun WidgetPresetTile(context: Context, preset: WidgetPreset, palette: WidgetPalette, onClick: Action) {
    val description = if (preset.lastApplied) {
        context.getString(R.string.widget_last_preset, preset.name)
    } else {
        preset.name
    }
    Row(
        modifier = GlanceModifier
            .height(48.dp)
            .clickable(onClick)
            .semantics { contentDescription = description }
            .background(ColorProvider(palette.lavender))
            .cornerRadius(15.dp)
            .padding(horizontal = 8.dp, vertical = 5.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Image(
            provider = ImageProvider(R.drawable.bolt_24px),
            contentDescription = null,
            modifier = GlanceModifier.size(17.dp),
            colorFilter = ColorFilter.tint(ColorProvider(palette.onLavender)),
        )
        Spacer(GlanceModifier.width(6.dp))
        Text(
            preset.name,
            modifier = GlanceModifier.defaultWeight(),
            style = widgetStyle(palette, palette.onLavender, 11.5f, FontWeight.Medium),
            maxLines = 1,
        )
        if (preset.lastApplied) {
            Image(
                provider = ImageProvider(R.drawable.check_24px),
                contentDescription = null,
                modifier = GlanceModifier.size(13.dp),
                colorFilter = ColorFilter.tint(ColorProvider(palette.onLavender)),
            )
        }
    }
}

/** A heading of the 4×4 panel. */
@Composable
internal fun WidgetSectionLabel(text: String, palette: WidgetPalette) {
    Text(
        text,
        modifier = GlanceModifier.padding(start = 2.dp, top = 9.dp, bottom = 5.dp),
        style = widgetStyle(palette, palette.text, 12f, FontWeight.Bold),
        maxLines = 1,
    )
}

/** The small round button that opens the device in the client. */
@Composable
internal fun WidgetOpenButton(context: Context, tint: Color, modifier: GlanceModifier = GlanceModifier) {
    Box(
        modifier = modifier
            .size(44.dp)
            .clickable(openDeviceAction(context))
            .semantics { contentDescription = context.getString(R.string.widget_open_device) },
        contentAlignment = Alignment.Center,
    ) {
        Image(
            provider = ImageProvider(R.drawable.arrow_outward_24px),
            contentDescription = null,
            modifier = GlanceModifier.size(17.dp),
            colorFilter = ColorFilter.tint(ColorProvider(tint)),
        )
    }
}

/** The name the sentinel shows for a device that is not known yet. */
internal fun fallbackDeviceName(context: Context, state: DeviceWidgetState?): String =
    state?.deviceName?.takeIf { it.isNotEmpty() } ?: context.getString(R.string.app_name)

/** The translated model without the manufacturer, for the hero cards. */
internal fun translatedShortModel(model: String): String = translateDeviceModel(model).removePrefix("Soundcore ")
