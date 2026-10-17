package com.oppzippy.openscq30.ui.sound

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SoundCapabilitiesTest {
    @Test
    fun aCategoryOpensItsPageWhenThePageShowsAllOfItsSettings() {
        assertTrue(coveredByPage(listOf("presetEqualizerProfile", "volumeAdjustments"), BOTH_EARBUDS_SETTING_IDS))
        assertTrue(coveredByPage(listOf("ambientSoundMode", "windNoiseSuppression"), SoundCapabilities.modes))
    }

    @Test
    fun aCategoryWithASettingThePageLacksKeepsThePlainList() {
        assertFalse(coveredByPage(listOf("volumeAdjustments", "someOtherSetting"), BOTH_EARBUDS_SETTING_IDS))
    }

    @Test
    fun anEmptyCategoryKeepsThePlainList() {
        assertFalse(coveredByPage(emptyList(), BOTH_EARBUDS_SETTING_IDS))
    }

    @Test
    fun theEqualizerOpensItsPageWhileOneEarbudIsConnected() {
        // the device offers no equalizer settings then, but profiles can still be imported and exported on the page
        assertTrue(equalizerOpensItsPage(emptyList(), oneEarbudOnly = true))
        assertTrue(equalizerOpensItsPage(listOf("volumeAdjustments"), oneEarbudOnly = false))
        assertFalse(equalizerOpensItsPage(emptyList(), oneEarbudOnly = false))
        assertFalse(equalizerOpensItsPage(listOf("volumeAdjustments", "someOtherSetting"), oneEarbudOnly = true))
    }
}
