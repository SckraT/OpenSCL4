package com.oppzippy.openscq30.ui.sound

import com.oppzippy.openscq30.lib.wrapper.Equalizer
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class SoundEqualizerValuesTest {
    private val equalizer = Equalizer(listOf(100u), fractionDigits = 1, min = -120, max = 134)

    @Test
    fun acceptsDeviceLimitsAndRussianDecimalSeparator() {
        assertEquals((-120).toShort(), parseEqualizerValue("-12", equalizer))
        assertEquals(134.toShort(), parseEqualizerValue("13.4", equalizer))
        assertEquals(25.toShort(), parseEqualizerValue(" 2,5 ", equalizer))
        assertEquals(0.toShort(), parseEqualizerValue("0.0", equalizer))
    }

    @Test
    fun rejectsValuesThatWouldOverflowOrRequireRounding() {
        listOf("13.5", "-12.1", "6553.6", "1.01", "", "-", "NaN", "1.2.3").forEach {
            assertNull(it, parseEqualizerValue(it, equalizer))
        }
    }

    @Test
    fun formatsStoredValuesWithoutChangingDevicePrecision() {
        assertEquals("13.4", formatEqualizerValue(134, 1))
        assertEquals("-12", formatEqualizerValue(-120, 1))
        assertEquals("0.25", formatEqualizerValue(25, 2))
        assertEquals("0", formatEqualizerValue(0, 1))
    }
}
