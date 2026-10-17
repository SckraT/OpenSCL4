package com.oppzippy.openscq30.ui.sound

import androidx.annotation.DrawableRes
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.lib.bindings.translateDeviceModel
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.lib.wrapper.Value
import com.oppzippy.openscq30.lib.wrapper.toValue

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
    hasEqualizerCategory: Boolean = false,
    onSound: () -> Unit,
    onEqualizer: () -> Unit,
    onControls: () -> Unit,
    onComfort: () -> Unit,
    onConnections: () -> Unit,
    onSpatial: () -> Unit,
    onCase: () -> Unit,
    onAllSettings: () -> Unit,
    setSettings: (List<Pair<String, Value>>) -> Unit,
) {
    val largeText = LocalDensity.current.fontScale > 1.4f
    val values = settings.toMap()
    val modelName = translateDeviceModel(deviceModel)
    Column(
        Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 20.dp, vertical = 8.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Surface(shape = RoundedCornerShape(32.dp), color = MaterialTheme.colorScheme.secondaryContainer) {
            Box(Modifier.fillMaxWidth().heightIn(min = 172.dp)) {
                val art = deviceArt(deviceModel)
                if (!largeText && art != null) {
                    SoundDeviceArt(art, Modifier.align(Alignment.CenterEnd).padding(8.dp).size(184.dp))
                } else if (!largeText) {
                    Icon(
                        painterResource(R.drawable.headphones_24px),
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.onSecondaryContainer.copy(alpha = 0.2f),
                        modifier = Modifier.align(Alignment.CenterEnd).padding(end = 16.dp).size(120.dp),
                    )
                }
                Column(
                    Modifier.fillMaxWidth(if (largeText) 1f else 0.59f).padding(20.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Text(
                        stringResource(R.string.sound_overview_your_device),
                        style = MaterialTheme.typography.labelMedium,
                    )
                    Text(
                        modelName.removePrefix("Soundcore "),
                        style = MaterialTheme.typography.headlineLarge.copy(fontSize = 30.sp, lineHeight = 34.sp),
                        fontWeight = FontWeight.Medium,
                    )
                    Surface(
                        shape = RoundedCornerShape(50),
                        color = MaterialTheme.colorScheme.onSecondaryContainer.copy(alpha = 0.09f),
                    ) {
                        Text(
                            stringResource(R.string.connected),
                            style = MaterialTheme.typography.labelSmall,
                            modifier = Modifier.padding(horizontal = 10.dp, vertical = 7.dp),
                        )
                    }
                }
            }
        }
        val batteries = listOf(
            Triple("batteryLevelLeft", R.string.sound_overview_left, R.drawable.earbuds_2_24px),
            Triple("batteryLevelRight", R.string.sound_overview_right, R.drawable.earbuds_2_24px),
            Triple("caseBatteryLevel", R.string.sound_overview_case, R.drawable.earbud_case_24px),
            Triple("batteryLevel", R.string.sound_overview_battery, R.drawable.headphones_24px),
        ).mapNotNull { (id, label, icon) ->
            (values[id] as? Setting.InformationSetting)?.let { Triple(it, label, icon) }
        }
        batteries.chunked(if (largeText) 1 else 3).forEach { batteryRow ->
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                batteryRow.forEach { (setting, label, icon) ->
                    val name = stringResource(label)
                    Surface(
                        shape = RoundedCornerShape(20.dp),
                        color = MaterialTheme.colorScheme.surfaceContainerLowest,
                        modifier = Modifier.weight(1f).clearAndSetSemantics {
                            contentDescription = "$name: ${setting.translatedValue}"
                        },
                    ) {
                        Column(Modifier.padding(13.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                            Row(
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(4.dp),
                            ) {
                                Icon(painterResource(icon), contentDescription = null, modifier = Modifier.size(15.dp))
                                Text(name, style = MaterialTheme.typography.labelSmall)
                            }
                            Text(
                                setting.translatedValue,
                                style = MaterialTheme.typography.headlineSmall,
                                fontWeight = FontWeight.Medium,
                            )
                            batteryProgress(setting.value)?.let { progress ->
                                LinearProgressIndicator(
                                    progress = { progress },
                                    modifier = Modifier.fillMaxWidth(),
                                    color = MaterialTheme.colorScheme.onSecondaryContainer,
                                    trackColor = MaterialTheme.colorScheme.surfaceContainerHighest,
                                    drawStopIndicator = {},
                                )
                            }
                        }
                    }
                }
            }
        }
        val oneEarbudOnly = isOnlyOneEarbudConnected(values)
        if (oneEarbudOnly) {
            SoundOneEarbudNotice()
        }
        val ambient = values["ambientSoundMode"] as? Setting.SelectSetting
        if (hasSound) {
            SoundSectionHeading(stringResource(R.string.sound_overview_sound_mode), onClick = onSound)
            ambient?.setting?.options?.chunked(if (largeText) 1 else 2)?.forEach { options ->
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    options.forEach { option ->
                        val selected = ambient.value == option
                        Surface(
                            modifier = Modifier.weight(1f),
                            shape = RoundedCornerShape(if (selected) 24.dp else 16.dp),
                            color = if (selected) {
                                MaterialTheme.colorScheme.primaryContainer
                            } else {
                                MaterialTheme.colorScheme.surfaceContainer
                            },
                            contentColor = if (selected) {
                                MaterialTheme.colorScheme.onPrimaryContainer
                            } else {
                                MaterialTheme.colorScheme.onSurface
                            },
                        ) {
                            Box(
                                Modifier.fillMaxWidth().heightIn(min = 64.dp)
                                    .selectable(selected, role = Role.RadioButton) {
                                        setSettings(listOf("ambientSoundMode" to option.toValue()))
                                    }.padding(12.dp),
                            ) {
                                Row(
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                                ) {
                                    Icon(painterResource(soundModeIcon(option)), null, Modifier.size(20.dp))
                                    Text(
                                        ambient.setting.localizedOptions.getOrElse(
                                            ambient.setting.options.indexOf(option),
                                        ) {
                                            option
                                        },
                                        style = MaterialTheme.typography.labelMedium,
                                        modifier = Modifier.weight(1f),
                                    )
                                }
                                if (selected) {
                                    Icon(
                                        painterResource(R.drawable.check_24px),
                                        null,
                                        Modifier.align(Alignment.TopEnd).size(10.dp),
                                    )
                                }
                            }
                        }
                    }
                    if (options.size == 1 && !largeText) Spacer(Modifier.weight(1f))
                }
            }
        }
        if (values["volumeAdjustments"] is Setting.EqualizerSetting || values.containsKey("presetEqualizerProfile")) {
            SoundSectionHeading(stringResource(R.string.sound_overview_your_sound))
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
        } else if (hasEqualizerCategory && oneEarbudOnly) {
            // The equalizer can't be read or changed until both earbuds are connected. Keep it in place with the
            // reason instead of letting it disappear.
            SoundSectionHeading(stringResource(R.string.sound_overview_your_sound))
            SoundDashboardLink(
                stringResource(R.string.equalizer),
                stringResource(R.string.sound_overview_needs_both_earbuds),
                R.drawable.equalizer_24px,
                onClick = {},
                enabled = false,
            )
        }
        if (hasControls || hasComfort) {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                if (hasControls) {
                    SoundShortcut(
                        stringResource(R.string.sound_overview_controls),
                        R.drawable.sound_hand,
                        onControls,
                        Modifier.weight(1f),
                    )
                }
                if (hasComfort) {
                    SoundShortcut(
                        stringResource(R.string.sound_overview_comfort),
                        R.drawable.sound_heart,
                        onComfort,
                        Modifier.weight(1f),
                        warm = true,
                    )
                }
            }
        }
        if (values.containsKey("spatialAudio")) {
            SoundDashboardLink(
                stringResource(R.string.sound_overview_spatial),
                stringResource(
                    if ((values["spatialAudio"] as? Setting.ToggleSetting)?.value ==
                        true
                    ) {
                        R.string.sound_overview_on
                    } else {
                        R.string.sound_overview_off
                    },
                ),
                R.drawable.speaker_24px,
                onSpatial,
            )
        }
        if (hasConnections) {
            SoundDashboardLink(
                stringResource(R.string.sound_overview_connections),
                null,
                R.drawable.devices_other_24px,
                onConnections,
            )
        }
        if (hasCase) {
            SoundDashboardLink(
                stringResource(R.string.sound_overview_case),
                null,
                R.drawable.earbud_case_24px,
                onCase,
            )
        }
        SoundDashboardLink(
            stringResource(R.string.sound_overview_all_settings),
            stringResource(R.string.sound_overview_all_settings_hint),
            R.drawable.settings_24px,
            onAllSettings,
        )
    }
}

/** Native battery values are fractions, e.g. 83/100 or 7/10 for coarse case readings. */
internal fun batteryProgress(raw: String): Float? {
    val parts = raw.trim().split('/')
    val level: Float
    val maximum: Float
    if (parts.size == 2) {
        level = parts[0].trim().toFloatOrNull() ?: return null
        maximum = parts[1].trim().toFloatOrNull() ?: return null
    } else if (parts.size == 1) {
        level = raw.trim().removeSuffix("%").trim().toFloatOrNull() ?: return null
        maximum = 100f
    } else {
        return null
    }
    return if (level.isFinite() && maximum.isFinite() && maximum > 0f) {
        (level / maximum).coerceIn(0f, 1f)
    } else {
        null
    }
}

/**
 * The earbuds report whether they are connected to each other. With one of them in the case, settings that need both,
 * such as the equalizer, are not available.
 */
internal fun isOnlyOneEarbudConnected(values: Map<String, Setting>): Boolean =
    (values["twsStatus"] as? Setting.InformationSetting)?.value == "Disconnected"

/** Settings the device only exposes while both earbuds are connected. */
internal val BOTH_EARBUDS_SETTING_IDS = setOf("presetEqualizerProfile", "customEqualizerProfile", "volumeAdjustments")

/** Whether a settings category is empty while only one earbud is connected because all of it needs both earbuds. */
internal fun categoryNeedsBothEarbuds(categoryId: String): Boolean = categoryId == "equalizer"

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
