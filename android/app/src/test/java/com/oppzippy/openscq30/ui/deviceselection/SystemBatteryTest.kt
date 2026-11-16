package com.oppzippy.openscq30.ui.deviceselection

import com.oppzippy.openscq30.R
import org.junit.Assert.assertEquals
import org.junit.Test

class SystemBatteryTest {
    @Test
    fun the_system_percentage_is_one_battery_tile() {
        for (percent in listOf(0, 1, 50, 99, 100)) {
            val batteries = systemBattery(percent, isTws = false)
            assertEquals("one tile for $percent", 1, batteries.size)
            assertEquals(R.string.sound_overview_battery, batteries[0].label)
            assertEquals("$percent%", batteries[0].value)
            assertEquals(percent / 100f, batteries[0].progress!!, 0.0001f)
        }
    }

    @Test
    fun a_pair_says_the_number_comes_from_android() {
        assertEquals(R.string.sound_overview_battery_android, systemBattery(80, isTws = true)[0].label)
        assertEquals(R.string.sound_overview_battery, systemBattery(80, isTws = false)[0].label)
    }

    @Test
    fun the_progress_stays_inside_the_bar() {
        assertEquals(0f, systemBattery(-1, isTws = false)[0].progress!!, 0.0001f)
        assertEquals(1f, systemBattery(101, isTws = false)[0].progress!!, 0.0001f)
    }
}
