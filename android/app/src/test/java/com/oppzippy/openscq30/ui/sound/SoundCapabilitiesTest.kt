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
}
