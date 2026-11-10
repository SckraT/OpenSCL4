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
        val availability = settingsPresentation(oneEarbud).availability.getValue("ldac")

        assertFalse(availability.isAvailable())
        assertEquals(AvailabilityReason.REQUIRES_BOTH_EARBUDS, availability.blockedReason())
        assertEquals(Setting.ToggleSetting(true), oneEarbud.getValue("ldac"))
    }

    @Test
    fun theEqualizerIsUnavailableWithOneEarbud() {
        val availability = settingsPresentation(oneEarbud).availability.getValue(EQUALIZER_AVAILABILITY_ID)

        assertTrue(availability is ClientSettingAvailability.Unavailable)
        assertEquals(AvailabilityReason.REQUIRES_BOTH_EARBUDS, availability.blockedReason())
        // the device stops reporting the equalizer settings entirely with one earbud
        val withoutEqualizerSettings = mapOf("twsStatus" to twsStatus("Disconnected"))
        assertEquals(
            availability,
            settingsPresentation(withoutEqualizerSettings).availability.getValue(EQUALIZER_AVAILABILITY_ID),
        )
    }

    @Test
    fun everySettingIsAvailableWithBothEarbuds() {
        val settings = bothEarbuds - EQUALIZER_AVAILABILITY_ID + ("wearingDetection" to Setting.ToggleSetting(true))
        val availability = settingsPresentation(settings).availability
        assertEquals(settings.keys + EQUALIZER_AVAILABILITY_ID, availability.keys)
        for ((id, status) in availability) {
            assertTrue(id, status.isAvailable())
        }
    }

    @Test
    fun theAvailabilityMapCoversEverySetting() {
        val availability = settingsPresentation(oneEarbud).availability

        assertEquals(oneEarbud.keys, availability.keys)
        assertTrue(availability.getValue("ldac") is ClientSettingAvailability.ReadOnly)
        assertTrue(availability.getValue("volumeAdjustments") is ClientSettingAvailability.Unavailable)
    }

    @Test
    fun withoutAProvidedSnapshotNoSettingLooksAvailable() {
        val availability = settingAvailability(emptyMap(), EQUALIZER_AVAILABILITY_ID)

        assertTrue(availability is ClientSettingAvailability.Unavailable)
        assertEquals(AvailabilityReason.NO_DEVICE_STATE, availability.blockedReason())
    }

    @Test
    fun providedAvailabilityIsUsedInsteadOfTheDefault() {
        val availability = settingsPresentation(bothEarbuds).availability

        assertTrue(settingAvailability(availability, EQUALIZER_AVAILABILITY_ID).isAvailable())
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
