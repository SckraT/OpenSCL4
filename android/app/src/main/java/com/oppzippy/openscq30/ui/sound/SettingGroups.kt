package com.oppzippy.openscq30.ui.sound

import androidx.annotation.DrawableRes
import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.lib.bindings.translateCategoryId
import com.oppzippy.openscq30.lib.wrapper.Setting

/**
 * The device's settings by what they are for, in the order All device settings lists them. Every setting belongs to
 * exactly one group, whose page shows it. Groups that match a library category take its name.
 */
enum class SettingGroup(val id: String, @DrawableRes val icon: Int, val libraryCategory: String? = null) {
    SoundModes("soundModes", R.drawable.sound_noise),
    Equalizer("equalizer", R.drawable.equalizer_24px),
    SoundEffects("soundEffects", R.drawable.speaker_24px),
    Controls("controls", R.drawable.sound_hand),
    Comfort("comfort", R.drawable.sound_heart),
    Power("power", R.drawable.power_settings_new_24px),
    Connections("connections", R.drawable.devices_other_24px),
    Case("case", R.drawable.earbud_case_24px),
    Sleep("sleep", R.drawable.bedtime_24px),
    Lights("lights", R.drawable.light_mode_24px, libraryCategory = "lights"),
    Alarms("alarms", R.drawable.notifications_24px, libraryCategory = "alarms"),
    About("about", R.drawable.info_24px, libraryCategory = "deviceInformation"),
    Other("other", R.drawable.settings_24px, libraryCategory = "miscellaneous"),
}

@Composable
fun SettingGroup.title(): String = when (this) {
    SettingGroup.SoundModes -> stringResource(R.string.sound_overview_sound_mode)
    SettingGroup.Equalizer -> stringResource(R.string.equalizer)
    SettingGroup.SoundEffects -> stringResource(R.string.sound_overview_effects)
    SettingGroup.Controls -> stringResource(R.string.sound_overview_controls)
    SettingGroup.Comfort -> stringResource(R.string.sound_overview_comfort)
    SettingGroup.Power -> stringResource(R.string.sound_overview_power)
    SettingGroup.Connections -> stringResource(R.string.sound_overview_connections)
    SettingGroup.Case -> stringResource(R.string.sound_overview_case)
    SettingGroup.Sleep -> stringResource(R.string.sound_overview_sleep)
    SettingGroup.Lights, SettingGroup.Alarms, SettingGroup.About, SettingGroup.Other ->
        translateCategoryId(requireNotNull(libraryCategory))
}

private val groupsById: Map<String, SettingGroup> = buildMap {
    fun put(group: SettingGroup, vararg ids: String) = ids.forEach { put(it, group) }
    SoundCapabilities.modes.forEach { put(it, SettingGroup.SoundModes) }
    // the sleep earbuds turn noise canceling on and off with a toggle of its own
    put(SettingGroup.SoundModes, "noiseCanceling")
    // how the sound gets to the earbuds and what is done to it
    put(
        SettingGroup.SoundEffects,
        "ldac", "gamingMode", "spatialAudio", "spatialAudioMode", "spatialAudioMusicMode", "immersiveExperience",
        "dolbyAudio", "surroundSound",
    )
    // feedback for, and a lock on, the touch controls
    put(SettingGroup.Controls, "touchTone", "touchLock")
    // wearing the earbuds, talking with them in and how loud they get
    put(
        SettingGroup.Comfort,
        "wearingDetection", "autoPlayPause", "wearingTone", "easyChat", "easyChatWaitTime", "sideTone",
        "soundLeakCompensation", "volumeBalance", "volume",
    )
    put(
        SettingGroup.Power,
        "autoPowerOff", "autoPowerOffPrompt", "powerOff", "lowBatteryPrompt", "voicePrompt", "listeningModePrompt",
        "noiseCancelingPrompt",
    )
    put(SettingGroup.Connections, "dualConnections", "dualConnectionsDevices")
    SoundCapabilities.case.forEach { put(it, SettingGroup.Case) }
    put(
        SettingGroup.Sleep,
        "autoStopTimer", "autoStopTimerDuration", "autoSwitchOnceAsleep", "listeningMode", "defaultListeningMode",
        "incomingCallsDuringBluetoothMode",
    )
}

/**
 * The group of a setting: by what the setting is first, then by its library category. Going by the setting first
 * also keeps the Liberty 4 Pro's spatial audio, which the library lists under its case too, off the case page.
 */
internal fun settingGroup(settingId: String, categoryId: String?): SettingGroup {
    groupsById[settingId]?.let { return it }
    if (SoundCapabilities.isControl(settingId)) return SettingGroup.Controls
    return when (categoryId) {
        "soundModes" -> SettingGroup.SoundModes
        "equalizer", "equalizerImportExport" -> SettingGroup.Equalizer
        "buttonConfiguration" -> SettingGroup.Controls
        "limitHighVolume" -> SettingGroup.Comfort
        "dualConnections" -> SettingGroup.Connections
        "case" -> SettingGroup.Case
        "lights" -> SettingGroup.Lights
        "alarms" -> SettingGroup.Alarms
        "deviceInformation" -> SettingGroup.About
        else -> SettingGroup.Other
    }
}

/** The device's settings by group, each in the device's order. Groups without settings are left out. */
internal fun groupSettings(
    settings: List<Pair<String, Setting>>,
    byCategory: Map<String, List<Pair<String, Setting>>>,
): Map<SettingGroup, List<Pair<String, Setting>>> {
    val categoryOf = byCategory.flatMap { (category, list) -> list.map { it.first to category } }.toMap()
    return settings.groupBy { (id, _) -> settingGroup(id, categoryOf[id]) }
}
