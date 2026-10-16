package com.oppzippy.openscq30.ui.sound

import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.features.preferences.EarbudsColor
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.ui.theme.darkColorScheme
import com.oppzippy.openscq30.ui.theme.lightColorScheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SoundEarbudsArtTest {
    @Test
    fun lightThemeShowsTheSilverEarbuds() {
        assertEquals(R.drawable.sound_liberty_4_pro_silver, earbudsArtImage(lightColorScheme))
    }

    @Test
    fun darkThemeShowsTheBlackEarbuds() {
        assertEquals(R.drawable.sound_liberty_4_pro_black, earbudsArtImage(darkColorScheme))
    }

    @Test
    fun chosenColorWinsOverTheTheme() {
        assertEquals(R.drawable.sound_liberty_4_pro_black, earbudsArtImage(lightColorScheme, EarbudsColor.Black))
        assertEquals(R.drawable.sound_liberty_4_pro_silver, earbudsArtImage(darkColorScheme, EarbudsColor.Silver))
    }

    private fun twsStatus(value: String) = mapOf("twsStatus" to Setting.InformationSetting(value, value))

    @Test
    fun oneEarbudIsDetectedFromTheTwsStatus() {
        assertTrue(isOnlyOneEarbudConnected(twsStatus("Disconnected")))
        assertFalse(isOnlyOneEarbudConnected(twsStatus("Connected")))
    }

    @Test
    fun devicesWithoutTwsStatusAreNotTreatedAsOneEarbud() {
        assertFalse(isOnlyOneEarbudConnected(emptyMap()))
    }

    @Test
    fun onlyTheEqualizerCategoryNeedsBothEarbuds() {
        assertTrue(categoryNeedsBothEarbuds("equalizer"))
        assertFalse(categoryNeedsBothEarbuds("equalizerImportExport"))
        assertFalse(categoryNeedsBothEarbuds("soundModes"))
    }
}
