package com.oppzippy.openscq30.ui.sound

import androidx.annotation.DrawableRes
import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.lib.bindings.ClientSettingCategory
import com.oppzippy.openscq30.lib.bindings.clientDisplayedSettings
import com.oppzippy.openscq30.lib.bindings.clientGroupSettings
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

/**
 * The device's settings by group, each in the device's order, drawn as the library says. Groups without settings are
 * left out.
 */
internal fun groupSettings(byCategory: Map<String, List<Pair<String, Setting>>>): Map<SettingGroup, List<Pair<String, Setting>>> {
    val categories = byCategory.map { (category, settings) ->
        ClientSettingCategory(category, settings.map { it.first })
    }
    val displayed = clientDisplayedSettings(byCategory.values.flatten().toMap())
    return buildMap {
        for (group in clientGroupSettings(categories)) {
            val shown = group.ids.mapNotNull { id -> displayed[id]?.let { id to it } }
            if (shown.isNotEmpty()) put(SettingGroup.valueOf(group.group), shown)
        }
    }
}
