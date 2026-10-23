package com.oppzippy.openscq30.ui.sound

import androidx.annotation.DrawableRes
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.features.preferences.DeviceUiPreferencesViewModel
import com.oppzippy.openscq30.features.preferences.SoundModesView
import com.oppzippy.openscq30.lib.bindings.ClientSettingAvailability
import com.oppzippy.openscq30.lib.bindings.clientBatteryProgress
import com.oppzippy.openscq30.lib.bindings.clientBothEarbudsSettingIds
import com.oppzippy.openscq30.lib.bindings.clientCategoryNeedsBothEarbuds
import com.oppzippy.openscq30.lib.bindings.clientIsOnlyOneEarbudConnected
import com.oppzippy.openscq30.lib.bindings.translateDeviceModel
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.lib.wrapper.Value
import com.oppzippy.openscq30.lib.wrapper.blockedReason
import com.oppzippy.openscq30.lib.wrapper.isAvailable
import com.oppzippy.openscq30.lib.wrapper.toValue
import com.oppzippy.openscq30.ui.deviceselection.ActiveDeviceState
import com.oppzippy.openscq30.ui.deviceselection.DeviceBattery
import com.oppzippy.openscq30.ui.deviceselection.deviceBatteries
import com.oppzippy.openscq30.ui.deviceselection.withMissingEarbud
import com.oppzippy.openscq30.ui.utils.slidesWithDeviceCard

/** Values are read from the active device. Unsupported batteries/settings are omitted. */
@Composable
fun SoundOverview(
    deviceModel: String,
    settings: List<Pair<String, Setting>>,
    hasSound: Boolean,
    hasControls: Boolean,
    hasComfort: Boolean,
    hasConnections: Boolean,
    hasCase: Boolean,
    hasEffects: Boolean,
    hasPower: Boolean,
    hasEqualizerCategory: Boolean = false,
    macAddress: String? = null,
    quickPresets: List<String> = emptyList(),
    activeQuickPreset: String? = null,
    onActivateQuickPreset: (String) -> Unit = {},
    onSound: () -> Unit,
    onEqualizer: () -> Unit,
    onControls: () -> Unit,
    onComfort: () -> Unit,
    onConnections: () -> Unit,
    onSpatial: () -> Unit,
    onPower: () -> Unit,
    onCase: () -> Unit,
    onAllSettings: () -> Unit,
    onEditLayout: () -> Unit = {},
    setSettings: (List<Pair<String, Value>>) -> Unit,
    preferences: DeviceUiPreferencesViewModel = hiltViewModel(),
) {
    val values = remember(settings) { settings.toMap() }
    val availability = remember(values) { settingAvailability(values) }
    val equalizerAvailability = remember(values) { settingAvailability(values, "volumeAdjustments") }
    val equalizerAvailable = equalizerAvailability.isAvailable()
    val oneEarbudOnly = isOnlyOneEarbudConnected(values)
    val showOneEarbudBadge by preferences.oneEarbudBadgeFlow.collectAsState()
    val soundModesView by preferences.soundModesViewFlow.collectAsState()
    val storedLayout by remember(deviceModel) { preferences.overviewLayoutFlow(deviceModel) }.collectAsState()
    val layout = OverviewLayout.parse(storedLayout)
    val storedToggles by remember(deviceModel) { preferences.quickTogglesFlow(deviceModel) }.collectAsState()
    val availableToggles = settings.filter { it.second is Setting.ToggleSetting }.map { it.first }
    val quickToggles = quickToggleIds(storedToggles, availableToggles).mapNotNull { id ->
        (values[id] as? Setting.ToggleSetting)?.let { id to it }
    }
    val batteries = withMissingEarbud(deviceBatteries(values), oneEarbudOnly)
    val scrollState = rememberScrollState()
    var cardBottom by remember { mutableIntStateOf(0) }
    // the card has scrolled out of sight: a narrow bar keeps the name and batteries on screen
    val collapsed by remember { derivedStateOf { cardBottom > 0 && scrollState.value > cardBottom } }
    Box {
        Column(
            Modifier.verticalScroll(scrollState).padding(horizontal = 20.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            DeviceHeroCard(
                model = deviceModel,
                isDemo = false,
                state = ActiveDeviceState.Connected,
                batteries = batteries,
                macAddress = macAddress,
                oneEarbudBadge = oneEarbudOnly && showOneEarbudBadge,
                modifier = Modifier.onGloballyPositioned { cardBottom = it.size.height },
            ) {
                if (availableToggles.isNotEmpty()) {
                    QuickToggleRow(
                        toggles = quickToggles,
                        available = availableToggles,
                        availability = availability,
                        onToggle = { id, on -> setSettings(listOf(id to on.toValue())) },
                        onChoose = { preferences.setQuickToggles(deviceModel, it.joinToString(",")) },
                    )
                }
            }
            Column(Modifier.slidesWithDeviceCard(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                val blocks = layout.visibleBlocks.filter { block ->
                    when (block) {
                        OverviewBlock.SoundMode -> hasSound
                        OverviewBlock.Equalizer -> hasEqualizerBlock(values, hasEqualizerCategory, !equalizerAvailable)
                        OverviewBlock.QuickPresets -> quickPresets.isNotEmpty()
                        OverviewBlock.Controls -> hasControls
                        OverviewBlock.Comfort -> hasComfort
                        OverviewBlock.Spatial -> hasEffects
                        OverviewBlock.Power -> hasPower
                        OverviewBlock.Connections -> hasConnections
                        OverviewBlock.Case -> hasCase
                    }
                }
                var index = 0
                while (index < blocks.size) {
                    val block = blocks[index]
                    val next = blocks.getOrNull(index + 1)
                    // controls and comfort side by side when they follow each other
                    val pair =
                        next != null && setOf(block, next) == setOf(OverviewBlock.Controls, OverviewBlock.Comfort)
                    when {
                        pair -> Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            listOfNotNull(block, next).forEach { shortcut ->
                                OverviewShortcut(shortcut, onControls, onComfort, Modifier.weight(1f))
                            }
                        }

                        block == OverviewBlock.SoundMode -> {
                            SoundSectionHeading(stringResource(R.string.sound_overview_sound_mode), onClick = onSound)
                            val ambient = values["ambientSoundMode"] as? Setting.SelectSetting
                            if (soundModesView == SoundModesView.Scale && supportsSoundModeScale(deviceModel, values)) {
                                SoundModeScale(values, setSettings)
                            } else if (ambient != null) {
                                SoundModeTiles(ambient, compact = true) {
                                    setSettings(listOf("ambientSoundMode" to it.toValue()))
                                }
                            }
                        }

                        block == OverviewBlock.Equalizer ->
                            OverviewEqualizer(values, equalizerAvailability, onEqualizer)

                        block == OverviewBlock.QuickPresets -> {
                            SoundSectionHeading(stringResource(R.string.quick_presets))
                            val readOnly = LocalSettingsReadOnly.current
                            FlowRow(
                                horizontalArrangement = Arrangement.spacedBy(8.dp),
                                verticalArrangement = Arrangement.spacedBy(4.dp),
                            ) {
                                quickPresets.forEach { name ->
                                    FilterChip(
                                        selected = name == activeQuickPreset,
                                        onClick = { onActivateQuickPreset(name) },
                                        label = { Text(name) },
                                        enabled = !readOnly,
                                    )
                                }
                            }
                        }

                        block == OverviewBlock.Controls || block == OverviewBlock.Comfort ->
                            OverviewShortcut(block, onControls, onComfort, Modifier.fillMaxWidth())

                        block == OverviewBlock.Spatial -> SoundDashboardLink(
                            stringResource(R.string.sound_overview_effects),
                            // whether spatial audio is on, for the devices that have it
                            (values["spatialAudio"] as? Setting.ToggleSetting)?.let {
                                val state = if (it.value) {
                                    R.string.sound_overview_spatial_on
                                } else {
                                    R.string.sound_overview_spatial_off
                                }
                                stringResource(state)
                            },
                            R.drawable.speaker_24px,
                            onSpatial,
                        )

                        block == OverviewBlock.Power -> SoundDashboardLink(
                            stringResource(R.string.sound_overview_power),
                            null,
                            R.drawable.power_settings_new_24px,
                            onPower,
                        )

                        block == OverviewBlock.Connections -> SoundDashboardLink(
                            stringResource(R.string.sound_overview_connections),
                            null,
                            R.drawable.devices_other_24px,
                            onConnections,
                        )

                        block == OverviewBlock.Case -> SoundDashboardLink(
                            stringResource(R.string.sound_overview_case),
                            null,
                            R.drawable.earbud_case_24px,
                            onCase,
                        )
                    }
                    index += if (pair) 2 else 1
                }
                SoundDashboardLink(
                    stringResource(R.string.sound_overview_all_settings),
                    stringResource(R.string.sound_overview_all_settings_hint),
                    R.drawable.settings_24px,
                    onAllSettings,
                )
                TextButton(onClick = onEditLayout, modifier = Modifier.align(Alignment.CenterHorizontally)) {
                    Icon(painterResource(R.drawable.edit_24px), null, Modifier.size(18.dp))
                    Text(stringResource(R.string.sound_overview_edit), Modifier.padding(start = 8.dp))
                }
            }
        }
        AnimatedVisibility(
            visible = collapsed,
            modifier = Modifier.align(Alignment.TopCenter),
            enter = fadeIn() + slideInVertically { -it },
            exit = fadeOut() + slideOutVertically { -it },
        ) {
            CompactDeviceBar(deviceModel, batteries)
        }
    }
}

private fun hasEqualizerBlock(
    values: Map<String, Setting>,
    hasEqualizerCategory: Boolean,
    equalizerUnavailable: Boolean,
) = values["volumeAdjustments"] is Setting.EqualizerSetting || values.containsKey("presetEqualizerProfile") ||
    (hasEqualizerCategory && equalizerUnavailable)

@Composable
private fun OverviewEqualizer(
    values: Map<String, Setting>,
    availability: ClientSettingAvailability,
    onEqualizer: () -> Unit,
) {
    SoundSectionHeading(stringResource(R.string.sound_overview_your_sound))
    if (values["volumeAdjustments"] is Setting.EqualizerSetting || values.containsKey("presetEqualizerProfile")) {
        val profileName = (values["customEqualizerProfile"] as? Setting.ModifiableSelectSetting)?.value
            ?: (values["presetEqualizerProfile"] as? Setting.PresetEqualizerProfileSelect)?.let {
                it.value?.let { value -> it.select.localizedOptions.getOrNull(it.select.options.indexOf(value)) }
            }
            ?: (values["presetEqualizerProfile"] as? Setting.OptionalSelectSetting)?.let {
                it.value?.let { value -> it.setting.localizedOptions.getOrNull(it.setting.options.indexOf(value)) }
            }
        SoundDashboardLink(
            stringResource(R.string.equalizer),
            profileName ?: stringResource(R.string.custom),
            R.drawable.equalizer_24px,
            onEqualizer,
        )
    } else if (!availability.isAvailable()) {
        // The equalizer can not be read or changed until both earbuds are connected. Keep it in place with the reason
        // instead of letting it disappear.
        SoundDashboardLink(
            stringResource(R.string.equalizer),
            stringResource(availabilityReasonText(requireNotNull(availability.blockedReason()))),
            R.drawable.equalizer_24px,
            onClick = {},
            enabled = false,
        )
    }
}

@Composable
private fun OverviewShortcut(block: OverviewBlock, onControls: () -> Unit, onComfort: () -> Unit, modifier: Modifier) {
    if (block == OverviewBlock.Controls) {
        SoundShortcut(stringResource(R.string.sound_overview_controls), R.drawable.sound_hand, onControls, modifier)
    } else {
        SoundShortcut(
            stringResource(R.string.sound_overview_comfort),
            R.drawable.sound_heart,
            onComfort,
            modifier,
            warm = true,
        )
    }
}

/** The device name and batteries in one line, shown while the card is scrolled out of sight. */
@Composable
private fun CompactDeviceBar(deviceModel: String, batteries: List<DeviceBattery>) {
    // the strip around the bar hides the content scrolling beneath it
    Surface(
        modifier = Modifier.fillMaxWidth()
            .background(MaterialTheme.colorScheme.background)
            .padding(horizontal = 12.dp, vertical = 4.dp),
        shape = RoundedCornerShape(20.dp),
        color = MaterialTheme.colorScheme.secondaryContainer,
        contentColor = MaterialTheme.colorScheme.onSecondaryContainer,
        shadowElevation = 3.dp,
    ) {
        Row(
            Modifier.padding(horizontal = 16.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            DeviceArtOrIcon(deviceModel, Modifier.size(28.dp))
            Text(
                translateDeviceModel(deviceModel).removePrefix("Soundcore "),
                style = MaterialTheme.typography.titleSmall,
                modifier = Modifier.weight(1f),
                maxLines = 1,
            )
            Text(
                batteries.take(3).joinToString(" · ") { it.value },
                style = MaterialTheme.typography.labelMedium,
            )
        }
    }
}

internal fun batteryProgress(raw: String): Float? = clientBatteryProgress(raw)

internal fun isOnlyOneEarbudConnected(values: Map<String, Setting>): Boolean = clientIsOnlyOneEarbudConnected(values)

internal val BOTH_EARBUDS_SETTING_IDS by lazy { clientBothEarbudsSettingIds().toSet() }

internal fun categoryNeedsBothEarbuds(categoryId: String): Boolean = clientCategoryNeedsBothEarbuds(categoryId)

@Composable
internal fun SoundOneEarbudNotice(
    modifier: Modifier = Modifier,
    message: String = stringResource(R.string.sound_overview_one_earbud_message),
) {
    Surface(
        shape = RoundedCornerShape(20.dp),
        color = MaterialTheme.colorScheme.tertiaryContainer,
        contentColor = MaterialTheme.colorScheme.onTertiaryContainer,
        modifier = modifier.fillMaxWidth(),
    ) {
        Row(
            Modifier.padding(16.dp),
            horizontalArrangement = Arrangement.spacedBy(14.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(painterResource(R.drawable.earbuds_2_24px), contentDescription = null)
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(
                    stringResource(R.string.sound_overview_one_earbud_title),
                    style = MaterialTheme.typography.titleSmall,
                )
                Text(message, style = MaterialTheme.typography.bodySmall)
            }
        }
    }
}

@DrawableRes
internal fun soundModeIcon(option: String): Int = when (option) {
    "NoiseCanceling" -> R.drawable.sound_noise
    "Transparency" -> R.drawable.sound_transparency
    "AirplaneMode" -> R.drawable.sound_plane
    else -> R.drawable.headphones_24px
}

@Composable
private fun SoundSectionHeading(title: String, onClick: (() -> Unit)? = null) {
    Row(Modifier.fillMaxWidth().padding(top = 7.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(
            title,
            modifier = Modifier.weight(1f),
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.SemiBold,
        )
        if (onClick != null) TextButton(onClick = onClick) { Text(stringResource(R.string.sound_overview_customize)) }
    }
}

@Composable
fun SoundDashboardLink(
    title: String,
    description: String?,
    @DrawableRes icon: Int,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
) {
    Surface(
        onClick = onClick,
        enabled = enabled,
        shape = RoundedCornerShape(24.dp),
        color = MaterialTheme.colorScheme.surfaceContainerLowest,
        modifier = modifier.fillMaxWidth(),
    ) {
        Row(
            Modifier.padding(18.dp).heightIn(min = 48.dp).alpha(if (enabled) 1f else 0.6f),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Surface(shape = RoundedCornerShape(16.dp), color = MaterialTheme.colorScheme.primaryContainer) {
                Icon(
                    painterResource(icon),
                    null,
                    Modifier.padding(12.dp).size(23.dp),
                    tint = MaterialTheme.colorScheme.onPrimaryContainer,
                )
            }
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(title, style = MaterialTheme.typography.titleMedium)
                if (description !=
                    null
                ) {
                    Text(
                        description,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            if (enabled) {
                Icon(
                    painterResource(R.drawable.sound_chevron),
                    null,
                    Modifier.size(19.dp),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

@Composable
private fun SoundShortcut(
    title: String,
    @DrawableRes icon: Int,
    onClick: () -> Unit,
    modifier: Modifier,
    warm: Boolean = false,
) {
    Surface(
        onClick = onClick,
        modifier = modifier,
        shape = RoundedCornerShape(24.dp),
        color = if (warm) MaterialTheme.colorScheme.secondaryContainer else MaterialTheme.colorScheme.surfaceContainer,
    ) {
        Column(Modifier.padding(17.dp).heightIn(min = 62.dp), verticalArrangement = Arrangement.spacedBy(11.dp)) {
            Icon(
                painterResource(icon),
                null,
                tint = if (warm) MaterialTheme.colorScheme.onSecondaryContainer else MaterialTheme.colorScheme.primary,
            )
            Text(title, style = MaterialTheme.typography.labelLarge)
        }
    }
}
