package com.oppzippy.openscq30.widget

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The configuration screen stores which settings the widget shows, per device model. The switch handler only knows the
 * current selection and which row moved; this is that decision away from the activity.
 */
class WidgetSelectionTest {
    @Test
    fun enablingASettingWithNothingStoredStartsASelection() {
        assertEquals(setOf("ambientSoundMode"), toggledSettingIds(emptySet(), "ambientSoundMode", true))
    }

    @Test
    fun enablingASettingOnANeverConfiguredDeviceSelectsIt() {
        // a device the user never configured has no stored selection at all
        assertEquals(setOf("ldac"), toggledSettingIds(null, "ldac", true))
    }

    @Test
    fun enablingASettingKeepsTheOnesAlreadySelected() {
        val stored = setOf("ambientSoundMode", "wearingDetection")

        assertEquals(stored + "ldac", toggledSettingIds(stored, "ldac", true))
    }

    @Test
    fun disablingASettingRemovesOnlyThatOne() {
        val stored = setOf("ambientSoundMode", "wearingDetection")

        assertEquals(setOf("ambientSoundMode"), toggledSettingIds(stored, "wearingDetection", false))
    }

    @Test
    fun disablingTheLastSettingLeavesAnEmptySelection() {
        assertEquals(emptySet<String>(), toggledSettingIds(setOf("ldac"), "ldac", false))
    }

    @Test
    fun disablingWithoutAStoredSelectionStaysEmpty() {
        assertEquals(emptySet<String>(), toggledSettingIds(null, "ldac", false))
    }

    @Test
    fun enablingAnAlreadySelectedSettingKeepsItSelected() {
        assertEquals(setOf("ldac"), toggledSettingIds(setOf("ldac"), "ldac", true))
    }

    @Test
    fun theStoredSelectionIsNotChanged() {
        val stored = setOf("ambientSoundMode")

        toggledSettingIds(stored, "ldac", true)

        assertEquals(setOf("ambientSoundMode"), stored)
    }
}
