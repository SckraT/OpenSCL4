package com.oppzippy.openscq30.ui.sound

import com.oppzippy.openscq30.lib.wrapper.QuickPreset
import com.oppzippy.openscq30.lib.wrapper.QuickPresetField
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.lib.wrapper.toValue
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SoundPresetsTest {
    @Test
    fun emptyPresetIsNotApplied() {
        assertFalse(isSoundPresetApplied(QuickPreset("Empty", emptyList()), emptyMap()))
    }

    @Test
    fun presetWithOnlyDisabledFieldsIsNotApplied() {
        val preset = QuickPreset("Disabled", listOf(QuickPresetField("wearingDetection", true.toValue(), false)))
        assertFalse(isSoundPresetApplied(preset, mapOf("wearingDetection" to Setting.ToggleSetting(true))))
    }

    @Test
    fun enabledFieldMustBeAvailable() {
        val preset = QuickPreset("Missing", listOf(QuickPresetField("wearingDetection", true.toValue(), true)))
        assertFalse(isSoundPresetApplied(preset, emptyMap()))
    }

    @Test
    fun enabledFieldMustMatchCurrentDeviceValue() {
        val preset = QuickPreset("Different", listOf(QuickPresetField("wearingDetection", true.toValue(), true)))
        assertFalse(isSoundPresetApplied(preset, mapOf("wearingDetection" to Setting.ToggleSetting(false))))
    }

    @Test
    fun matchingEnabledFieldIsApplied() {
        val preset = QuickPreset("Matching", listOf(QuickPresetField("wearingDetection", true.toValue(), true)))
        assertTrue(isSoundPresetApplied(preset, mapOf("wearingDetection" to Setting.ToggleSetting(true))))
    }

    @Test
    fun disabledMismatchDoesNotInvalidateAppliedPreset() {
        val preset = QuickPreset(
            "Partial",
            listOf(
                QuickPresetField("wearingDetection", true.toValue(), true),
                QuickPresetField("limitHighVolume", false.toValue(), false),
            ),
        )
        val settings = mapOf("wearingDetection" to Setting.ToggleSetting(true), "limitHighVolume" to Setting.ToggleSetting(true))
        assertTrue(isSoundPresetApplied(preset, settings))
    }

    @Test
    fun allEnabledFieldsMustMatch() {
        val preset = QuickPreset(
            "Multiple",
            listOf(
                QuickPresetField("wearingDetection", true.toValue(), true),
                QuickPresetField("limitHighVolume", true.toValue(), true),
            ),
        )
        val settings = mapOf("wearingDetection" to Setting.ToggleSetting(true), "limitHighVolume" to Setting.ToggleSetting(false))
        assertFalse(isSoundPresetApplied(preset, settings))
    }

    @Test
    fun duplicateCategoryFieldsProduceOneEditorAndSummaryEntry() {
        val spatial = QuickPresetField("spatialAudio", true.toValue(), true)
        val volume = QuickPresetField("limitHighVolume", false.toValue(), true)
        val preset = QuickPreset("Shared categories", listOf(spatial, volume, spatial))

        val fields = soundPresetFields(preset)

        assertEquals(listOf(spatial, volume), fields)
        assertEquals(fields.size, fields.map { it.settingId }.distinct().size)
        assertEquals(2, fields.count { it.isEnabled })
    }

    @Test
    fun lastEnabledDuplicateMatchesTheFinalNativeActivationValue() {
        val first = QuickPresetField("spatialAudio", true.toValue(), true)
        val last = QuickPresetField("spatialAudio", false.toValue(), true)
        val preset = QuickPreset("Ordered values", listOf(first, last))

        assertEquals(listOf(last), soundPresetFields(preset))
        assertTrue(isSoundPresetApplied(preset, mapOf("spatialAudio" to Setting.ToggleSetting(false))))
        assertFalse(isSoundPresetApplied(preset, mapOf("spatialAudio" to Setting.ToggleSetting(true))))
    }

    @Test
    fun disabledDuplicateDoesNotOverrideAnEnabledOccurrence() {
        val enabled = QuickPresetField("spatialAudio", true.toValue(), true)
        val disabled = QuickPresetField("spatialAudio", false.toValue(), false)
        val preset = QuickPreset("Mixed flags", listOf(enabled, disabled))

        assertEquals(listOf(enabled), soundPresetFields(preset))
        assertTrue(isSoundPresetApplied(preset, mapOf("spatialAudio" to Setting.ToggleSetting(true))))
    }

    @Test
    fun duplicatesWithNoEnabledOccurrenceRemainDisabled() {
        val first = QuickPresetField("spatialAudio", true.toValue(), false)
        val last = QuickPresetField("spatialAudio", false.toValue(), false)
        val preset = QuickPreset("Disabled duplicates", listOf(first, last))

        assertEquals(listOf(last), soundPresetFields(preset))
        assertFalse(isSoundPresetApplied(preset, mapOf("spatialAudio" to Setting.ToggleSetting(false))))
    }

    // Two presets that enable different settings can both match the device at the same time
    private val anc = QuickPreset("ANC", listOf(QuickPresetField("wearingDetection", true.toValue(), true)))
    private val equalizer = QuickPreset("Equalizer", listOf(QuickPresetField("limitHighVolume", false.toValue(), true)))
    private val deviceMatchingBoth = mapOf(
        "wearingDetection" to Setting.ToggleSetting(true),
        "limitHighVolume" to Setting.ToggleSetting(false),
    )

    @Test
    fun onlyTheLastAppliedPresetIsAppliedWhenSeveralMatch() {
        assertEquals(SoundPresetStatus.Applied, soundPresetStatus(anc, deviceMatchingBoth, "ANC"))
        assertEquals(SoundPresetStatus.MatchesCurrent, soundPresetStatus(equalizer, deviceMatchingBoth, "ANC"))
    }

    @Test
    fun matchingPresetIsNotAppliedWhenNothingWasApplied() {
        assertEquals(SoundPresetStatus.MatchesCurrent, soundPresetStatus(anc, deviceMatchingBoth, null))
        assertEquals(SoundPresetStatus.MatchesCurrent, soundPresetStatus(equalizer, deviceMatchingBoth, null))
    }

    @Test
    fun lastAppliedPresetStopsBeingAppliedWhenOneOfItsSettingsChanges() {
        val changed = deviceMatchingBoth + ("wearingDetection" to Setting.ToggleSetting(false))
        assertEquals(SoundPresetStatus.NotApplied, soundPresetStatus(anc, changed, "ANC"))
        assertEquals(SoundPresetStatus.MatchesCurrent, soundPresetStatus(equalizer, changed, "ANC"))
    }

    @Test
    fun changingASettingThePresetDoesNotUseKeepsItApplied() {
        val changed = deviceMatchingBoth + ("limitHighVolume" to Setting.ToggleSetting(true))
        assertEquals(SoundPresetStatus.Applied, soundPresetStatus(anc, changed, "ANC"))
    }

    @Test
    fun presetWithoutEnabledFieldsIsNeverApplied() {
        val empty = QuickPreset("Empty", listOf(QuickPresetField("wearingDetection", true.toValue(), false)))
        assertEquals(SoundPresetStatus.NotApplied, soundPresetStatus(empty, deviceMatchingBoth, "Empty"))
    }

    @Test
    fun presetsWithAnEnabledEqualizerFieldNeedBothEarbuds() {
        val equalizerPreset = QuickPreset(
            "Bass",
            listOf(QuickPresetField("presetEqualizerProfile", "BassBooster".toValue(), true)),
        )
        val disabledEqualizer = QuickPreset(
            "Off",
            listOf(QuickPresetField("volumeAdjustments", "x".toValue(), false)),
        )
        assertTrue(presetsUseBothEarbudSettings(listOf(anc, equalizerPreset)))
        assertFalse(presetsUseBothEarbudSettings(listOf(anc, disabledEqualizer)))
        assertFalse(presetsUseBothEarbudSettings(emptyList()))
    }
}
