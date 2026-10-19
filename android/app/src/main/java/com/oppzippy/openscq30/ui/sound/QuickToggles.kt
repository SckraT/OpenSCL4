package com.oppzippy.openscq30.ui.sound

import com.oppzippy.openscq30.lib.bindings.clientQuickToggleIds

internal fun quickToggleIds(stored: String?, available: List<String>): List<String> =
    clientQuickToggleIds(stored?.split(','), available)
