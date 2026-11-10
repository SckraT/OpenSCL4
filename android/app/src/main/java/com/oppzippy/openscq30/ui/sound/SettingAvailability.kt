package com.oppzippy.openscq30.ui.sound

import androidx.annotation.StringRes
import androidx.compose.runtime.staticCompositionLocalOf
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.lib.bindings.AvailabilityReason
import com.oppzippy.openscq30.lib.bindings.ClientSettingAvailability
import com.oppzippy.openscq30.lib.bindings.ClientSettingControl
import com.oppzippy.openscq30.lib.bindings.ClientSettingsPresentation
import com.oppzippy.openscq30.lib.bindings.clientSettingsPresentation
import com.oppzippy.openscq30.lib.wrapper.Setting

internal const val EQUALIZER_AVAILABILITY_ID = "volumeAdjustments"

/** Include the equalizer even when the device stops reporting its settings with one earbud connected. */
internal fun settingsPresentation(values: Map<String, Setting>): ClientSettingsPresentation =
    clientSettingsPresentation(values, listOf(EQUALIZER_AVAILABILITY_ID))

internal val LocalSettingAvailability = staticCompositionLocalOf<Map<String, ClientSettingAvailability>> {
    emptyMap()
}

internal val LocalSettingControls = staticCompositionLocalOf<Map<String, ClientSettingControl>> { emptyMap() }

@StringRes
internal fun availabilityReasonText(reason: AvailabilityReason): Int = when (reason) {
    AvailabilityReason.REQUIRES_BOTH_EARBUDS -> R.string.setting_availability_requires_both_earbuds
    AvailabilityReason.NO_READ_BACK -> R.string.setting_availability_no_read_back
}
