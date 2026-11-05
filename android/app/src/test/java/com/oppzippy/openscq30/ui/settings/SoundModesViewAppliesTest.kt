package com.oppzippy.openscq30.ui.settings

import com.oppzippy.openscq30.lib.wrapper.Range
import com.oppzippy.openscq30.lib.wrapper.Select
import com.oppzippy.openscq30.lib.wrapper.Setting
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SoundModesViewAppliesTest {
    private val modes = listOf("NoiseCanceling", "Transparency", "Normal")

    /** A device the scale can show: all three modes and one strength for the current one. */
    private fun scaleDevice(mode: String = "Normal") = mapOf(
        "ambientSoundMode" to Setting.SelectSetting(Select(modes, modes), mode),
        "manualNoiseCanceling" to Setting.I32RangeSetting(Range(1, 5, 1), 3),
        "manualTransparency" to Setting.I32RangeSetting(Range(1, 5, 1), 2),
    )

    @Test
    fun aDeviceShownAsOneScaleHasTwoViewsToChooseBetween() {
        assertTrue(soundModesViewApplies("SoundcoreA3954", scaleDevice()))
    }

    @Test
    fun aDeviceWithoutTheScaleHasNothingToChoose() {
        // Space One Pro offers the modes separately and never the scale, so both views would look the same.
        assertFalse(soundModesViewApplies("SoundcoreA3062", scaleDevice()))
        // the only model with the scale still needs all three modes before the scale can show them
        val noNormal = scaleDevice() + (
            "ambientSoundMode" to Setting.SelectSetting(
                Select(listOf("NoiseCanceling", "Transparency"), listOf("a", "b")),
                "NoiseCanceling",
            )
            )
        assertFalse(soundModesViewApplies("SoundcoreA3954", noNormal))
    }

    @Test
    fun withNothingConnectedThereIsNothingToChoose() {
        assertFalse(soundModesViewApplies(null, emptyMap()))
    }
}
