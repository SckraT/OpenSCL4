package com.oppzippy.openscq30.ui.sound

import com.oppzippy.openscq30.lib.wrapper.Range
import com.oppzippy.openscq30.lib.wrapper.Select
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SoundDeviceEditorsTest {
    @Test
    fun safeVolumeSendsOnlySupportedSteps() {
        val range = Range(start = 75, end = 100, step = 5)
        assertEquals(75, snapSoundRangeValue(range, 75f))
        assertEquals(85, snapSoundRangeValue(range, 83f))
        assertEquals(90, snapSoundRangeValue(range, 92f))
        assertEquals(100, snapSoundRangeValue(range, 100f))
    }

    @Test
    fun rangesQuantizeRelativeToTheirStart() {
        val range = Range(start = 3, end = 15, step = 4)
        assertEquals(3, snapSoundRangeValue(range, -10f))
        assertEquals(7, snapSoundRangeValue(range, 8f))
        assertEquals(15, snapSoundRangeValue(range, 100f))
    }

    @Test
    fun unknownSelectionKeepsItsActualValue() {
        val select = Select(options = listOf("Music", "Movie"), localizedOptions = listOf("Музыка", "Фильмы"))
        assertEquals("Музыка", select.localizedOption("Music"))
        assertEquals("NewBackendMode", select.localizedOption("NewBackendMode"))
        assertNull(select.localizedOption(null))
    }

    @Test
    fun absentLocalizedLabelsUseBackendOptions() {
        val select = Select(options = listOf("English", "Japanese"), localizedOptions = listOf("English"))
        assertEquals("Japanese", select.localizedOption("Japanese"))
    }

    @Test
    fun sideTabsIncludeCallGesturesAndExcludeOtherCapabilities() {
        assertTrue(isSideControl("leftSlideUp", "left"))
        assertTrue(isSideControl("rightLongPressDuringCall", "right"))
        assertFalse(isSideControl("rightSlideUp", "left"))
        assertFalse(isSideControl("batteryLevelLeft", "left"))
        assertFalse(isSideControl("leftFutureFeature", "left"))
    }
}
