package com.oppzippy.openscq30.ui.sound

import android.content.Context
import androidx.annotation.DrawableRes
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import com.oppzippy.openscq30.R
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
fun SettingGroup.title(): String = title(LocalContext.current)

/** The group's name outside a composition, such as on the home screen widget. */
fun SettingGroup.title(context: Context): String = when (this) {
    SettingGroup.SoundModes -> context.getString(R.string.sound_overview_sound_mode)

    SettingGroup.Equalizer -> context.getString(R.string.equalizer)

    SettingGroup.SoundEffects -> context.getString(R.string.sound_overview_effects)

    SettingGroup.Controls -> context.getString(R.string.sound_overview_controls)

    SettingGroup.Comfort -> context.getString(R.string.sound_overview_comfort)

    SettingGroup.Power -> context.getString(R.string.sound_overview_power)

    SettingGroup.Connections -> context.getString(R.string.sound_overview_connections)

    SettingGroup.Case -> context.getString(R.string.sound_overview_case)

    SettingGroup.Sleep -> context.getString(R.string.sound_overview_sleep)

    SettingGroup.Lights, SettingGroup.Alarms, SettingGroup.About, SettingGroup.Other ->
        translateCategoryId(requireNotNull(libraryCategory))
}

/** The shared group with the library's name, or [SettingGroup.Other] for a group this client does not know yet. */
internal fun settingGroup(name: String): SettingGroup = SettingGroup.entries.firstOrNull { it.name == name }
    ?: SettingGroup.Other

/**
 * The device's settings by group, each in the device's order, drawn as the library says. Groups without settings are
 * left out.
 */
internal fun groupSettings(
    byCategory: Map<String, List<Pair<String, Setting>>>,
): Map<SettingGroup, List<Pair<String, Setting>>> {
    val snapshot = DeviceSettingsSnapshot.of(byCategory)
    val displayed = clientDisplayedSettings(snapshot.settings)
    return buildMap {
        for (group in clientGroupSettings(snapshot.categories)) {
            val shown = group.ids.mapNotNull { id -> displayed[id]?.let { id to it } }
            if (shown.isNotEmpty()) put(settingGroup(group.group), shown)
        }
    }
}
