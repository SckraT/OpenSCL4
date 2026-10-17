package com.oppzippy.openscq30.ui.sound

import androidx.annotation.StringRes
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.features.preferences.DeviceUiPreferencesViewModel
import com.oppzippy.openscq30.features.preferences.SoundModesView
import com.oppzippy.openscq30.lib.bindings.translateSettingId
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.lib.wrapper.Value
import com.oppzippy.openscq30.lib.wrapper.toValue

@Composable
fun SoundModesPage(
    deviceModel: String,
    settings: List<Pair<String, Setting>>,
    setSettings: (List<Pair<String, Value>>) -> Unit,
    preferences: DeviceUiPreferencesViewModel = hiltViewModel(),
) {
    val ambient = settings.firstOrNull { it.first == "ambientSoundMode" }?.second as? Setting.SelectSetting
    val soundModesView by preferences.soundModesViewFlow.collectAsState()
    val values = settings.toMap()
    // on the scale, the strengths are part of it, so their own sliders are left out
    val useScale = soundModesView == SoundModesView.Scale && supportsSoundModeScale(deviceModel, values)
    SoundDevicePage(
        headline = R.string.sound_device_modes_headline,
        description = R.string.sound_device_modes_description,
    ) {
        if (useScale) {
            SoundModeScale(values, setSettings)
        } else if (ambient != null) {
            SoundModeTiles(ambient) { setSettings(listOf("ambientSoundMode" to it.toValue())) }
        }
        settings.filter { (id, _) ->
            if (ambient == null) {
                true
            } else {
                when (id) {
                    "ambientSoundMode" -> false

                    "manualNoiseCanceling" -> !useScale && ambient.value == "NoiseCanceling"

                    "noiseCancelingMode", "customNoiseCanceling", "adaptiveNoiseCanceling" ->
                        ambient.value == "NoiseCanceling"

                    "manualTransparency" -> !useScale && ambient.value == "Transparency"

                    "transparencyMode" -> ambient.value == "Transparency"

                    "airplaneMode" -> ambient.value == "AirplaneMode"

                    else -> true
                }
            }
        }.forEach { (id, setting) -> SoundSettingEditor(id, setting, setSettings) }
    }
}

/**
 * The ambient sound modes as tiles, two per row, the same on the device overview and on the sound modes page.
 * Compact tiles are lower, for the overview.
 */
@Composable
internal fun SoundModeTiles(ambient: Setting.SelectSetting, compact: Boolean = false, onSelect: (String) -> Unit) {
    val largeText = LocalDensity.current.fontScale > 1.4f
    Column(Modifier.fillMaxWidth().selectableGroup(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        ambient.setting.options.chunked(if (largeText) 1 else 2).forEach { options ->
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                options.forEach { option ->
                    val selected = ambient.value == option
                    Surface(
                        modifier = Modifier.weight(1f),
                        shape = RoundedCornerShape(28.dp),
                        color = if (selected) {
                            MaterialTheme.colorScheme.primary
                        } else {
                            MaterialTheme.colorScheme.surfaceContainer
                        },
                        contentColor = if (selected) {
                            MaterialTheme.colorScheme.onPrimary
                        } else {
                            MaterialTheme.colorScheme.onSurface
                        },
                    ) {
                        Column(
                            Modifier.fillMaxWidth().heightIn(min = if (compact) 88.dp else 112.dp)
                                .selectable(selected, role = Role.RadioButton) { onSelect(option) }
                                .padding(18.dp),
                            verticalArrangement = Arrangement.spacedBy(if (compact) 12.dp else 18.dp),
                        ) {
                            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                                Icon(painterResource(soundModeIcon(option)), contentDescription = null)
                                if (selected) {
                                    Icon(painterResource(R.drawable.check_24px), contentDescription = null)
                                }
                            }
                            Text(
                                ambient.setting.localizedOption(option) ?: option,
                                style = MaterialTheme.typography.labelLarge,
                            )
                        }
                    }
                }
                if (options.size == 1 && !largeText) Spacer(Modifier.weight(1f))
            }
        }
    }
}

@Composable
fun SoundControlsPage(settings: List<Pair<String, Setting>>, setSettings: (List<Pair<String, Value>>) -> Unit) {
    val hasLeft = settings.any { isSideControl(it.first, "left") }
    val hasRight = settings.any { isSideControl(it.first, "right") }
    var rightSelected by rememberSaveable { mutableStateOf(false) }
    val selectedSide = if (hasRight && (!hasLeft || rightSelected)) "right" else "left"
    SoundDevicePage(headline = R.string.sound_device_controls_headline) {
        if (hasLeft || hasRight) {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                if (hasLeft) {
                    FilterChip(
                        selected = selectedSide == "left",
                        onClick = { rightSelected = false },
                        label = { Text(stringResource(R.string.sound_device_left_earbud)) },
                        modifier = Modifier.weight(1f).heightIn(min = 56.dp),
                    )
                }
                if (hasRight) {
                    FilterChip(
                        selected = selectedSide == "right",
                        onClick = { rightSelected = true },
                        label = { Text(stringResource(R.string.sound_device_right_earbud)) },
                        modifier = Modifier.weight(1f).heightIn(min = 56.dp),
                    )
                }
            }
            Surface(shape = RoundedCornerShape(28.dp), color = MaterialTheme.colorScheme.secondaryContainer) {
                Row(
                    Modifier.fillMaxWidth().padding(24.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(24.dp),
                ) {
                    Icon(
                        painterResource(R.drawable.earbuds_2_24px),
                        contentDescription = null,
                        modifier = Modifier.size(64.dp),
                    )
                    Text(
                        stringResource(
                            if (selectedSide ==
                                "right"
                            ) {
                                R.string.sound_device_right_earbud
                            } else {
                                R.string.sound_device_left_earbud
                            },
                        ),
                        style = MaterialTheme.typography.titleMedium,
                    )
                }
            }
        }
        settings.filter { (id, _) -> isSideControl(id, selectedSide) }.forEach { (id, setting) ->
            SoundSettingEditor(id, setting, setSettings)
        }
        settings.filterNot { (id, _) -> isSideControl(id, "left") || isSideControl(id, "right") }
            .forEach { (id, setting) -> SoundSettingEditor(id, setting, setSettings) }
    }
}

/** Use actual capability ids: an unrelated setting starting with 'left' is not a gesture. */
internal fun isSideControl(settingId: String, side: String): Boolean = settingId in listOf(
    "${side}SinglePress",
    "${side}DoublePress",
    "${side}TriplePress",
    "${side}LongPress",
    "${side}SlideUp",
    "${side}SlideDown",
    "${side}DoublePressDuringCall",
    "${side}LongPressDuringCall",
)

@Composable
fun SoundComfortPage(settings: List<Pair<String, Setting>>, setSettings: (List<Pair<String, Value>>) -> Unit) {
    SoundDevicePage(
        headline = R.string.sound_device_comfort_headline,
        description = R.string.sound_device_comfort_description,
    ) {
        settings.forEach { (id, setting) -> SoundSettingEditor(id, setting, setSettings) }
        if (settings.any { it.first == "limitHighVolume" }) {
            SoundDeviceNote(R.string.sound_device_volume_note)
        }
    }
}

@Composable
fun SoundConnectionsPage(settings: List<Pair<String, Setting>>, setSettings: (List<Pair<String, Value>>) -> Unit) {
    val multipoint = settings.any { it.first == "dualConnections" || it.first == "dualConnectionsDevices" }
    SoundDevicePage(
        headline = if (multipoint) {
            R.string.sound_device_connections_headline
        } else {
            R.string.sound_device_connections_generic_headline
        },
        description = if (multipoint) R.string.sound_device_connections_intro else null,
    ) {
        if (multipoint) {
            Surface(shape = RoundedCornerShape(28.dp), color = MaterialTheme.colorScheme.tertiaryContainer) {
                Row(
                    Modifier.fillMaxWidth().padding(24.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceEvenly,
                ) {
                    Icon(
                        painterResource(R.drawable.bluetooth_24px),
                        contentDescription = null,
                        modifier = Modifier.size(32.dp),
                    )
                    Icon(
                        painterResource(R.drawable.earbuds_2_24px),
                        contentDescription = null,
                        modifier = Modifier.size(52.dp),
                    )
                    Icon(
                        painterResource(R.drawable.devices_other_24px),
                        contentDescription = null,
                        modifier = Modifier.size(32.dp),
                    )
                }
            }
        }
        settings.forEach { (id, setting) -> SoundSettingEditor(id, setting, setSettings) }
        if (multipoint) SoundDeviceNote(R.string.sound_device_connections_note)
    }
}

@Composable
fun SoundCasePage(settings: List<Pair<String, Setting>>, setSettings: (List<Pair<String, Value>>) -> Unit) {
    SoundDevicePage(headline = R.string.sound_device_case_headline) {
        val battery = settings.firstOrNull { it.first == "caseBatteryLevel" }
        if (battery != null) {
            val value = (battery.second as? Setting.InformationSetting)?.translatedValue
            if (value != null) {
                Surface(shape = RoundedCornerShape(28.dp), color = MaterialTheme.colorScheme.tertiaryContainer) {
                    Column(Modifier.fillMaxWidth().padding(24.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                        Text(translateSettingId(battery.first), style = MaterialTheme.typography.labelLarge)
                        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                            Text(value, style = MaterialTheme.typography.headlineLarge, modifier = Modifier.weight(1f))
                            Icon(painterResource(R.drawable.earbud_case_24px), contentDescription = null)
                        }
                    }
                }
            } else {
                SoundSettingEditor(battery.first, battery.second, setSettings)
            }
        }
        settings.filterNot { it.first == "caseBatteryLevel" }.forEach { (id, setting) ->
            SoundSettingEditor(id, setting, setSettings)
        }
        if (settings.any { it.first in setOf("findDevice", "remoteCamera", "atmospheric") }) {
            SoundDeviceNote(R.string.sound_device_case_note)
        }
    }
}

/** How the sound gets to the device and what is done to it: the codec, latency, spatial audio and other effects. */
@Composable
fun SoundEffectsPage(settings: List<Pair<String, Setting>>, setSettings: (List<Pair<String, Value>>) -> Unit) {
    val spatial = settings.any { it.first == "spatialAudio" }
    SoundDevicePage(
        headline = R.string.sound_device_spatial_headline,
        description = R.string.sound_device_effects_description,
    ) {
        if (spatial) {
            Surface(shape = RoundedCornerShape(28.dp), color = MaterialTheme.colorScheme.primaryContainer) {
                Column(
                    Modifier.fillMaxWidth().padding(32.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Icon(
                        painterResource(R.drawable.headphones_24px),
                        contentDescription = null,
                        modifier = Modifier.size(80.dp),
                        tint = MaterialTheme.colorScheme.onPrimaryContainer,
                    )
                }
            }
        }
        settings.forEach { (id, setting) ->
            if (id == "spatialAudioMode" && setting is Setting.SelectSetting) {
                Text(translateSettingId(id), style = MaterialTheme.typography.titleSmall)
                FlowRow(
                    Modifier.fillMaxWidth().selectableGroup(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    setting.setting.options.forEach { option ->
                        FilterChip(
                            selected = setting.value == option,
                            onClick = { setSettings(listOf(id to option.toValue())) },
                            label = { Text(setting.setting.localizedOption(option) ?: option) },
                            modifier = Modifier.heightIn(min = 48.dp),
                        )
                    }
                }
            } else {
                SoundSettingEditor(id, setting, setSettings)
            }
        }
        if (spatial) SoundDeviceNote(R.string.sound_device_spatial_note)
    }
}

/** When the device turns itself off, and the voice prompts it gives. */
@Composable
fun SoundPowerPage(settings: List<Pair<String, Setting>>, setSettings: (List<Pair<String, Value>>) -> Unit) {
    SoundDevicePage(
        headline = R.string.sound_device_power_headline,
        description = R.string.sound_device_power_description,
    ) {
        settings.forEach { (id, setting) -> SoundSettingEditor(id, setting, setSettings) }
    }
}

/** Also usable for future capabilities and categories without a dedicated layout. */
@Composable
fun SoundSettingsListPage(settings: List<Pair<String, Setting>>, setSettings: (List<Pair<String, Value>>) -> Unit) {
    Column(
        Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        settings.forEach { (id, setting) -> SoundSettingEditor(id, setting, setSettings) }
        Spacer(Modifier.height(16.dp))
    }
}

@Composable
private fun SoundDevicePage(
    @StringRes headline: Int,
    @StringRes description: Int? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    Column(
        Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text(
            stringResource(headline),
            modifier = Modifier.semantics { heading() },
            style = MaterialTheme.typography.headlineLarge.copy(
                fontSize = 30.sp,
                lineHeight = 34.sp,
                fontWeight = FontWeight.Bold,
            ),
        )
        description?.let {
            Text(
                stringResource(it),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Spacer(Modifier.height(4.dp))
        content()
        Spacer(Modifier.height(16.dp))
    }
}

@Composable
private fun SoundDeviceNote(@StringRes text: Int) {
    Surface(shape = RoundedCornerShape(24.dp), color = MaterialTheme.colorScheme.surfaceContainer) {
        Row(
            Modifier.fillMaxWidth().padding(16.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Icon(painterResource(R.drawable.info_24px), contentDescription = null, modifier = Modifier.size(20.dp))
            Text(
                stringResource(text),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.weight(1f),
            )
        }
    }
}
