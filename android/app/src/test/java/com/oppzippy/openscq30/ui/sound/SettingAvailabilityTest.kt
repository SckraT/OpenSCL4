package com.oppzippy.openscq30.ui.sound

import com.oppzippy.openscq30.lib.bindings.AvailabilityReason
import com.oppzippy.openscq30.lib.bindings.ClientSettingAvailability
import com.oppzippy.openscq30.lib.wrapper.QuickPreset
import com.oppzippy.openscq30.lib.wrapper.QuickPresetField
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.lib.wrapper.blockedReason
import com.oppzippy.openscq30.lib.wrapper.isAvailable
import com.oppzippy.openscq30.lib.wrapper.toValue
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SettingAvailabilityTest {
    private fun twsStatus(value: String): Setting = Setting.InformationSetting(value, value)

    private val oneEarbud = mapOf(
        "twsStatus" to twsStatus("Disconnected"),
        "ldac" to Setting.ToggleSetting(true),
        "volumeAdjustments" to Setting.InformationSetting("", ""),
    )

    private val bothEarbuds = oneEarbud + ("twsStatus" to twsStatus("Connected"))

    @Test
    fun ldacCanBeReadButNotChangedWithOneEarbud() {
        val availability = settingAvailability(oneEarbud, "ldac")

        assertFalse(availability.isAvailable())
        assertEquals(AvailabilityReason.REQUIRES_BOTH_EARBUDS, availability.blockedReason())
        assertEquals(Setting.ToggleSetting(true), oneEarbud.getValue("ldac"))
    }

    @Test
    fun theEqualizerIsUnavailableWithOneEarbud() {
        val availability = settingAvailability(oneEarbud, "volumeAdjustments")

        assertTrue(availability is ClientSettingAvailability.Unavailable)
        assertEquals(AvailabilityReason.REQUIRES_BOTH_EARBUDS, availability.blockedReason())
        // the device stops reporting the equalizer settings entirely with one earbud
        val withoutEqualizerSettings = mapOf("twsStatus" to twsStatus("Disconnected"))
        assertTrue(
            settingAvailability(withoutEqualizerSettings, "volumeAdjustments") is ClientSettingAvailability.Unavailable,
        )
    }

    @Test
    fun everySettingIsAvailableWithBothEarbuds() {
        for (id in listOf("ldac", "volumeAdjustments", "wearingDetection")) {
            assertTrue(id, settingAvailability(bothEarbuds, id).isAvailable())
        }
    }

    @Test
    fun theAvailabilityMapCoversEverySetting() {
        val availability = settingAvailability(oneEarbud)

        assertEquals(oneEarbud.keys, availability.keys)
        assertTrue(availability.getValue("ldac") is ClientSettingAvailability.ReadOnly)
        assertTrue(availability.getValue("volumeAdjustments") is ClientSettingAvailability.Unavailable)
    }

    @Test
    fun presetsWithBlockedFieldsAreReported() {
        val preset = QuickPreset(
            "LDAC",
            listOf(QuickPresetField("ldac", true.toValue(), true)),
        )

        assertTrue(presetsUseBlockedSettings(listOf(preset), oneEarbud))
        assertFalse(presetsUseBlockedSettings(listOf(preset), bothEarbuds))
    }
}
