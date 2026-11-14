package com.oppzippy.openscq30.ui.sound

import com.oppzippy.openscq30.lib.bindings.ClientSettingCategory
import com.oppzippy.openscq30.lib.bindings.SettingCategory
import com.oppzippy.openscq30.lib.wrapper.Setting

/**
 * What a device reports: its settings, each once, and its categories, shaped for the shared client rules.
 *
 * The widget and the device settings screens derive both here instead of reshaping the device output wherever they
 * consume it, so the shared grouping always sees the same settings and categories.
 */
internal class DeviceSettingsSnapshot(
    /** Every setting the device reports, each once, under the id it reports it as. */
    val settings: Map<String, Setting>,
    /** The device's categories as the shared client grouping wants them. */
    val categories: List<ClientSettingCategory>,
) {
    companion object {
        /** The snapshot of the categories a device reports. */
        fun of(categories: List<SettingCategory>): DeviceSettingsSnapshot = DeviceSettingsSnapshot(
            settings = categories
                .flatMap { it.settings }
                .distinctBy { it.settingId }
                .associate { it.settingId to it.setting },
            categories = categories.toClientCategories(),
        )

        /** The snapshot of the map of category to settings a device settings screen holds. */
        fun of(byCategory: Map<String, List<Pair<String, Setting>>>): DeviceSettingsSnapshot = DeviceSettingsSnapshot(
            settings = byCategory
                .values
                .flatten()
                .distinctBy { it.first }
                .associate { it.first to it.second },
            categories = byCategory.map { (category, settings) ->
                ClientSettingCategory(category, settings.map { it.first })
            },
        )
    }
}

/** The categories as the shared client grouping wants them: each category's setting ids in the device's order. */
internal fun List<SettingCategory>.toClientCategories(): List<ClientSettingCategory> = map { category ->
    ClientSettingCategory(category.categoryId, category.settings.map { it.settingId })
}
