package com.oppzippy.openscq30.lib.wrapper

import com.oppzippy.openscq30.lib.bindings.AvailabilityReason
import com.oppzippy.openscq30.lib.bindings.ClientSettingAvailability

fun ClientSettingAvailability.isAvailable(): Boolean = this is ClientSettingAvailability.Available

fun ClientSettingAvailability.blockedReason(): AvailabilityReason? = when (this) {
    ClientSettingAvailability.Available -> null
    is ClientSettingAvailability.ReadOnly -> reason
    is ClientSettingAvailability.Unavailable -> reason
}
