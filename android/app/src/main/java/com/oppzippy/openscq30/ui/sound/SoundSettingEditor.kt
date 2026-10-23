@file:OptIn(ExperimentalMaterial3Api::class)

package com.oppzippy.openscq30.ui.sound

import android.content.ClipData
import android.content.ClipboardManager
import android.text.format.DateFormat
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TimePicker
import androidx.compose.material3.TimePickerDialog
import androidx.compose.material3.TimePickerState
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.core.content.getSystemService
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.lib.bindings.AvailabilityReason
import com.oppzippy.openscq30.lib.bindings.WriteOutcome
import com.oppzippy.openscq30.lib.bindings.translateSettingId
import com.oppzippy.openscq30.lib.wrapper.MultiSelectWithRemoveCommandInner
import com.oppzippy.openscq30.lib.wrapper.Range
import com.oppzippy.openscq30.lib.wrapper.Select
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.lib.wrapper.Value
import com.oppzippy.openscq30.lib.wrapper.blockedReason
import com.oppzippy.openscq30.lib.wrapper.isAvailable
import com.oppzippy.openscq30.lib.wrapper.toValue
import com.oppzippy.openscq30.ui.PendingSettingWrite
import com.oppzippy.openscq30.ui.devicesettings.composables.Equalizer
import com.oppzippy.openscq30.ui.devicesettings.composables.ReadOnlyEqualizer
import com.oppzippy.openscq30.ui.utils.ModifiableSelect
import com.oppzippy.openscq30.ui.utils.throttledState
import java.time.LocalTime
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import kotlin.math.roundToInt
import kotlinx.coroutines.launch

/**
 * Sets a setting and waits for the result, for settings whose editors have to keep what the user entered if setting it
 * fails, such as importing. Editors fall back to a fire and forget change when nothing provides this.
 */
val LocalSettingValueSetter = staticCompositionLocalOf<(suspend (String, Value) -> Result<Unit>)?> { null }

/** Writes that the device has not confirmed yet. The controls show the requested value, disabled, until they arrive. */
val LocalPendingSettingWrites = staticCompositionLocalOf<Map<String, PendingSettingWrite>> { emptyMap() }

/** The last outcome of a write per setting, when it was not applied. */
val LocalSettingWriteErrors = staticCompositionLocalOf<Map<String, WriteOutcome>> { emptyMap() }

/** While the connection is recovering, controls show the last known values but cannot change them. */
val LocalSettingsReadOnly = staticCompositionLocalOf { false }

/** Renders the backend capability directly, including options unknown to this UI version. */
@Composable
fun SoundSettingEditor(
    settingId: String,
    setting: Setting,
    setSettings: (List<Pair<String, Value>>) -> Unit,
    modifier: Modifier = Modifier,
) {
    val name = translateSettingId(settingId)
    val pending = LocalPendingSettingWrites.current[settingId]
    val requested = pending?.requestedValue
    val readOnly = LocalSettingsReadOnly.current
    val availability = LocalSettingAvailability.current[settingId]
    val enabled = pending == null && !readOnly && availability?.isAvailable() != false
    val onChange: (Value) -> Unit = { if (enabled) setSettings(listOf(settingId to it)) }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Surface(
            modifier = Modifier.fillMaxWidth().testTag("sound_setting_$settingId"),
            shape = RoundedCornerShape(24.dp),
            color = MaterialTheme.colorScheme.surfaceContainerLowest,
        ) {
            when (setting) {
                is Setting.ToggleSetting -> {
                    val value = (requested as? Value.BoolValue)?.value ?: setting.value
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .heightIn(min = 72.dp)
                            .toggleable(value = value, enabled = enabled, role = Role.Switch) {
                                onChange(it.toValue())
                            }
                            .padding(horizontal = 20.dp, vertical = 16.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(16.dp),
                    ) {
                        SoundSettingLabel(settingId, name, Modifier.weight(1f))
                        Switch(checked = value, enabled = enabled, onCheckedChange = null)
                    }
                }

                is Setting.SelectSetting -> SoundSelectSetting(
                    name,
                    setting.setting,
                    selectValue(requested, setting.value),
                    optional = false,
                    enabled = enabled,
                    onSelect = { value -> value?.let { onChange(it.toValue()) } },
                )

                is Setting.OptionalSelectSetting -> SoundSelectSetting(
                    name,
                    setting.setting,
                    selectValue(requested, setting.value),
                    optional = true,
                    enabled = enabled,
                    onSelect = { onChange(it.toValue()) },
                )

                is Setting.PresetEqualizerProfileSelect -> SoundSelectSetting(
                    name,
                    setting.select,
                    selectValue(requested, setting.value),
                    optional = true,
                    enabled = enabled,
                    onSelect = { onChange(it.toValue()) },
                )

                is Setting.I32RangeSetting -> SoundRangeSetting(
                    name,
                    setting,
                    (requested as? Value.I32Value)?.value ?: setting.value,
                    enabled,
                    onChange,
                )

                is Setting.MultiSelectSetting -> SoundMultiSelectSetting(
                    name = name,
                    select = setting.setting,
                    values = (requested as? Value.StringVecValue)?.value ?: setting.values,
                    enabled = enabled,
                    onChange = { onChange(it.toValue()) },
                )

                is Setting.MultiSelectWithRemoveSetting -> SoundMultiSelectSetting(
                    name = name,
                    select = setting.setting,
                    values = (requested as? Value.StringVecValue)?.value ?: setting.values,
                    enabled = enabled,
                    onChange = { onChange(it.toValue()) },
                    onRemove = { onChange(MultiSelectWithRemoveCommandInner.Remove(it).toValue()) },
                    isConnections = settingId == "dualConnectionsDevices",
                )

                is Setting.InformationSetting -> SoundInformationSetting(name, setting.translatedValue)

                is Setting.Action -> SoundActionSetting(name, enabled) { onChange(true.toValue()) }

                is Setting.HueColorPicker -> {
                    val (hue, setHue) = throttledState(
                        (requested as? Value.F32Value)?.value ?: setting.hue,
                        250,
                    ) { onChange(it.toValue()) }
                    Column(Modifier.padding(20.dp)) {
                        Text(name, style = MaterialTheme.typography.titleMedium)
                        Slider(
                            value = hue,
                            valueRange = 0f..360f,
                            enabled = enabled,
                            onValueChange = setHue,
                            modifier = Modifier.semantics { contentDescription = name },
                        )
                        Text(hue.roundToInt().toString(), style = MaterialTheme.typography.labelLarge)
                    }
                }

                is Setting.TimeOfDay -> {
                    val minutes = (requested as? Value.I32Value)?.value
                    SoundTimeSetting(
                        name,
                        minutes?.let { LocalTime.of(it / 60, it % 60) } ?: setting.getLocalTime(),
                        enabled,
                    ) {
                        onChange((it.hour * 60 + it.minute).toValue())
                    }
                }

                is Setting.ImportStringSetting -> {
                    val setValue = LocalSettingValueSetter.current
                    SoundImportSetting(name, setting.confirmationMessage, enabled) { text ->
                        if (setValue != null) {
                            setValue(settingId, text.toValue())
                        } else {
                            onChange(text.toValue())
                            Result.success(Unit)
                        }
                    }
                }

                is Setting.ModifiableSelectSetting -> Column(Modifier.padding(20.dp)) {
                    ModifiableSelect(
                        name = name,
                        options = setting.setting.localizedOptions,
                        selectedIndex = setting.value?.let { setting.setting.options.indexOf(it) },
                        onSelect = { onChange(setting.setting.options[it].toValue()) },
                        onAddOption = {
                            onChange(
                                com.oppzippy.openscq30.lib.wrapper.ModifiableSelectCommandInner.Add(it).toValue(),
                            )
                        },
                        onRemoveOption = {
                            onChange(
                                com.oppzippy.openscq30.lib.wrapper.ModifiableSelectCommandInner.Remove(
                                    setting.setting.options[it],
                                ).toValue(),
                            )
                        },
                    )
                }

                is Setting.EqualizerSetting -> {
                    val values = (requested as? Value.I16VecValue)?.value ?: setting.value
                    Column(Modifier.padding(20.dp)) {
                        Text(name, style = MaterialTheme.typography.titleMedium)
                        if (setting.readOnly || !enabled) {
                            ReadOnlyEqualizer(
                                modifier = Modifier.fillMaxWidth().height(120.dp),
                                color = MaterialTheme.colorScheme.primary,
                                bands = setting.setting.bandHz,
                                values = values,
                                minValue = setting.setting.min,
                                maxValue = setting.setting.max,
                                fractionDigits = setting.setting.fractionDigits,
                            )
                        } else {
                            Equalizer(
                                bands = setting.setting.bandHz,
                                values = values,
                                minValue = setting.setting.min,
                                maxValue = setting.setting.max,
                                fractionDigits = setting.setting.fractionDigits,
                                onValueChange = { changedIndex, value ->
                                    onChange(
                                        setting.value.mapIndexed { index, old ->
                                            if (index == changedIndex) value else old
                                        }.toValue(),
                                    )
                                },
                            )
                        }
                    }
                }
            }
        }
        availability?.blockedReason()?.let { reason ->
            Text(
                stringResource(availabilityReasonText(reason)),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                style = MaterialTheme.typography.bodySmall,
                modifier = Modifier.padding(horizontal = 20.dp).testTag("sound_setting_reason_$settingId"),
            )
        }
        if (pending != null) {
            LinearProgressIndicator(Modifier.fillMaxWidth().padding(horizontal = 20.dp))
        }
        LocalSettingWriteErrors.current[settingId]?.let { outcome ->
            Text(
                writeErrorText(outcome),
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodySmall,
                modifier = Modifier.padding(horizontal = 20.dp),
            )
        }
    }
}

private fun selectValue(requested: Value?, confirmed: String?): String? = when (requested) {
    is Value.StringValue -> requested.value
    is Value.OptionalStringValue -> requested.value
    else -> confirmed
}

@Composable
internal fun writeErrorText(outcome: WriteOutcome): String = when (outcome) {
    is WriteOutcome.Rejected -> stringResource(R.string.setting_write_rejected)

    WriteOutcome.TimedOut -> stringResource(R.string.setting_write_timed_out)

    WriteOutcome.Disconnected -> stringResource(R.string.setting_write_disconnected)

    is WriteOutcome.UnsupportedInState -> when (outcome.reason) {
        AvailabilityReason.REQUIRES_BOTH_EARBUDS ->
            stringResource(R.string.setting_write_unsupported_requires_both_earbuds)

        AvailabilityReason.NO_READ_BACK -> stringResource(R.string.setting_write_unsupported_no_read_back)
    }

    is WriteOutcome.Applied -> ""
}

@Composable
private fun SoundSettingLabel(settingId: String, name: String, modifier: Modifier = Modifier) {
    val description = when (settingId) {
        "wearingDetection", "autoPlayPause" -> R.string.sound_device_wearing_description

        "limitHighVolume" -> R.string.sound_device_volume_description

        "easyChat" -> R.string.sound_device_chat_description

        "dualConnections" -> R.string.sound_device_connections_description

        "findDevice", "remoteCamera", "atmospheric", "caseSpatialAudio" ->
            R.string.sound_device_case_feature_description

        else -> null
    }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(name, style = MaterialTheme.typography.titleMedium)
        description?.let {
            Text(
                stringResource(it),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                style = MaterialTheme.typography.bodySmall,
            )
        }
    }
}

@Composable
private fun SoundSelectSetting(
    name: String,
    select: Select,
    selectedValue: String?,
    optional: Boolean,
    enabled: Boolean,
    onSelect: (String?) -> Unit,
) {
    var isOpen by rememberSaveable { mutableStateOf(false) }
    Surface(
        onClick = { if (enabled) isOpen = true },
        enabled = enabled,
        color = MaterialTheme.colorScheme.surfaceContainerLowest,
    ) {
        Row(
            Modifier.fillMaxWidth().heightIn(min = 76.dp).padding(20.dp),
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Text(name, style = MaterialTheme.typography.titleMedium)
                Text(
                    select.localizedOption(selectedValue) ?: stringResource(R.string.none),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Icon(painterResource(R.drawable.equalizer_24px), contentDescription = null)
        }
    }
    if (isOpen) {
        ModalBottomSheet(
            onDismissRequest = { isOpen = false },
            sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        ) {
            Column(
                Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).selectableGroup().padding(20.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Text(name, style = MaterialTheme.typography.headlineSmall, modifier = Modifier.semantics { heading() })
                Spacer(Modifier.height(12.dp))
                if (optional) {
                    SoundSelectOption(stringResource(R.string.none), selectedValue == null) {
                        onSelect(null)
                        isOpen = false
                    }
                }
                select.options.forEachIndexed { index, option ->
                    SoundSelectOption(select.localizedOptions.getOrElse(index) { option }, selectedValue == option) {
                        onSelect(option)
                        isOpen = false
                    }
                }
                Spacer(Modifier.height(16.dp))
            }
        }
    }
}

internal fun Select.localizedOption(value: String?): String? {
    if (value == null) return null
    val index = options.indexOf(value)
    return localizedOptions.getOrNull(index) ?: value
}

@Composable
private fun SoundSelectOption(label: String, selected: Boolean, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 56.dp)
            .selectable(selected, role = Role.RadioButton, onClick = onClick)
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        RadioButton(selected = selected, onClick = null)
        Text(label, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
    }
}

/** Quantize relative to the capability's start, rather than assuming a 0-based range. */
internal fun snapSoundRangeValue(range: Range<Int>, value: Float): Int {
    val step = range.step.coerceAtLeast(1)
    return (range.start + ((value - range.start) / step).roundToInt() * step)
        .coerceIn(range.start, range.end)
}

@Composable
private fun SoundRangeSetting(
    name: String,
    setting: Setting.I32RangeSetting,
    displayedValue: Int,
    enabled: Boolean,
    onChange: (Value) -> Unit,
) {
    val range = setting.setting
    var displayed by remember(name, range, displayedValue) { mutableStateOf(displayedValue) }
    Column(Modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(name, style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
            Surface(color = MaterialTheme.colorScheme.primaryContainer, shape = RoundedCornerShape(20.dp)) {
                Text(
                    displayed.toString(),
                    modifier = Modifier.padding(horizontal = 14.dp, vertical = 8.dp),
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.onPrimaryContainer,
                )
            }
        }
        if (range.end > range.start) {
            Slider(
                modifier = Modifier.testTag("$name slider").semantics { contentDescription = name },
                enabled = enabled,
                value = displayed.toFloat().coerceIn(range.start.toFloat(), range.end.toFloat()),
                steps = ((range.end - range.start) / range.step.coerceAtLeast(1) - 1).coerceAtLeast(0),
                valueRange = range.start.toFloat()..range.end.toFloat(),
                onValueChange = { displayed = snapSoundRangeValue(range, it) },
                onValueChangeFinished = { onChange(displayed.toValue()) },
            )
        }
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text(range.start.toString(), style = MaterialTheme.typography.labelMedium)
            Text(range.end.toString(), style = MaterialTheme.typography.labelMedium)
        }
    }
}

@Composable
private fun SoundMultiSelectSetting(
    name: String,
    select: Select,
    values: List<String>,
    enabled: Boolean,
    onChange: (List<String>) -> Unit,
    onRemove: ((String) -> Unit)? = null,
    isConnections: Boolean = false,
) {
    var removeOption by rememberSaveable { mutableStateOf<String?>(null) }
    Column(Modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(name, style = MaterialTheme.typography.titleMedium)
        if (select.options.isEmpty()) {
            Text(
                stringResource(if (isConnections) R.string.sound_device_no_connections else R.string.none),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        select.options.forEachIndexed { index, option ->
            val isSelected = option in values
            val label = select.localizedOptions.getOrElse(index) { option }
            // The protocol supports at most two active multipoint connections.
            val optionEnabled = enabled && (!isConnections || isSelected || values.size < 2)
            Row(
                Modifier.fillMaxWidth().heightIn(min = 64.dp).then(
                    if (isConnections) {
                        Modifier
                    } else {
                        Modifier.toggleable(isSelected, enabled = enabled, role = Role.Checkbox) { checked ->
                            onChange(if (checked) values + option else values - option)
                        }
                    },
                ),
                horizontalArrangement = Arrangement.spacedBy(12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(Modifier.weight(1f)) {
                    Text(label, style = MaterialTheme.typography.bodyLarge)
                    if (isConnections) {
                        Text(
                            stringResource(
                                if (isSelected) R.string.connected else R.string.sound_device_not_connected,
                            ),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
                if (isConnections) {
                    Switch(
                        modifier = Modifier.semantics { contentDescription = label },
                        checked = isSelected,
                        enabled = optionEnabled,
                        onCheckedChange = { checked ->
                            onChange(if (checked) values + option else values - option)
                        },
                    )
                } else {
                    Checkbox(checked = isSelected, enabled = enabled, onCheckedChange = null)
                }
                if (onRemove != null && !isSelected) {
                    IconButton(enabled = enabled, onClick = { removeOption = option }) {
                        Icon(
                            painterResource(R.drawable.remove_24px),
                            contentDescription = stringResource(R.string.sound_device_forget_device),
                        )
                    }
                }
            }
        }
    }
    removeOption?.let { option ->
        AlertDialog(
            onDismissRequest = { removeOption = null },
            title = { Text(stringResource(R.string.sound_device_forget_device)) },
            text = { Text(stringResource(R.string.delete_confirm, select.localizedOption(option) ?: option)) },
            confirmButton = {
                TextButton(onClick = {
                    onRemove?.invoke(option)
                    removeOption = null
                }) {
                    Text(stringResource(R.string.delete))
                }
            },
            dismissButton = {
                TextButton(onClick = { removeOption = null }) { Text(stringResource(R.string.cancel)) }
            },
        )
    }
}

@Composable
private fun SoundInformationSetting(name: String, value: String) {
    val context = LocalContext.current
    Row(
        Modifier.fillMaxWidth().heightIn(min = 72.dp).padding(start = 20.dp, end = 8.dp, top = 12.dp, bottom = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(name, style = MaterialTheme.typography.titleMedium)
            Text(value, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        IconButton(onClick = {
            context.getSystemService<ClipboardManager>()?.setPrimaryClip(ClipData.newPlainText(name, value))
        }) {
            Icon(
                painterResource(R.drawable.content_copy_24px),
                contentDescription = stringResource(R.string.copy_x_to_clipboard, name),
            )
        }
    }
}

@Composable
private fun SoundActionSetting(name: String, enabled: Boolean, onExecute: () -> Unit) {
    var isConfirming by rememberSaveable { mutableStateOf(false) }
    TextButton(
        modifier = Modifier.fillMaxWidth().heightIn(min = 64.dp).padding(8.dp),
        enabled = enabled,
        onClick = { isConfirming = true },
    ) { Text(name) }
    if (isConfirming) {
        AlertDialog(
            onDismissRequest = { isConfirming = false },
            title = { Text(name) },
            confirmButton = {
                TextButton(onClick = {
                    isConfirming = false
                    onExecute()
                }) { Text(stringResource(R.string.confirm)) }
            },
            dismissButton = {
                TextButton(onClick = { isConfirming = false }) { Text(stringResource(R.string.cancel)) }
            },
        )
    }
}

@Composable
private fun SoundTimeSetting(name: String, time: LocalTime, enabled: Boolean, onChange: (LocalTime) -> Unit) {
    var picker by remember { mutableStateOf<TimePickerState?>(null) }
    val is24Hour = DateFormat.is24HourFormat(LocalContext.current)
    Column(Modifier.padding(20.dp)) {
        Text(name, style = MaterialTheme.typography.titleMedium)
        TextButton(
            enabled = enabled,
            onClick = { picker = TimePickerState(time.hour, time.minute, is24Hour) },
        ) {
            Text(time.format(DateTimeFormatter.ofLocalizedTime(FormatStyle.SHORT)))
        }
    }
    picker?.let { state ->
        TimePickerDialog(
            onDismissRequest = { picker = null },
            title = { Text(name) },
            confirmButton = {
                TextButton(onClick = {
                    onChange(LocalTime.of(state.hour, state.minute))
                    picker = null
                }) {
                    Text(stringResource(R.string.confirm))
                }
            },
        ) { TimePicker(state) }
    }
}

@Composable
private fun SoundImportSetting(
    name: String,
    confirmation: String?,
    enabled: Boolean,
    onImport: suspend (String) -> Result<Unit>,
) {
    // The text is only cleared once the import succeeded, so a failed import can be fixed instead of retyped. It also
    // has to survive the activity being recreated, since it is usually pasted in from somewhere else.
    var text by rememberSaveable { mutableStateOf("") }
    var confirming by rememberSaveable { mutableStateOf(false) }
    var importError by rememberSaveable { mutableStateOf<String?>(null) }
    var importing by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()

    fun import() {
        if (importing) return
        importing = true
        importError = null
        scope.launch {
            onImport(text)
                .onSuccess { text = "" }
                .onFailure { importError = it.message ?: it.toString() }
            importing = false
        }
    }

    Column(Modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        OutlinedTextField(
            value = text,
            onValueChange = {
                text = it
                importError = null
            },
            label = { Text(name) },
            isError = importError != null,
            supportingText = importError?.let { message ->
                { Text(stringResource(R.string.sound_import_failed, message)) }
            },
            modifier = Modifier.fillMaxWidth(),
        )
        Button(
            onClick = {
                if (confirmation == null) {
                    import()
                } else {
                    confirming = true
                }
            },
            enabled = text.isNotBlank() && !importing && enabled,
        ) { Text(stringResource(R.string.import_)) }
    }
    if (confirming) {
        AlertDialog(
            onDismissRequest = { confirming = false },
            title = { Text(name) },
            text = { Text(confirmation.orEmpty()) },
            confirmButton = {
                TextButton(onClick = {
                    confirming = false
                    import()
                }) {
                    Text(stringResource(R.string.confirm))
                }
            },
            dismissButton = {
                TextButton(onClick = { confirming = false }) { Text(stringResource(R.string.cancel)) }
            },
        )
    }
}
