package com.oppzippy.openscq30.widget

/**
 * The selection stored under [SettingWidget.settingIdsKey] after the switch for [settingId] moved.
 *
 * A device the user never configured has no stored selection at all, which reads like an empty one; turning a setting
 * on then starts the selection. The stored set is never changed in place.
 */
internal fun toggledSettingIds(selectedSettingIds: Set<String>?, settingId: String, isSelected: Boolean): Set<String> =
    if (isSelected) {
        selectedSettingIds.orEmpty() + settingId
    } else {
        selectedSettingIds.orEmpty() - settingId
    }

/**
 * Adds or removes a quick preset of the device panel, keeping the order the user picked and never more than [maxSlots].
 * A preset that is already selected cannot be added twice.
 */
internal fun togglePresetSelection(selected: List<String>, name: String, maxSlots: Int): List<String> = when {
    name in selected -> selected - name
    selected.size >= maxSlots -> selected
    else -> selected + name
}
