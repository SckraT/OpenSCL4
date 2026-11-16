package com.oppzippy.openscq30.features.battery

/** Which battery reading the app prefers while the experimental system source is on. */
enum class SystemBatteryMode {
    /** The device's own levels, as without the feature. */
    Off,

    /** The system percentage, but only for a device with a single battery. */
    SingleBatteryOnly,

    /** The system percentage for a pair of earbuds too, at the cost of its per-earbud and case levels. */
    Always,
}
