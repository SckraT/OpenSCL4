package com.oppzippy.openscq30.ui.sound

import com.oppzippy.openscq30.lib.bindings.clientCaseSettingIds
import com.oppzippy.openscq30.lib.bindings.clientEqualizerOpensItsPage
import com.oppzippy.openscq30.lib.bindings.clientIsControl
import com.oppzippy.openscq30.lib.bindings.clientIsTws
import com.oppzippy.openscq30.lib.bindings.clientSoundModeSettingIds
import com.oppzippy.openscq30.lib.bindings.clientStrengthIsSetByUser
import com.oppzippy.openscq30.lib.wrapper.Setting

internal object SoundCapabilities {
    val case by lazy { clientCaseSettingIds().toSet() }
    val modes by lazy { clientSoundModeSettingIds().toSet() }
    fun isControl(id: String): Boolean = clientIsControl(id)
}

internal fun isTws(values: Map<String, Setting>): Boolean = clientIsTws(values)

internal fun strengthIsSetByUser(values: Map<String, Setting>, modeId: String): Boolean =
    clientStrengthIsSetByUser(values, modeId)

internal fun equalizerOpensItsPage(categorySettingIds: Collection<String>, oneEarbudOnly: Boolean): Boolean =
    clientEqualizerOpensItsPage(categorySettingIds.toList(), oneEarbudOnly)
