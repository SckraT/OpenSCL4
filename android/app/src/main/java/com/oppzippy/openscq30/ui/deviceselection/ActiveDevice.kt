package com.oppzippy.openscq30.ui.deviceselection

import androidx.annotation.StringRes
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.lib.bindings.ClientBattery
import com.oppzippy.openscq30.lib.bindings.ClientBatteryLabel
import com.oppzippy.openscq30.lib.bindings.clientDeviceBatteries
import com.oppzippy.openscq30.lib.bindings.clientWithMissingEarbud
import com.oppzippy.openscq30.lib.wrapper.PairedDevice
import com.oppzippy.openscq30.lib.wrapper.Setting

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

/** The app-control axis for the device list, derived from the active device screen state. */
fun ActiveDevice?.toAppControlSnapshot(): AppControlSnapshot = if (this == null) {
    AppControlSnapshot()
} else {
    AppControlSnapshot(
        macAddress = macAddress,
        phase = when (state) {
            ActiveDeviceState.Connecting -> AppControl.Connecting
            ActiveDeviceState.Connected -> AppControl.Controlling
        },
    )
}

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

private val batteryLabels = mapOf(
    ClientBatteryLabel.LEFT to R.string.sound_overview_left,
    ClientBatteryLabel.RIGHT to R.string.sound_overview_right,
    ClientBatteryLabel.CASE to R.string.sound_overview_case,
    ClientBatteryLabel.BATTERY to R.string.sound_overview_battery,
)

private fun ClientBattery.toDeviceBattery(): DeviceBattery =
    DeviceBattery(batteryLabels.getValue(label), value, progress)

fun deviceBatteries(settings: Map<String, Setting>): List<DeviceBattery> =
    clientDeviceBatteries(settings).map { it.toDeviceBattery() }

fun withMissingEarbud(batteries: List<DeviceBattery>, oneEarbudOnly: Boolean): List<DeviceBattery> =
    clientWithMissingEarbud(
        batteries.map { battery ->
            ClientBattery(
                batteryLabels.entries.first { it.value == battery.label }.key,
                battery.value,
                battery.progress,
            )
        },
        oneEarbudOnly,
    ).map { it.toDeviceBattery() }

/** Model names start with the brand, which the list shows separately: "Soundcore Liberty 4 Pro". */
internal fun splitBrand(modelName: String): Pair<String?, String> {
    val brand = "Soundcore "
    return if (modelName.startsWith(brand) && modelName.length > brand.length) {
        brand.trim() to modelName.removePrefix(brand)
    } else {
        null to modelName
    }
}
