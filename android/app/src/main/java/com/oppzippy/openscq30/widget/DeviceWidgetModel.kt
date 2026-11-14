package com.oppzippy.openscq30.widget

import androidx.compose.ui.graphics.Color
import com.oppzippy.openscq30.lib.bindings.AvailabilityReason
import com.oppzippy.openscq30.lib.bindings.ClientBattery
import com.oppzippy.openscq30.lib.bindings.ClientBatteryLabel
import com.oppzippy.openscq30.lib.bindings.ClientSettingAvailability
import com.oppzippy.openscq30.lib.bindings.clientDeviceBatteries
import com.oppzippy.openscq30.lib.bindings.clientIsOnlyOneEarbudConnected
import com.oppzippy.openscq30.lib.bindings.clientIsTws
import com.oppzippy.openscq30.lib.bindings.clientWithMissingEarbud
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.ui.sound.settingAvailability
import com.oppzippy.openscq30.ui.sound.settingsPresentation
import kotlinx.serialization.Serializable

/** The dash the widgets show where the device has no reading, so a missing value never reads as 0%. */
internal const val WIDGET_UNKNOWN_VALUE = "—"

/** Ambient sound mode ids as the device reports them. */
internal const val WIDGET_MODE_NOISE_CANCELING = "NoiseCanceling"
internal const val WIDGET_MODE_TRANSPARENCY = "Transparency"
internal const val WIDGET_MODE_NORMAL = "Normal"
internal const val WIDGET_MODE_AIRPLANE = "AirplaneMode"

/** The modes the 4×2 widget puts in its one row, in the order the presentation fixes. */
internal val WIDGET_ACTION_MODES = listOf(
    WIDGET_MODE_NOISE_CANCELING,
    WIDGET_MODE_TRANSPARENCY,
    WIDGET_MODE_NORMAL,
)

/** Which battery the widget draws. */
@Serializable
enum class WidgetBatteryCell { Left, Right, Case, Whole }

/**
 * One battery cell: the reading and, when it is a level, how full the bar is. A cell with no reading keeps its
 * position and shows [WIDGET_UNKNOWN_VALUE] without a bar.
 */
@Serializable
data class WidgetBattery(val cell: WidgetBatteryCell, val value: String, val progress: Float? = null)

/** A sound mode the device supports, with the device's localized label and the id it reports. */
@Serializable
data class WidgetMode(val id: String, val label: String)

/** The sound modes of the connected device and whether the widgets may change them now. */
@Serializable
data class WidgetSoundModes(
    val current: String? = null,
    val modes: List<WidgetMode> = emptyList(),
    val canSwitch: Boolean = false,
    val unavailableReason: AvailabilityReason? = null,
) {
    /** The mode the 2×2 switcher goes to next, or null when the device cannot switch now. */
    val toggleTarget: String?
        get() {
            if (!canSwitch) return null
            val target = if (current == WIDGET_MODE_NOISE_CANCELING) {
                WIDGET_MODE_TRANSPARENCY
            } else {
                WIDGET_MODE_NOISE_CANCELING
            }
            return target.takeIf { id -> modes.any { it.id == id } }
        }

    /** The modes the 4×2 row draws, in the presentation's order. */
    val actionModes: List<WidgetMode>
        get() = WIDGET_ACTION_MODES.mapNotNull { id -> modes.firstOrNull { it.id == id } }

    /** Whether [mode] is the confirmed current mode. Airplane mode matches none of the three row buttons. */
    fun isSelected(mode: String): Boolean = current == mode
}

/** One quick preset slot of the device panel, with the last-applied mark of the current connection. */
@Serializable
data class WidgetPreset(val name: String, val lastApplied: Boolean = false)

/** Everything a device widget draws from the current connection. */
@Serializable
sealed class DeviceWidgetState {
    abstract val deviceName: String

    @Serializable
    data class Disconnected(override val deviceName: String = "", val deviceModel: String = "") : DeviceWidgetState()

    @Serializable
    data class Connecting(override val deviceName: String) : DeviceWidgetState()

    @Serializable
    data class Recovering(override val deviceName: String) : DeviceWidgetState()

    @Serializable
    data class Lost(override val deviceName: String, val macAddress: String) : DeviceWidgetState()

    @Serializable
    data class Connected(
        override val deviceName: String,
        val deviceModel: String,
        val batteryCells: List<WidgetBattery>,
        val soundModes: WidgetSoundModes,
        val presets: List<WidgetPreset> = emptyList(),
        val oneEarbudOnly: Boolean = false,
    ) : DeviceWidgetState()
}

/** The battery cells the widgets draw for the device, keeping a position for every reading. */
internal fun widgetBatteryCells(snapshot: Map<String, Setting>): List<WidgetBattery> {
    val oneEarbudOnly = clientIsOnlyOneEarbudConnected(snapshot)
    val reported = clientWithMissingEarbud(clientDeviceBatteries(snapshot), oneEarbudOnly)
    val byCell = reported.associateBy { it.label.toWidgetCell() }
    return if (clientIsTws(snapshot)) {
        listOf(WidgetBatteryCell.Left, WidgetBatteryCell.Right, WidgetBatteryCell.Case).map { cell ->
            byCell[cell]?.toWidgetBattery(cell) ?: WidgetBattery(cell, WIDGET_UNKNOWN_VALUE)
        }
    } else {
        // a single-battery model shows its overall charge in one cell
        listOf(
            byCell[WidgetBatteryCell.Whole]?.toWidgetBattery(WidgetBatteryCell.Whole) ?: WidgetBattery(
                WidgetBatteryCell.Whole,
                WIDGET_UNKNOWN_VALUE,
            ),
        )
    }
}

private fun ClientBatteryLabel.toWidgetCell(): WidgetBatteryCell = when (this) {
    ClientBatteryLabel.LEFT -> WidgetBatteryCell.Left
    ClientBatteryLabel.RIGHT -> WidgetBatteryCell.Right
    ClientBatteryLabel.CASE -> WidgetBatteryCell.Case
    ClientBatteryLabel.BATTERY -> WidgetBatteryCell.Whole
}

private fun ClientBattery.toWidgetBattery(cell: WidgetBatteryCell): WidgetBattery = WidgetBattery(cell, value, progress)

/** The sound modes of the snapshot, decided by the shared availability and control rules. */
internal fun widgetSoundModes(snapshot: Map<String, Setting>): WidgetSoundModes {
    val setting = snapshot["ambientSoundMode"] as? Setting.SelectSetting ?: return WidgetSoundModes()
    val modes = setting.setting.options.zip(setting.setting.localizedOptions).map { (id, label) ->
        WidgetMode(id, label)
    }
    val presentation = settingsPresentation(snapshot)
    val availability = settingAvailability(presentation.availability, "ambientSoundMode")
    val controlEnabled = presentation.controls["ambientSoundMode"] == null
    val available = availability is ClientSettingAvailability.Available && controlEnabled
    val reason = when (availability) {
        is ClientSettingAvailability.ReadOnly -> availability.reason
        is ClientSettingAvailability.Unavailable -> availability.reason
        else -> null
    }
    return WidgetSoundModes(
        current = setting.value,
        modes = modes,
        canSwitch = available && modes.size > 1,
        unavailableReason = reason,
    )
}

/** The state of a connected device, with the preset slots of this widget instance. */
internal fun connectedWidgetState(
    deviceName: String,
    deviceModel: String,
    snapshot: Map<String, Setting>,
    presetSlots: List<String>,
    availablePresetNames: Set<String>?,
    lastAppliedPreset: String?,
): DeviceWidgetState.Connected {
    val presets = presetSlots
        .filter { name -> availablePresetNames == null || name in availablePresetNames }
        .map { name -> WidgetPreset(name, lastApplied = name == lastAppliedPreset) }
    return DeviceWidgetState.Connected(
        deviceName = deviceName,
        deviceModel = deviceModel,
        batteryCells = widgetBatteryCells(snapshot),
        soundModes = widgetSoundModes(snapshot),
        presets = presets,
        oneEarbudOnly = clientIsOnlyOneEarbudConnected(snapshot),
    )
}

/** The OpenSCL4 palette the prototypes use, for both themes. */
data class WidgetPalette(
    val isDark: Boolean,
    val background: Color,
    val text: Color,
    val muted: Color,
    val primary: Color,
    val onPrimary: Color,
    val lavender: Color,
    val onLavender: Color,
    val tile: Color,
    val sage: Color,
    val onSage: Color,
    val leaf: Color,
    val battery: Color,
    val line: Color,
)

private val lightWidgetPalette = WidgetPalette(
    isDark = false,
    background = Color(0xFFF7F5F2),
    text = Color(0xFF25222A),
    muted = Color(0xFF6B6470),
    primary = Color(0xFF6740B4),
    onPrimary = Color(0xFFFFFFFF),
    lavender = Color(0xFFE9DFFC),
    onLavender = Color(0xFF402270),
    tile = Color(0xFFF0ECF4),
    sage = Color(0xFFE7EEDD),
    onSage = Color(0xFF34462D),
    leaf = Color(0xFF526347),
    battery = Color(0xFFF6F9F1),
    line = Color(0xFFDCD6E0),
)

private val darkWidgetPalette = WidgetPalette(
    isDark = true,
    background = Color(0xFF16141B),
    text = Color(0xFFF1EDF6),
    muted = Color(0xFFCAC1D1),
    primary = Color(0xFFD4BAFF),
    onPrimary = Color(0xFF311356),
    lavender = Color(0xFF403052),
    onLavender = Color(0xFFEBDDFF),
    tile = Color(0xFF231F29),
    sage = Color(0xFF303B2C),
    onSage = Color(0xFFD8E8CA),
    leaf = Color(0xFFBBCDAA),
    battery = Color(0xFF253021),
    line = Color(0xFF49414F),
)

internal fun widgetPalette(dark: Boolean): WidgetPalette = if (dark) darkWidgetPalette else lightWidgetPalette
