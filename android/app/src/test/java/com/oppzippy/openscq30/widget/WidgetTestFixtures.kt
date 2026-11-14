package com.oppzippy.openscq30.widget

import com.oppzippy.openscq30.lib.bindings.SettingCategory
import com.oppzippy.openscq30.lib.bindings.SettingIdSettingPair
import com.oppzippy.openscq30.lib.wrapper.Select
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.ui.sound.DeviceSettingsSnapshot
import com.oppzippy.openscq30.ui.sound.toClientCategories

/** The categories a fixture device reports; every setting in them is a toggle unless the test gives another value. */
internal fun settingCategories(vararg entries: Pair<String, List<String>>): List<SettingCategory> =
    entries.map { (category, settingIds) ->
        SettingCategory(category, settingIds.map { SettingIdSettingPair(it, Setting.ToggleSetting(false)) })
    }

/** A select setting with [value] chosen from [options]. */
internal fun selectSetting(vararg options: String, value: String): Setting =
    Setting.SelectSetting(Select(options.toList(), options.toList()), value)

/** The snapshot the fixture device reports: the settings behind [values] and the [categories] it lists them in. */
internal fun settingsSnapshot(
    categories: List<SettingCategory>,
    values: Map<String, Setting>,
): DeviceSettingsSnapshot = DeviceSettingsSnapshot(values, categories.toClientCategories())
