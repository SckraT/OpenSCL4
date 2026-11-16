package com.oppzippy.openscq30.features.battery

import com.oppzippy.openscq30.lib.wrapper.Setting
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class BatteryShapeTest {
    private fun information(raw: String) = Setting.InformationSetting(raw, raw)

    private fun settings(vararg pairs: Pair<String, String>): Map<String, Setting> =
        pairs.associate { (id, raw) -> id to information(raw) }

    @Test
    fun the_scale_comes_from_the_raw_fraction() {
        assertEquals(5, batteryShape(settings("batteryLevelLeft" to "5/5")).scale)
        assertEquals(5, batteryShape(settings("batteryLevelLeft" to "4/5")).scale)
        assertEquals(10, batteryShape(settings("batteryLevel" to "8/10")).scale)
        assertEquals(100, batteryShape(settings("batteryLevelLeft" to "75/100")).scale)
    }

    @Test
    fun an_unknown_format_has_no_scale() {
        assertNull(batteryShape(emptyMap()).scale)
        assertNull(batteryShape(settings("batteryLevelLeft" to "unknown")).scale)
        assertNull(batteryShape(settings("batteryLevelLeft" to "1/0")).scale)
    }

    @Test
    fun a_pair_is_told_by_its_per_earbud_levels() {
        assertTrue(batteryShape(settings("batteryLevelLeft" to "5/5", "batteryLevelRight" to "4/5")).isTws)
        assertTrue(batteryShape(settings("batteryLevelRight" to "4/5")).isTws)
        assertFalse(batteryShape(settings("batteryLevel" to "8/10")).isTws)
    }

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
