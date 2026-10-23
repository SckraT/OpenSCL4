package com.oppzippy.openscq30.ui.sound

import androidx.annotation.StringRes
import androidx.compose.runtime.staticCompositionLocalOf
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.lib.bindings.AvailabilityReason
import com.oppzippy.openscq30.lib.bindings.ClientSettingAvailability
import com.oppzippy.openscq30.lib.bindings.clientSettingAvailability
import com.oppzippy.openscq30.lib.wrapper.Setting

internal fun settingAvailability(values: Map<String, Setting>, settingId: String): ClientSettingAvailability =
    clientSettingAvailability(values, settingId)

/** Every setting's availability in the device's current state, by setting id. */
internal fun settingAvailability(values: Map<String, Setting>): Map<String, ClientSettingAvailability> =
    values.keys.associateWith { clientSettingAvailability(values, it) }

internal val LocalSettingAvailability = staticCompositionLocalOf<Map<String, ClientSettingAvailability>> {
    emptyMap()
}

@StringRes
internal fun availabilityReasonText(reason: AvailabilityReason): Int = when (reason) {
    AvailabilityReason.REQUIRES_BOTH_EARBUDS -> R.string.setting_availability_requires_both_earbuds
    AvailabilityReason.NO_READ_BACK -> R.string.setting_availability_no_read_back
}
