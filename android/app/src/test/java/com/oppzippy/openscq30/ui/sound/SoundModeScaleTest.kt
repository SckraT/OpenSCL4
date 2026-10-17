package com.oppzippy.openscq30.ui.sound

import com.oppzippy.openscq30.lib.wrapper.Range
import com.oppzippy.openscq30.lib.wrapper.Select
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.lib.wrapper.toValue
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SoundModeScaleTest {
    private val modes = listOf("NoiseCanceling", "Transparency", "Normal", "AirplaneMode")

    private fun device(mode: String, noiseCanceling: Int = 3, transparency: Int = 2) = mapOf(
        "ambientSoundMode" to Setting.SelectSetting(Select(modes, modes), mode),
        "manualNoiseCanceling" to Setting.I32RangeSetting(Range(1, 5, 1), noiseCanceling),
        "manualTransparency" to Setting.I32RangeSetting(Range(1, 5, 1), transparency),
    )

    @Test
    fun theScaleIsForModelsWithOneSliderAndAllThreeModes() {
        assertTrue(supportsSoundModeScale("SoundcoreA3954", device("Normal")))
        // the device offers only the strength of the current mode
        assertTrue(supportsSoundModeScale("SoundcoreA3954", device("Normal") - "manualTransparency"))
        assertFalse(supportsSoundModeScale("SoundcoreA3951", device("Normal")))
        val noNormal = device("Normal") + (
            "ambientSoundMode" to Setting.SelectSetting(
                Select(listOf("NoiseCanceling", "Transparency"), listOf("a", "b")),
                "NoiseCanceling",
            )
            )
        assertFalse(supportsSoundModeScale("SoundcoreA3954", noNormal))
    }

    @Test
    fun theCurrentModeIsAPositionOnTheScale() {
        assertEquals(6, soundModeScaleValue(device("Normal")))
        assertEquals(9, soundModeScaleValue(device("NoiseCanceling", noiseCanceling = 3)))
        assertEquals(11, soundModeScaleValue(device("NoiseCanceling", noiseCanceling = 5)))
        assertEquals(4, soundModeScaleValue(device("Transparency", transparency = 2)))
        assertEquals(1, soundModeScaleValue(device("Transparency", transparency = 5)))
        assertNull(soundModeScaleValue(device("AirplaneMode")))
    }

    @Test
    fun aModeWithoutItsStrengthIsAtTheWeakestPosition() {
        assertEquals(7, soundModeScaleValue(device("NoiseCanceling") - "manualNoiseCanceling"))
        assertEquals(5, soundModeScaleValue(device("Transparency") - "manualTransparency"))
    }

    @Test
    fun aPositionSetsTheModeAndItsStrength() {
        assertEquals(
            listOf("ambientSoundMode" to "Transparency".toValue(), "manualTransparency" to 5.toValue()),
            soundModeScaleSettings(1),
        )
        assertEquals(listOf("ambientSoundMode" to "Normal".toValue()), soundModeScaleSettings(6))
        assertEquals(
            listOf("ambientSoundMode" to "NoiseCanceling".toValue(), "manualNoiseCanceling" to 1.toValue()),
            soundModeScaleSettings(7),
        )
    }

    @Test
    fun everyPositionReadsBackAsItself() {
        (1..11).forEach { position ->
            val settings = soundModeScaleSettings(position).toMap()
            val mode = (settings["ambientSoundMode"] as com.oppzippy.openscq30.lib.wrapper.Value.StringValue).value
            val strength = (
                (settings["manualNoiseCanceling"] ?: settings["manualTransparency"])
                    as? com.oppzippy.openscq30.lib.wrapper.Value.I32Value
                )?.value
            val values = device(
                mode,
                noiseCanceling = if (mode == "NoiseCanceling") strength!! else 1,
                transparency = if (mode == "Transparency") strength!! else 1,
            )
            assertEquals(position, soundModeScaleValue(values))
        }
    }
}
