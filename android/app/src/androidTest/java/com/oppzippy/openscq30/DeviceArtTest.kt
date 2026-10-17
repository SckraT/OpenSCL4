package com.oppzippy.openscq30

import com.oppzippy.openscq30.lib.bindings.deviceModels
import com.oppzippy.openscq30.ui.sound.DeviceArt
import com.oppzippy.openscq30.ui.sound.deviceArt
import org.junit.Assert.assertEquals
import org.junit.Test

class DeviceArtTest {
    @Test
    fun everyDeviceModelHasAnIllustration() {
        // The development model is not real hardware and keeps the generic icon.
        val models = deviceModels() - "SoundcoreDevelopment"
        assertEquals(emptyList<String>(), models.filter { deviceArt(it) == null })
    }

    @Test
    fun everyIllustrationShowsAtLeastOneDeviceModel() {
        assertEquals(DeviceArt.entries.toSet(), deviceModels().mapNotNull { deviceArt(it) }.toSet())
    }
}
