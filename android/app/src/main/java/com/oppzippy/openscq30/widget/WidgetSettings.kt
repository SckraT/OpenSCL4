package com.oppzippy.openscq30.widget

import com.oppzippy.openscq30.lib.bindings.AvailabilityReason
import com.oppzippy.openscq30.lib.bindings.ClientSettingAvailability
import com.oppzippy.openscq30.lib.bindings.ClientSettingControl
import com.oppzippy.openscq30.lib.bindings.clientDisplayedSettings
import com.oppzippy.openscq30.lib.bindings.clientGroupSettings
import com.oppzippy.openscq30.lib.bindings.clientIsOnlyOneEarbudConnected
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.ui.deviceselection.deviceBatteries
import com.oppzippy.openscq30.ui.deviceselection.withMissingEarbud
import com.oppzippy.openscq30.ui.sound.DeviceSettingsSnapshot
import com.oppzippy.openscq30.ui.sound.SettingGroup
import com.oppzippy.openscq30.ui.sound.settingAvailability
import com.oppzippy.openscq30.ui.sound.settingGroup
import com.oppzippy.openscq30.ui.sound.settingsPresentation
import kotlinx.serialization.Serializable

/**
 * A selected setting the widget draws: its control, its value when it cannot be changed, or the reason it has no value
 * to draw.
 *
 * The [group] is the shared [SettingGroup] the setting belongs to, so the widget can draw the same headings as the
 * client. Rows cached by an older version have no group and are still read.
 */
@Serializable
internal sealed class WidgetSetting {
    abstract val settingId: String
    abstract val group: SettingGroup?

    @Serializable
    data class Editable(
        override val settingId: String,
        val setting: Setting,
        override val group: SettingGroup? = null,
    ) : WidgetSetting()

    @Serializable
    data class ReadOnly(
        override val settingId: String,
        val setting: Setting,
        val reason: AvailabilityReason,
        override val group: SettingGroup? = null,
    ) : WidgetSetting()

    @Serializable
    data class Blocked(
        override val settingId: String,
        val reason: AvailabilityReason,
        override val group: SettingGroup? = null,
    ) : WidgetSetting()
}

/** The rows of a connected widget, grouped by what the settings are for and ordered as the client lists them. */
internal fun List<WidgetSetting>.groups(): List<Pair<SettingGroup, List<WidgetSetting>>> {
    val grouped = linkedMapOf<SettingGroup, MutableList<WidgetSetting>>()
    for (row in this) {
        grouped.getOrPut(row.group ?: SettingGroup.Other) { mutableListOf() }.add(row)
    }
    return grouped.map { (group, rows) -> group to rows.toList() }
}

/**
 * The selected settings the widget can draw, decided from the device's [snapshot] and the shared client rules.
 *
 * The same pipeline the device settings screens use decides what the widget shows:
 * [clientDisplayedSettings] removes settings with nothing to draw, the shared presentation decides editor dependencies
 * ([ClientSettingControl]) and setting availability, and [clientGroupSettings] gives the order of the groups and of the
 * settings inside them. Rows whose editor is [ClientSettingControl.HIDDEN] or [ClientSettingControl.DISABLED] are left
 * out: a dependent setting reappears when its parent setting is switched on, the same way it does in the client.
 */
internal fun widgetSettings(
    selectedSettingIds: Collection<String>,
    snapshot: DeviceSettingsSnapshot,
): List<WidgetSetting> {
    val displayed = clientDisplayedSettings(snapshot.settings)
    val presentation = settingsPresentation(displayed)
    val selected = selectedSettingIds.toSet()

    return clientGroupSettings(snapshot.categories).flatMap { group ->
        val sharedGroup = settingGroup(group.group)
        group.ids.filter { it in selected }.mapNotNull { settingId ->
            val setting = displayed[settingId] ?: return@mapNotNull null
            if (presentation.controls[settingId] != null) {
                return@mapNotNull null
            }
            when (val availability = settingAvailability(presentation.availability, settingId)) {
                is ClientSettingAvailability.Available -> WidgetSetting.Editable(settingId, setting, sharedGroup)

                is ClientSettingAvailability.ReadOnly ->
                    WidgetSetting.ReadOnly(settingId, setting, availability.reason, sharedGroup)

                is ClientSettingAvailability.Unavailable ->
                    WidgetSetting.Blocked(settingId, availability.reason, sharedGroup)
            }
        }
    }
}

/** What the widget's header says about the device's batteries. */
internal data class WidgetHeader(
    /** Battery values in the shared order, ready to join into one line. */
    val batteryValues: List<String>,
    /** Only one earbud is connected, so the header can say that the rest is unavailable. */
    val oneEarbudOnly: Boolean,
)

/** The header facts, read through the shared battery rules. */
internal fun widgetHeader(snapshot: Map<String, Setting>): WidgetHeader {
    val oneEarbudOnly = clientIsOnlyOneEarbudConnected(snapshot)
    val batteries = withMissingEarbud(deviceBatteries(snapshot), oneEarbudOnly)
    return WidgetHeader(batteries.map { it.value }, oneEarbudOnly)
}

/** One setting the configuration screen offers, and why it cannot be shown now. */
internal data class WidgetConfigurationSetting(
    val settingId: String,
    /** The widget can draw this setting now; false means selecting it would show nothing. */
    val canShow: Boolean,
    /** The reason shown beside the switch, or null when the setting is available and independent. */
    val note: WidgetConfigurationNote? = null,
)

/** Why a setting cannot be shown now, or extra context when it can only be read. */
internal sealed class WidgetConfigurationNote {
    data class Availability(val reason: AvailabilityReason) : WidgetConfigurationNote()

    data object DependsOnOtherSetting : WidgetConfigurationNote()
}

/** One shared group on the configuration screen. */
internal data class WidgetConfigurationGroup(val group: SettingGroup, val settings: List<WidgetConfigurationSetting>)

/**
 * The settings the configuration screen offers, grouped and ordered by the shared client rules.
 *
 * The list comes from the same displayed settings the widget draws, so a setting with nothing to draw is not offered:
 * the adaptive noise canceling level, for example, exists only while the device runs in its adaptive mode. A setting the
 * widget can draw but cannot change in the current device state is still listed, disabled and with its reason, so the
 * choice is not a mystery; a hidden dependent setting is left out, like everywhere else in the client.
 */
internal fun widgetConfigurationGroups(snapshot: DeviceSettingsSnapshot): List<WidgetConfigurationGroup> {
    val displayed = clientDisplayedSettings(snapshot.settings)
    val presentation = settingsPresentation(displayed)

    return clientGroupSettings(snapshot.categories).mapNotNull { group ->
        val settings = group.ids.mapNotNull { settingId ->
            displayed[settingId] ?: return@mapNotNull null
            val availability = settingAvailability(presentation.availability, settingId)
            when {
                presentation.controls[settingId] == ClientSettingControl.HIDDEN -> null

                presentation.controls[settingId] == ClientSettingControl.DISABLED ->
                    WidgetConfigurationSetting(
                        settingId,
                        canShow = false,
                        note = WidgetConfigurationNote.DependsOnOtherSetting,
                    )

                availability is ClientSettingAvailability.Unavailable ->
                    WidgetConfigurationSetting(
                        settingId,
                        canShow = false,
                        note = WidgetConfigurationNote.Availability(availability.reason),
                    )

                availability is ClientSettingAvailability.ReadOnly ->
                    WidgetConfigurationSetting(
                        settingId,
                        canShow = true,
                        note = WidgetConfigurationNote.Availability(availability.reason),
                    )

                else -> WidgetConfigurationSetting(settingId, canShow = true)
            }
        }
        if (settings.isEmpty()) {
            null
        } else {
            WidgetConfigurationGroup(settingGroup(group.group), settings)
        }
    }
}
