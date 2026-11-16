package com.oppzippy.openscq30.features.battery

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class BatteryShapeTest {
    @Test
    fun the_system_percentage_is_only_used_where_it_helps() {
        val single = BatteryShape(isTws = false, scale = 5)
        val coarsePair = BatteryShape(isTws = true, scale = 5)
        val exactPair = BatteryShape(isTws = true, scale = 100)

        assertFalse(useSystemBattery(SystemBatteryMode.Off, single, 80))
        assertFalse(useSystemBattery(SystemBatteryMode.SingleBatteryOnly, single, null))
        assertTrue(useSystemBattery(SystemBatteryMode.SingleBatteryOnly, single, 80))
        assertFalse(useSystemBattery(SystemBatteryMode.SingleBatteryOnly, coarsePair, 80))
        assertTrue(useSystemBattery(SystemBatteryMode.Always, coarsePair, 80))
        // an exact pair would only lose detail: the system number is never used
        assertFalse(useSystemBattery(SystemBatteryMode.Always, exactPair, 100))
        assertFalse(systemBatteryApplies(exactPair))
        assertTrue(systemBatteryApplies(coarsePair))
        assertTrue(systemBatteryTwsApplies(coarsePair))
        assertFalse(systemBatteryTwsApplies(exactPair))
        assertFalse(systemBatteryTwsApplies(single))
    }
}
