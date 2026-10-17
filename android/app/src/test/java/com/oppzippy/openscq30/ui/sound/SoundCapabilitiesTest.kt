package com.oppzippy.openscq30.ui.sound

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SoundCapabilitiesTest {
    @Test
    fun theEqualizerOpensItsPageWhenThePageShowsAllOfItsSettings() {
        assertTrue(equalizerOpensItsPage(listOf("presetEqualizerProfile", "volumeAdjustments"), oneEarbudOnly = false))
        assertFalse(equalizerOpensItsPage(listOf("volumeAdjustments", "someOtherSetting"), oneEarbudOnly = false))
    }

    @Test
    fun theEqualizerOpensItsPageWhileOneEarbudIsConnected() {
        // the device offers no equalizer settings then, but profiles can still be imported and exported on the page
        assertTrue(equalizerOpensItsPage(emptyList(), oneEarbudOnly = true))
        assertFalse(equalizerOpensItsPage(emptyList(), oneEarbudOnly = false))
        assertFalse(equalizerOpensItsPage(listOf("volumeAdjustments", "someOtherSetting"), oneEarbudOnly = true))
    }
}
