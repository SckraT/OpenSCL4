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
        val preset = QuickPreset("Disabled", listOf(QuickPresetField("wear", true.toValue(), false)))
        assertFalse(isSoundPresetApplied(preset, mapOf("wear" to Setting.ToggleSetting(true))))
    }

    @Test
    fun enabledFieldMustBeAvailable() {
        val preset = QuickPreset("Missing", listOf(QuickPresetField("wear", true.toValue(), true)))
        assertFalse(isSoundPresetApplied(preset, emptyMap()))
    }

    @Test
    fun enabledFieldMustMatchCurrentDeviceValue() {
        val preset = QuickPreset("Different", listOf(QuickPresetField("wear", true.toValue(), true)))
        assertFalse(isSoundPresetApplied(preset, mapOf("wear" to Setting.ToggleSetting(false))))
    }

    @Test
    fun matchingEnabledFieldIsApplied() {
        val preset = QuickPreset("Matching", listOf(QuickPresetField("wear", true.toValue(), true)))
        assertTrue(isSoundPresetApplied(preset, mapOf("wear" to Setting.ToggleSetting(true))))
    }

    @Test
    fun disabledMismatchDoesNotInvalidateAppliedPreset() {
        val preset = QuickPreset(
            "Partial",
            listOf(
                QuickPresetField("wear", true.toValue(), true),
                QuickPresetField("safeVolume", false.toValue(), false),
            ),
        )
        val settings = mapOf("wear" to Setting.ToggleSetting(true), "safeVolume" to Setting.ToggleSetting(true))
        assertTrue(isSoundPresetApplied(preset, settings))
    }

    @Test
    fun allEnabledFieldsMustMatch() {
        val preset = QuickPreset(
            "Multiple",
            listOf(
                QuickPresetField("wear", true.toValue(), true),
                QuickPresetField("safeVolume", true.toValue(), true),
            ),
        )
        val settings = mapOf("wear" to Setting.ToggleSetting(true), "safeVolume" to Setting.ToggleSetting(false))
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
    private val anc = QuickPreset("ANC", listOf(QuickPresetField("wear", true.toValue(), true)))
    private val equalizer = QuickPreset("Equalizer", listOf(QuickPresetField("safeVolume", false.toValue(), true)))
    private val deviceMatchingBoth = mapOf(
        "wear" to Setting.ToggleSetting(true),
        "safeVolume" to Setting.ToggleSetting(false),
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
        val changed = deviceMatchingBoth + ("wear" to Setting.ToggleSetting(false))
        assertEquals(SoundPresetStatus.NotApplied, soundPresetStatus(anc, changed, "ANC"))
        assertEquals(SoundPresetStatus.MatchesCurrent, soundPresetStatus(equalizer, changed, "ANC"))
    }

    @Test
    fun changingASettingThePresetDoesNotUseKeepsItApplied() {
        val changed = deviceMatchingBoth + ("safeVolume" to Setting.ToggleSetting(true))
        assertEquals(SoundPresetStatus.Applied, soundPresetStatus(anc, changed, "ANC"))
    }

    @Test
    fun presetWithoutEnabledFieldsIsNeverApplied() {
        val empty = QuickPreset("Empty", listOf(QuickPresetField("wear", true.toValue(), false)))
        assertEquals(SoundPresetStatus.NotApplied, soundPresetStatus(empty, deviceMatchingBoth, "Empty"))
    }
}
