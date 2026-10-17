package com.oppzippy.openscq30.ui.sound

import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.features.preferences.EarbudsColor
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.ui.theme.darkColorScheme
import com.oppzippy.openscq30.ui.theme.lightColorScheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SoundDeviceArtTest {
    @Test
    fun lightThemeShowsTheSilverIllustration() {
        assertEquals(R.drawable.sound_art_stem_premium_silver, deviceArtImage(DeviceArt.LibertyStem, lightColorScheme))
    }

    @Test
    fun darkThemeShowsTheBlackIllustration() {
        assertEquals(R.drawable.sound_art_stem_premium_black, deviceArtImage(DeviceArt.LibertyStem, darkColorScheme))
    }

    @Test
    fun chosenColorWinsOverTheTheme() {
        assertEquals(
            R.drawable.sound_art_motion_plus_black,
            deviceArtImage(DeviceArt.MotionPlus, lightColorScheme, EarbudsColor.Black),
        )
        assertEquals(
            R.drawable.sound_art_headphones_silver,
            deviceArtImage(DeviceArt.Headphones, darkColorScheme, EarbudsColor.Silver),
        )
    }

    @Test
    fun modelsUseTheIllustrationOfTheirShape() {
        assertEquals(DeviceArt.LibertyStem, deviceArt("SoundcoreA3954"))
        assertEquals(DeviceArt.Headphones, deviceArt("SoundcoreA3028"))
        assertEquals(DeviceArt.MotionPlus, deviceArt("SoundcoreA3116"))
        assertEquals(DeviceArt.ClassicStem, deviceArt("SoundcoreD1202C"))
        assertEquals(DeviceArt.OpenStem, deviceArt("SoundcoreA3945"))
    }

    @Test
    fun unknownModelsHaveNoIllustration() {
        assertNull(deviceArt("SoundcoreA9999"))
        assertNull(deviceArt("A3954"))
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
