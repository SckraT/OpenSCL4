package com.oppzippy.openscq30.widget

import com.oppzippy.openscq30.lib.bindings.AvailabilityReason
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.ui.sound.SettingGroup
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class WidgetConfigurationTest {
    private val oneEarbud = mapOf("twsStatus" to Setting.InformationSetting("Disconnected", "Disconnected"))

    @Test
    fun theScreenListsTheSharedGroupsInTheirOrder() {
        val values = oneEarbud + mapOf(
            "wearingDetection" to Setting.ToggleSetting(true),
            "ambientSoundMode" to selectSetting("Normal", "Transparency", value = "Normal"),
        )
        val groups = widgetConfigurationGroups(
            settingsSnapshot(
                settingCategories(
                    "miscellaneous" to listOf("wearingDetection"),
                    "soundModes" to listOf("ambientSoundMode"),
                ),
                values,
            ),
        )

        assertEquals(listOf(SettingGroup.SoundModes, SettingGroup.Comfort), groups.map { it.group })
        assertEquals(listOf("ambientSoundMode"), groups.first().settings.map { it.settingId })
    }

    @Test
    fun aHiddenDependentSettingIsNotOffered() {
        // the music mode only exists while spatial audio runs in music mode
        val groups = widgetConfigurationGroups(
            settingsSnapshot(
                settingCategories(
                    "miscellaneous" to listOf("spatialAudio", "spatialAudioMode", "spatialAudioMusicMode"),
                ),
                mapOf(
                    "spatialAudio" to Setting.ToggleSetting(true),
                    "spatialAudioMode" to selectSetting("Music", "Podcast", value = "Podcast"),
                    "spatialAudioMusicMode" to Setting.ToggleSetting(true),
                ),
            ),
        )

        assertEquals(
            listOf("spatialAudio", "spatialAudioMode"),
            groups.single().settings.map { it.settingId },
        )
    }

    @Test
    fun aDependentSettingIsListedButCannotBeShownNow() {
        val groups = widgetConfigurationGroups(
            settingsSnapshot(
                settingCategories("miscellaneous" to listOf("easyChat", "easyChatWaitTime")),
                mapOf(
                    "easyChat" to Setting.ToggleSetting(false),
                    "easyChatWaitTime" to selectSetting("1", "2", value = "1"),
                ),
            ),
        )

        val waitTime = groups.single().settings.first { it.settingId == "easyChatWaitTime" }
        assertFalse(waitTime.canShow)
        assertEquals(WidgetConfigurationNote.DependsOnOtherSetting, waitTime.note)
    }

    @Test
    fun aSettingBehindAnOffParentIsListedAsDependingOnIt() {
        // the widget hides such a row while the parent is off, but the configuration screen says why instead of
        // leaving the setting out of the list
        val groups = widgetConfigurationGroups(
            settingsSnapshot(
                settingCategories("miscellaneous" to listOf("spatialAudio", "spatialAudioMusicMode")),
                mapOf(
                    "spatialAudio" to Setting.ToggleSetting(false),
                    "spatialAudioMusicMode" to Setting.ToggleSetting(true),
                ),
            ),
        )

        val musicMode = groups.single().settings.first { it.settingId == "spatialAudioMusicMode" }
        assertFalse(musicMode.canShow)
        assertEquals(WidgetConfigurationNote.DependsOnOtherSetting, musicMode.note)
    }

    @Test
    fun anUnavailableSettingIsListedWithItsReason() {
        val groups = widgetConfigurationGroups(
            settingsSnapshot(
                settingCategories("equalizer" to listOf("volumeAdjustments")),
                oneEarbud + ("volumeAdjustments" to Setting.ToggleSetting(true)),
            ),
        )

        val equalizer = groups.single().settings.single()
        assertFalse(equalizer.canShow)
        assertEquals(
            WidgetConfigurationNote.Availability(AvailabilityReason.REQUIRES_BOTH_EARBUDS),
            equalizer.note,
        )
    }

    @Test
    fun aReadOnlySettingCanStillBeShownAndSaysWhy() {
        val groups = widgetConfigurationGroups(
            settingsSnapshot(
                settingCategories("miscellaneous" to listOf("ldac")),
                oneEarbud + ("ldac" to Setting.ToggleSetting(true)),
            ),
        )

        val ldac = groups.single().settings.single()
        assertTrue(ldac.canShow)
        assertEquals(
            WidgetConfigurationNote.Availability(AvailabilityReason.REQUIRES_BOTH_EARBUDS),
            ldac.note,
        )
    }

    @Test
    fun anAvailableSettingNeedsNoNote() {
        val groups = widgetConfigurationGroups(
            settingsSnapshot(
                settingCategories("miscellaneous" to listOf("wearingDetection")),
                mapOf("wearingDetection" to Setting.ToggleSetting(true)),
            ),
        )

        val wearingDetection = groups.single().settings.single()
        assertTrue(wearingDetection.canShow)
        assertNull(wearingDetection.note)
    }

    @Test
    fun aSettingTheSharedRuleDoesNotDrawIsNotOffered() {
        // the adaptive noise canceling level reads as the device's own status and exists only while the device runs
        // in its adaptive mode; otherwise the shared displayed-settings rule leaves it out, so the widget can never
        // draw it, and the configuration must not offer it
        val groups = widgetConfigurationGroups(
            settingsSnapshot(
                settingCategories("soundModes" to listOf("noiseCancelingMode", "adaptiveNoiseCanceling")),
                mapOf(
                    "noiseCancelingMode" to selectSetting("Custom", "Adaptive", value = "Custom"),
                    "adaptiveNoiseCanceling" to Setting.InformationSetting("1/5", "1/5"),
                ),
            ),
        )

        assertEquals(listOf("noiseCancelingMode"), groups.single().settings.map { it.settingId })
    }

    @Test
    fun theAdaptiveLevelIsOfferedWhileTheDeviceIsAdaptive() {
        val groups = widgetConfigurationGroups(
            settingsSnapshot(
                settingCategories("soundModes" to listOf("noiseCancelingMode", "adaptiveNoiseCanceling")),
                mapOf(
                    "noiseCancelingMode" to selectSetting("Custom", "Adaptive", value = "Adaptive"),
                    "adaptiveNoiseCanceling" to Setting.InformationSetting("1/5", "1/5"),
                ),
            ),
        )

        assertEquals(
            listOf("noiseCancelingMode", "adaptiveNoiseCanceling"),
            groups.single().settings.map { it.settingId },
        )
    }
}
