package com.oppzippy.openscq30.ui.sound

import com.oppzippy.openscq30.lib.wrapper.Setting
import org.junit.Assert.assertEquals
import org.junit.Test

class SettingGroupsTest {
    private fun groupOf(id: String, category: String = "miscellaneous") = settingGroup(id, category)

    @Test
    fun theLibertyFourProMiscellaneousSettingsAllFindAGroup() {
        assertEquals(
            mapOf(
                "ldac" to SettingGroup.SoundEffects,
                "autoPowerOff" to SettingGroup.Power,
                "lowBatteryPrompt" to SettingGroup.Power,
                "spatialAudio" to SettingGroup.SoundEffects,
                "spatialAudioMode" to SettingGroup.SoundEffects,
                "spatialAudioMusicMode" to SettingGroup.SoundEffects,
                "easyChat" to SettingGroup.Comfort,
                "easyChatWaitTime" to SettingGroup.Comfort,
                "soundLeakCompensation" to SettingGroup.Comfort,
                "wearingDetection" to SettingGroup.Comfort,
            ),
            listOf(
                "ldac", "autoPowerOff", "lowBatteryPrompt", "spatialAudio", "spatialAudioMode", "spatialAudioMusicMode",
                "easyChat", "easyChatWaitTime", "soundLeakCompensation", "wearingDetection",
            ).associateWith { groupOf(it) },
        )
    }

    @Test
    fun whatASettingIsForComesBeforeItsLibraryCategory() {
        // the library also lists the Liberty 4 Pro's spatial audio under its case
        assertEquals(SettingGroup.SoundEffects, groupOf("spatialAudio", "case"))
        assertEquals(SettingGroup.Case, groupOf("caseBatteryLevel", "deviceInformation"))
        assertEquals(SettingGroup.Controls, groupOf("touchTone"))
        assertEquals(SettingGroup.Controls, groupOf("leftSinglePress", "buttonConfiguration"))
    }

    @Test
    fun settingsWithoutAGroupOfTheirOwnGoByTheirCategory() {
        assertEquals(SettingGroup.Comfort, groupOf("limitHighVolumeDbLimit", "limitHighVolume"))
        assertEquals(SettingGroup.About, groupOf("serialNumber", "deviceInformation"))
        assertEquals(SettingGroup.Equalizer, groupOf("importCustomEqualizerProfiles", "equalizerImportExport"))
        assertEquals(SettingGroup.Lights, groupOf("lightsColor", "lights"))
        assertEquals(SettingGroup.Other, groupOf("somethingNew"))
        assertEquals(SettingGroup.Other, settingGroup("somethingNew", null))
    }

    @Test
    fun theSleepEarbudsGetSleepAndPowerGroups() {
        assertEquals(SettingGroup.Sleep, groupOf("autoStopTimer"))
        assertEquals(SettingGroup.Sleep, groupOf("listeningMode"))
        assertEquals(SettingGroup.Power, groupOf("noiseCancelingPrompt"))
        assertEquals(SettingGroup.SoundModes, groupOf("noiseCanceling"))
    }

    @Test
    fun everySettingIsShownOnceInTheDeviceOrder() {
        val toggle = Setting.ToggleSetting(false)
        val settings = listOf("wearingDetection" to toggle, "ldac" to toggle, "spatialAudio" to toggle)
        val byCategory = mapOf(
            "case" to listOf("spatialAudio" to toggle),
            "miscellaneous" to listOf("wearingDetection" to toggle, "ldac" to toggle, "spatialAudio" to toggle),
        )
        val grouped = groupSettings(settings, byCategory)
        assertEquals(listOf("ldac", "spatialAudio"), grouped[SettingGroup.SoundEffects]?.map { it.first })
        assertEquals(listOf("wearingDetection"), grouped[SettingGroup.Comfort]?.map { it.first })
        assertEquals(null, grouped[SettingGroup.Case])
    }
}
