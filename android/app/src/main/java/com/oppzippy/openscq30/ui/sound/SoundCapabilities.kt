package com.oppzippy.openscq30.ui.sound

import com.oppzippy.openscq30.lib.bindings.clientEqualizerOpensItsPage
import com.oppzippy.openscq30.lib.bindings.clientIsTws
import com.oppzippy.openscq30.lib.wrapper.Setting

internal fun isTws(values: Map<String, Setting>): Boolean = clientIsTws(values)

internal fun equalizerOpensItsPage(categorySettingIds: Collection<String>, oneEarbudOnly: Boolean): Boolean =
    clientEqualizerOpensItsPage(categorySettingIds.toList(), oneEarbudOnly)
