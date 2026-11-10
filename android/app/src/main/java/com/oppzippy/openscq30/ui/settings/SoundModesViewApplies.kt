package com.oppzippy.openscq30.ui.settings

import com.oppzippy.openscq30.features.soundcoredevice.service.ConnectionStatus
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.ui.sound.supportsSoundModeScale

/**
 * Whether the sound mode view setting applies: only a device the scale can show has two views to choose between.
 * With nothing connected there is nothing to choose for, since the choice belongs to a device.
 */
internal fun soundModesViewApplies(model: String?, values: Map<String, Setting>): Boolean =
    model != null && supportsSoundModeScale(model, values)

/**
 * The same answer for both ways into the app settings: the choice belongs to the device the app controls, not to the
 * screen the user came from. Nothing connected, or a device that is still connecting, has no settings to ask about.
 */
internal fun soundModesViewApplies(
    connectionStatus: ConnectionStatus,
    settings: List<Pair<String, Setting>>,
): Boolean = soundModesViewApplies(connectionStatus.deviceManager?.device?.model(), settings.toMap())
