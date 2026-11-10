package com.oppzippy.openscq30.ui.sound

import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.features.preferences.EarbudsColor
import com.oppzippy.openscq30.lib.bindings.DeviceArt
import com.oppzippy.openscq30.lib.bindings.DeviceKind
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
        assertEquals(R.drawable.sound_art_stem_premium_silver, deviceArtImage(DeviceArt.LIBERTY_STEM, lightColorScheme))
    }

    @Test
    fun darkThemeShowsTheBlackIllustration() {
        assertEquals(R.drawable.sound_art_stem_premium_black, deviceArtImage(DeviceArt.LIBERTY_STEM, darkColorScheme))
    }

    @Test
    fun chosenColorWinsOverTheTheme() {
        assertEquals(
            R.drawable.sound_art_motion_plus_black,
            deviceArtImage(DeviceArt.MOTION_PLUS, lightColorScheme, EarbudsColor.Black),
        )
        assertEquals(
            R.drawable.sound_art_headphones_silver,
            deviceArtImage(DeviceArt.HEADPHONES, darkColorScheme, EarbudsColor.Silver),
        )
    }

    @Test
    fun modelsUseTheIllustrationOfTheirShape() {
        assertEquals(DeviceArt.LIBERTY_STEM, deviceArt("SoundcoreA3954"))
        assertEquals(DeviceArt.HEADPHONES, deviceArt("SoundcoreA3028"))
        assertEquals(DeviceArt.MOTION_PLUS, deviceArt("SoundcoreA3116"))
        assertEquals(DeviceArt.CLASSIC_STEM, deviceArt("SoundcoreD1202C"))
        assertEquals(DeviceArt.OPEN_STEM, deviceArt("SoundcoreA3945"))
    }

    @Test
    fun modelsAreGroupedByKind() {
        assertEquals(DeviceKind.EARBUDS, deviceArt("SoundcoreA3954")?.kind)
        assertEquals(DeviceKind.EARBUDS, deviceArt("SoundcoreA3968")?.kind)
        assertEquals(DeviceKind.OVER_EAR, deviceArt("SoundcoreA3028")?.kind)
        assertEquals(DeviceKind.OPEN_EAR, deviceArt("SoundcoreA3330")?.kind)
        assertEquals(DeviceKind.OPEN_EAR, deviceArt("SoundcoreA3876")?.kind)
        assertEquals(DeviceKind.SPEAKER, deviceArt("SoundcoreA3116")?.kind)
    }

    @Test
    fun ldacIsTheDefaultQuickToggle() {
        assertEquals(listOf("ldac"), quickToggleIds(null, listOf("wearingDetection", "ldac")))
        assertEquals(emptyList<String>(), quickToggleIds(null, listOf("wearingDetection")))
    }

    @Test
    fun chosenQuickTogglesKeepTheirOrderAndDropUnavailableOnes() {
        assertEquals(
            listOf("wearingDetection", "ldac"),
            quickToggleIds("wearingDetection,removed,ldac,ldac", listOf("ldac", "wearingDetection")),
        )
        assertEquals(emptyList<String>(), quickToggleIds("", listOf("ldac")))
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
