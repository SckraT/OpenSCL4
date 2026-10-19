package com.oppzippy.openscq30.ui.sound

import com.oppzippy.openscq30.lib.bindings.clientCaseSettingIds
import com.oppzippy.openscq30.lib.bindings.clientEqualizerOpensItsPage
import com.oppzippy.openscq30.lib.bindings.clientIsControl
import com.oppzippy.openscq30.lib.bindings.clientSoundModeSettingIds

internal object SoundCapabilities {
    val case by lazy { clientCaseSettingIds().toSet() }
    val modes by lazy { clientSoundModeSettingIds().toSet() }
    fun isControl(id: String): Boolean = clientIsControl(id)
}

internal fun equalizerOpensItsPage(categorySettingIds: Collection<String>, oneEarbudOnly: Boolean): Boolean =
    clientEqualizerOpensItsPage(categorySettingIds.toList(), oneEarbudOnly)
