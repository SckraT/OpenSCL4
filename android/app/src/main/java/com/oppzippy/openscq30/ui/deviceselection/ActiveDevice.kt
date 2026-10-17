package com.oppzippy.openscq30.ui.deviceselection

import androidx.annotation.StringRes
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.lib.wrapper.PairedDevice
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.ui.sound.batteryProgress

enum class ActiveDeviceState {
    Connecting,
    Connected,
}

data class DeviceBattery(@StringRes val label: Int, val value: String, val progress: Float?)

/** The device the app is connecting or connected to, with its battery levels once they are known. */
data class ActiveDevice(
    val macAddress: String,
    val state: ActiveDeviceState,
    val batteries: List<DeviceBattery> = emptyList(),
    /** Only one earbud is connected, and the user wants to be told. */
    val oneEarbudOnly: Boolean = false,
)

/**
 * The device at the top of the list: the one the app is connected or connecting to, otherwise the one the user opened
 * last, otherwise the first saved device.
 */
internal fun featuredDevice(
    devices: List<PairedDevice>,
    activeMacAddress: String?,
    lastMacAddress: String?,
): PairedDevice? = devices.firstOrNull { it.macAddress.equals(activeMacAddress, ignoreCase = true) }
    ?: devices.firstOrNull { it.macAddress.equals(lastMacAddress, ignoreCase = true) }
    ?: devices.firstOrNull()

/** Battery levels of the connected device, in the same order as on the device overview. */
fun deviceBatteries(settings: Map<String, Setting>): List<DeviceBattery> = listOf(
    "batteryLevelLeft" to R.string.sound_overview_left,
    "batteryLevelRight" to R.string.sound_overview_right,
    "caseBatteryLevel" to R.string.sound_overview_case,
    "batteryLevel" to R.string.sound_overview_battery,
).mapNotNull { (id, label) ->
    (settings[id] as? Setting.InformationSetting)?.let {
        DeviceBattery(label, it.translatedValue, batteryProgress(it.value))
    }
}

/**
 * With only one earbud connected, the device reports no level for the other one; show it as a dash in its place so it
 * is clear which earbud is missing.
 */
fun withMissingEarbud(batteries: List<DeviceBattery>, oneEarbudOnly: Boolean): List<DeviceBattery> {
    if (!oneEarbudOnly) return batteries
    val left = R.string.sound_overview_left
    val right = R.string.sound_overview_right
    val hasLeft = batteries.any { it.label == left }
    val hasRight = batteries.any { it.label == right }
    if (hasLeft == hasRight) return batteries
    val missing = DeviceBattery(if (hasLeft) right else left, "—", null)
    val earbuds = batteries.filter { it.label == left || it.label == right } + missing
    return earbuds.sortedBy { if (it.label == left) 0 else 1 } + batteries.filter { it.label != left && it.label != right }
}

/** Model names start with the brand, which the list shows separately: "Soundcore Liberty 4 Pro". */
internal fun splitBrand(modelName: String): Pair<String?, String> {
    val brand = "Soundcore "
    return if (modelName.startsWith(brand) && modelName.length > brand.length) {
        brand.trim() to modelName.removePrefix(brand)
    } else {
        null to modelName
    }
}
