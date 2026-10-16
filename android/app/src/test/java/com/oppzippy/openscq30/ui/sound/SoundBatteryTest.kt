package com.oppzippy.openscq30.ui.sound

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class SoundBatteryTest {
    @Test
    fun nativeBatteryFractionsRespectTheDeviceResolution() {
        assertEquals(0.83f, batteryProgress("83/100")!!, 0.0001f)
        assertEquals(0.7f, batteryProgress("7/10")!!, 0.0001f)
        assertEquals(1f, batteryProgress("5/5")!!, 0.0001f)
        assertEquals(0f, batteryProgress("0/100")!!, 0.0001f)
    }

    @Test
    fun malformedReadingsDoNotRenderAnInventedLevel() {
        for (raw in listOf("", "unknown", "1/0", "NaN/100", "50/Infinity", "1/2/3")) {
            assertNull(raw, batteryProgress(raw))
        }
    }
}
