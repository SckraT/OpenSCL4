package com.oppzippy.openscq30.features.phonelink

import kotlinx.coroutines.flow.StateFlow

/**
 * Phone-link signals for the currently visible device list. [start] seeds a profile scan and subscribes to live ACL
 * events; [refresh] re-runs the profile scan, for example when the list resumes.
 */
interface PhoneLinkMonitor {
    val snapshot: StateFlow<PhoneLinkSignals>

    fun start()

    fun stop()

    fun refresh()
}
