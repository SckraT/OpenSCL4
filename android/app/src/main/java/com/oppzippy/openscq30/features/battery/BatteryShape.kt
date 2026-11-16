package com.oppzippy.openscq30.features.battery

import com.oppzippy.openscq30.lib.wrapper.Setting

/**
 * What the device's own battery readings look like, which decides whether the system percentage may replace them.
 *
 * A device with one battery has nothing to lose: the system percentage is finer than a coarse level. A pair of
 * earbuds loses the per-earbud split, and one that already reports exact percentages loses precision as well.
 */
data class BatteryShape(
    val isTws: Boolean,
    /** The denominator of the raw reading: 5, 10 or 100. Null when the format is not recognised. */
    val scale: Int?,
)

/** The shape of the battery settings the device currently reports. */
fun batteryShape(settings: Map<String, Setting>): BatteryShape {
    fun scaleOf(id: String): Int? {
        val raw = (settings[id] as? Setting.InformationSetting)?.value ?: return null
        return raw.substringAfter('/', "").toIntOrNull()?.takeIf { it > 0 }
    }
    val isTws = settings.containsKey("batteryLevelLeft") || settings.containsKey("batteryLevelRight")
    val scale = scaleOf("batteryLevel") ?: scaleOf("batteryLevelLeft") ?: scaleOf("batteryLevelRight")
    return BatteryShape(isTws, scale)
}

/** Whether the system percentage should be shown instead of the device's own levels. */
fun useSystemBattery(mode: SystemBatteryMode, shape: BatteryShape, systemPercent: Int?): Boolean {
    if (systemPercent == null || mode == SystemBatteryMode.Off) return false
    // the pair reports exact percentages: the system's single number would be coarser, not finer
    if (shape.isTws && (shape.scale ?: 0) >= 100) return false
    return if (shape.isTws) mode == SystemBatteryMode.Always else true
}

/** Whether the system percentage can improve on this device at all. */
fun systemBatteryApplies(shape: BatteryShape): Boolean = !(shape.isTws && (shape.scale ?: 0) >= 100)

/** Whether the system percentage may also replace the pair's levels, at the cost of the per-earbud split. */
fun systemBatteryTwsApplies(shape: BatteryShape): Boolean = shape.isTws && systemBatteryApplies(shape)
