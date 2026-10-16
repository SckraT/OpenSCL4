@file:OptIn(
    androidx.compose.material3.ExperimentalMaterial3Api::class,
    androidx.compose.foundation.layout.ExperimentalLayoutApi::class,
)

package com.oppzippy.openscq30.ui.sound

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.layout
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.dp
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.lib.wrapper.Equalizer
import com.oppzippy.openscq30.lib.wrapper.ModifiableSelectCommandInner
import com.oppzippy.openscq30.lib.wrapper.Select
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.lib.wrapper.Value
import com.oppzippy.openscq30.lib.wrapper.toValue
import java.math.BigDecimal
import java.math.RoundingMode
import kotlin.math.roundToInt

private const val PRESET_PROFILE = "presetEqualizerProfile"
private const val CUSTOM_PROFILE = "customEqualizerProfile"
private const val VOLUME_ADJUSTMENTS = "volumeAdjustments"

/** Device values remain authoritative; drag previews are committed as one vector on release. */
@Composable
fun SoundEqualizerPage(
    settings: List<Pair<String, Setting>>,
    setSettings: (List<Pair<String, Value>>) -> Unit,
    onImportExport: () -> Unit,
) {
    val settingsById = settings.toMap()
    val equalizer = settingsById[VOLUME_ADJUSTMENTS] as? Setting.EqualizerSetting
    val preset = settingsById[PRESET_PROFILE]
    val presetSelect = when (preset) {
        is Setting.PresetEqualizerProfileSelect -> preset.select
        is Setting.OptionalSelectSetting -> preset.setting
        is Setting.SelectSetting -> preset.setting
        else -> null
    }
    val presetValue = when (preset) {
        is Setting.PresetEqualizerProfileSelect -> preset.value
        is Setting.OptionalSelectSetting -> preset.value
        is Setting.SelectSetting -> preset.value
        else -> null
    }
    val custom = settingsById[CUSTOM_PROFILE] as? Setting.ModifiableSelectSetting
    var editedBands by remember(equalizer?.setting) { mutableStateOf<Map<Int, Short>>(emptyMap()) }
    var showNumericEntry by remember { mutableStateOf(false) }
    var showSaveDialog by remember { mutableStateOf(false) }
    var deleteProfile by remember { mutableStateOf<String?>(null) }

    Column(
        modifier = Modifier.verticalScroll(rememberScrollState()).padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text(stringResource(R.string.sound_profile_eq_heading), style = MaterialTheme.typography.headlineLarge)
        Text(
            stringResource(R.string.sound_profile_eq_description),
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )

        if (presetSelect != null) {
            Row(
                modifier = Modifier.horizontalScroll(rememberScrollState()),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                presetSelect.options.forEachIndexed { index, option ->
                    FilterChip(
                        selected = presetValue == option,
                        onClick = { setSettings(listOf(PRESET_PROFILE to option.toValue())) },
                        label = { Text(presetSelect.localizedLabel(index)) },
                        leadingIcon = if (presetValue == option) {
                            { Icon(painterResource(R.drawable.check_24px), contentDescription = null) }
                        } else {
                            null
                        },
                    )
                }
            }
        }

        if (equalizer != null && equalizer.value.isNotEmpty() &&
            equalizer.setting.bandHz.size == equalizer.value.size && equalizer.setting.min < equalizer.setting.max
        ) {
            val bandScrollState = rememberScrollState()
            val displayedValues = equalizer.value.mapIndexed { index, value ->
                editedBands[index] ?: value
            }
            val commitEdit: () -> Unit = {
                if (editedBands.isNotEmpty()) {
                    // Merge with the latest device vector, preserving concurrent changes to untouched bands.
                    val values = equalizer.value.mapIndexed { index, old -> editedBands[index] ?: old }
                    setSettings(listOf(VOLUME_ADJUSTMENTS to values.toValue()))
                }
                editedBands = emptyMap()
            }

            Card(
                shape = RoundedCornerShape(24.dp),
                colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerLowest),
            ) {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                        Text(
                            stringResource(R.string.sound_profile_band_count, equalizer.setting.bandHz.size),
                            style = MaterialTheme.typography.labelLarge,
                            modifier = Modifier.weight(1f),
                        )
                        Text(stringResource(R.string.sound_profile_db), style = MaterialTheme.typography.labelMedium)
                    }
                    EqualizerChart(
                        values = displayedValues,
                        equalizer = equalizer.setting,
                        enabled = !equalizer.readOnly,
                        onEdit = { index, value -> editedBands = editedBands + (index to value) },
                        onCommit = commitEdit,
                        onCancel = { editedBands = emptyMap() },
                    )
                    Row(
                        modifier = Modifier.fillMaxWidth().horizontalScroll(bandScrollState),
                        horizontalArrangement = Arrangement.SpaceEvenly,
                    ) {
                        equalizer.setting.bandHz.forEachIndexed { index, hz ->
                            Column(
                                modifier = Modifier.width(56.dp),
                                horizontalAlignment = Alignment.CenterHorizontally,
                                verticalArrangement = Arrangement.spacedBy(4.dp),
                            ) {
                                val frequency = frequencyLabel(hz)
                                Text(
                                    formatEqualizerValue(displayedValues[index], equalizer.setting.fractionDigits),
                                    style = MaterialTheme.typography.labelLarge,
                                    color = MaterialTheme.colorScheme.primary,
                                )
                                BandSlider(
                                    value = displayedValues[index],
                                    equalizer = equalizer.setting,
                                    enabled = !equalizer.readOnly,
                                    label = frequency,
                                    onValueChange = { editedBands = editedBands + (index to it) },
                                    onValueChangeFinished = commitEdit,
                                )
                                Text(frequency, style = MaterialTheme.typography.labelSmall)
                            }
                        }
                    }
                    if (bandScrollState.canScrollForward || bandScrollState.canScrollBackward) {
                        Text(
                            stringResource(R.string.sound_profile_scroll_bands),
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    } else {
                        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                            Text(
                                stringResource(R.string.sound_profile_low_frequencies),
                                style = MaterialTheme.typography.labelSmall,
                            )
                            Text(
                                stringResource(R.string.sound_profile_high_frequencies),
                                style = MaterialTheme.typography.labelSmall,
                            )
                        }
                    }
                }
            }

            if (equalizer.readOnly) {
                Text(
                    stringResource(R.string.sound_profile_read_only),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            } else {
                FlowRow(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    OutlinedButton(onClick = { showNumericEntry = true }) {
                        Text(stringResource(R.string.sound_profile_enter_values))
                    }
                    OutlinedButton(
                        onClick = {
                            val zero = 0.coerceIn(
                                equalizer.setting.min.toInt(),
                                equalizer.setting.max.toInt(),
                            ).toShort()
                            setSettings(listOf(VOLUME_ADJUSTMENTS to List(equalizer.value.size) { zero }.toValue()))
                        },
                    ) { Text(stringResource(R.string.sound_profile_reset)) }
                }
            }

            if (settingsById["spatialAudio"] != null) {
                Card(
                    shape = RoundedCornerShape(24.dp),
                    colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh),
                ) {
                    Row(Modifier.padding(16.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                        Icon(painterResource(R.drawable.info_24px), contentDescription = null)
                        Text(
                            stringResource(R.string.sound_profile_spatial_warning),
                            style = MaterialTheme.typography.bodyMedium,
                        )
                    }
                }
            }

            if (custom != null) {
                Button(
                    modifier = Modifier.fillMaxWidth().heightIn(min = 52.dp),
                    onClick = { showSaveDialog = true },
                ) { Text(stringResource(R.string.sound_profile_save)) }
            }
        } else {
            Text(stringResource(R.string.sound_profile_eq_unavailable))
        }

        if (custom != null && custom.setting.options.isNotEmpty()) {
            Text(stringResource(R.string.sound_profile_saved_profiles), style = MaterialTheme.typography.titleLarge)
            Card(shape = RoundedCornerShape(24.dp)) {
                Column(Modifier.padding(12.dp)) {
                    custom.setting.options.forEachIndexed { index, option ->
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            TextButton(
                                modifier = Modifier.weight(1f).heightIn(min = 48.dp),
                                onClick = { setSettings(listOf(CUSTOM_PROFILE to option.toValue())) },
                            ) {
                                if (custom.value == option && presetValue == null) {
                                    Icon(
                                        painterResource(R.drawable.check_24px),
                                        contentDescription = stringResource(R.string.in_effect),
                                    )
                                }
                                Text(custom.setting.localizedLabel(index))
                            }
                            IconButton(onClick = { deleteProfile = option }) {
                                Icon(
                                    painterResource(R.drawable.remove_24px),
                                    contentDescription = stringResource(R.string.sound_profile_delete_named, option),
                                )
                            }
                        }
                    }
                }
            }
        }
        OutlinedButton(modifier = Modifier.fillMaxWidth().heightIn(min = 52.dp), onClick = onImportExport) {
            Text(stringResource(R.string.sound_profile_import_export))
        }
    }

    if (showNumericEntry && equalizer != null && !equalizer.readOnly) {
        NumericEqualizerSheet(
            equalizer = equalizer,
            onDismiss = { showNumericEntry = false },
            onApply = { values ->
                setSettings(listOf(VOLUME_ADJUSTMENTS to values.toValue()))
                showNumericEntry = false
            },
        )
    }
    if (showSaveDialog && custom != null) {
        SaveEqualizerDialog(
            existingNames = custom.setting.options,
            onDismiss = { showSaveDialog = false },
            onSave = { name ->
                setSettings(listOf(CUSTOM_PROFILE to ModifiableSelectCommandInner.Add(name).toValue()))
                showSaveDialog = false
            },
        )
    }
    deleteProfile?.let { name ->
        AlertDialog(
            onDismissRequest = { deleteProfile = null },
            title = { Text(stringResource(R.string.sound_profile_delete_title)) },
            text = { Text(stringResource(R.string.delete_confirm, name)) },
            confirmButton = {
                TextButton(
                    onClick = {
                        setSettings(listOf(CUSTOM_PROFILE to ModifiableSelectCommandInner.Remove(name).toValue()))
                        deleteProfile = null
                    },
                ) { Text(stringResource(R.string.delete)) }
            },
            dismissButton = {
                TextButton(onClick = { deleteProfile = null }) { Text(stringResource(R.string.cancel)) }
            },
        )
    }
}

@Composable
private fun BandSlider(
    value: Short,
    equalizer: Equalizer,
    enabled: Boolean,
    label: String,
    onValueChange: (Short) -> Unit,
    onValueChangeFinished: () -> Unit,
) {
    val description = stringResource(
        R.string.sound_profile_band_accessibility,
        label,
        formatEqualizerValue(value, equalizer.fractionDigits),
    )
    Slider(
        value = value.toFloat(),
        onValueChange = { onValueChange(it.roundToInt().toShort()) },
        onValueChangeFinished = onValueChangeFinished,
        steps = (equalizer.max.toInt() - equalizer.min.toInt() - 1).coerceAtLeast(0),
        valueRange = equalizer.min.toFloat()..equalizer.max.toFloat(),
        enabled = enabled,
        modifier = Modifier.width(48.dp).height(168.dp)
            .semantics { contentDescription = description }
            .layout { measurable, constraints ->
                // Measure the public horizontal control in landscape, then expose its
                // rotated 48 dp touch target to the surrounding band column.
                val placeable = measurable.measure(Constraints.fixed(constraints.maxHeight, constraints.maxWidth))
                layout(placeable.height, placeable.width) {
                    placeable.place(
                        x = (placeable.height - placeable.width) / 2,
                        y = (placeable.width - placeable.height) / 2,
                    )
                }
            }
            .rotate(-90f),
    )
}

@Composable
private fun EqualizerChart(
    values: List<Short>,
    equalizer: Equalizer,
    enabled: Boolean,
    onEdit: (Int, Short) -> Unit,
    onCommit: () -> Unit,
    onCancel: () -> Unit,
) {
    val primary = MaterialTheme.colorScheme.primary
    val guide = MaterialTheme.colorScheme.outlineVariant
    val currentEdit by rememberUpdatedState(onEdit)
    val currentCommit by rememberUpdatedState(onCommit)
    val currentCancel by rememberUpdatedState(onCancel)
    val description = stringResource(R.string.sound_profile_chart_description)
    Canvas(
        modifier = Modifier.fillMaxWidth().height(96.dp).semantics { contentDescription = description }
            .pointerInput(equalizer, enabled) {
                if (enabled) {
                    fun edit(position: Offset) {
                        val index = ((position.x / size.width) * (values.size - 1)).roundToInt().coerceIn(
                            values.indices,
                        )
                        val ratio = (1f - position.y / size.height).coerceIn(0f, 1f)
                        val value = (equalizer.min + ratio * (equalizer.max - equalizer.min)).roundToInt().toShort()
                        currentEdit(index, value)
                    }
                    detectDragGestures(
                        onDragStart = ::edit,
                        onDragEnd = { currentCommit() },
                        onDragCancel = { currentCancel() },
                        onDrag = { change, _ ->
                            change.consume()
                            edit(change.position)
                        },
                    )
                }
            },
    ) {
        val range = (equalizer.max - equalizer.min).toFloat()
        val inset = 5.dp.toPx()
        fun point(index: Int, value: Short): Offset = Offset(
            inset + (size.width - inset * 2) * index / (values.size - 1).coerceAtLeast(1),
            inset + (size.height - inset * 2) * (1f - (value - equalizer.min) / range),
        )
        val zeroY = point(0, 0).y.coerceIn(inset, size.height - inset)
        drawLine(guide, Offset(inset, zeroY), Offset(size.width - inset, zeroY), strokeWidth = 1.dp.toPx())
        val points = values.mapIndexed(::point)
        val path = Path().apply {
            points.forEachIndexed { index, point ->
                if (index ==
                    0
                ) {
                    moveTo(point.x, point.y)
                } else {
                    lineTo(point.x, point.y)
                }
            }
        }
        drawPath(path, primary, style = Stroke(width = 2.5.dp.toPx()))
        points.forEach { drawCircle(primary, 3.dp.toPx(), it) }
    }
}

@Composable
private fun NumericEqualizerSheet(
    equalizer: Setting.EqualizerSetting,
    onDismiss: () -> Unit,
    onApply: (List<Short>) -> Unit,
) {
    var values by remember {
        mutableStateOf(equalizer.value.map { formatEqualizerValue(it, equalizer.setting.fractionDigits) })
    }
    val parsed = values.map { parseEqualizerValue(it, equalizer.setting) }
    val rangeText = stringResource(
        R.string.sound_profile_numeric_range,
        formatEqualizerValue(equalizer.setting.min, equalizer.setting.fractionDigits),
        formatEqualizerValue(equalizer.setting.max, equalizer.setting.fractionDigits),
    )
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
    ) {
        Column(
            modifier = Modifier.imePadding().navigationBarsPadding().verticalScroll(
                rememberScrollState(),
            ).padding(20.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text(stringResource(R.string.sound_profile_precise_title), style = MaterialTheme.typography.headlineSmall)
            Text(
                rangeText,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(12.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                equalizer.setting.bandHz.forEachIndexed { index, hz ->
                    val signDescription = stringResource(R.string.sound_profile_change_sign, frequencyLabel(hz))
                    OutlinedTextField(
                        modifier = Modifier.width(152.dp),
                        value = values[index],
                        onValueChange = { text ->
                            values = values.mapIndexed { i, old ->
                                if (i ==
                                    index
                                ) {
                                    text
                                } else {
                                    old
                                }
                            }
                        },
                        label = { Text(frequencyLabel(hz)) },
                        singleLine = true,
                        isError = parsed[index] == null,
                        supportingText = if (parsed[index] == null) {
                            { Text(stringResource(R.string.sound_profile_invalid_value)) }
                        } else {
                            null
                        },
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                        trailingIcon = if (equalizer.setting.min < 0) {
                            {
                                // Decimal keyboards do not consistently expose a minus key.
                                IconButton(onClick = {
                                    values = values.mapIndexed { i, old ->
                                        if (i != index) {
                                            old
                                        } else if (old.startsWith('-')) {
                                            old.drop(1)
                                        } else {
                                            "-$old"
                                        }
                                    }
                                }) {
                                    Text("±", modifier = Modifier.semantics { contentDescription = signDescription })
                                }
                            }
                        } else {
                            null
                        },
                    )
                }
            }
            Button(
                modifier = Modifier.fillMaxWidth().heightIn(min = 52.dp),
                enabled = parsed.all { it != null },
                onClick = { if (parsed.all { it != null }) onApply(parsed.filterNotNull()) },
            ) { Text(stringResource(R.string.sound_profile_apply)) }
            TextButton(modifier = Modifier.fillMaxWidth(), onClick = onDismiss) {
                Text(stringResource(R.string.cancel))
            }
        }
    }
}

@Composable
private fun SaveEqualizerDialog(existingNames: List<String>, onDismiss: () -> Unit, onSave: (String) -> Unit) {
    var name by remember { mutableStateOf("") }
    var confirmOverwrite by remember { mutableStateOf(false) }
    val trimmedName = name.trim()
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.sound_profile_save)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(stringResource(R.string.sound_profile_save_description))
                OutlinedTextField(
                    value = name,
                    onValueChange = {
                        name = it
                        confirmOverwrite = false
                    },
                    label = { Text(stringResource(R.string.name)) },
                    singleLine = true,
                )
                if (confirmOverwrite) {
                    Text(
                        stringResource(R.string.sound_profile_overwrite_warning, trimmedName),
                        color = MaterialTheme.colorScheme.error,
                    )
                }
            }
        },
        confirmButton = {
            TextButton(
                enabled = trimmedName.isNotEmpty(),
                onClick = {
                    if (trimmedName in existingNames &&
                        !confirmOverwrite
                    ) {
                        confirmOverwrite = true
                    } else {
                        onSave(trimmedName)
                    }
                },
            ) {
                Text(
                    stringResource(
                        if (confirmOverwrite) R.string.sound_profile_replace else R.string.sound_profile_save_short,
                    ),
                )
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

private fun Select.localizedLabel(index: Int): String = localizedOptions.getOrNull(index) ?: options[index]

@Composable
private fun frequencyLabel(hz: UShort): String = if (hz.toInt() < 1000) {
    stringResource(R.string.hz, hz.toInt())
} else {
    stringResource(R.string.khz, BigDecimal(hz.toInt()).scaleByPowerOfTen(-3).stripTrailingZeros().toPlainString())
}

internal fun formatEqualizerValue(value: Short, fractionDigits: Short): String =
    BigDecimal(value.toInt()).scaleByPowerOfTen(-fractionDigits.toInt()).stripTrailingZeros().toPlainString()

/** Reject overflow, out-of-range values, and precision the device cannot represent. */
internal fun parseEqualizerValue(text: String, equalizer: Equalizer): Short? = try {
    val number = BigDecimal(text.trim().replace(',', '.'))
        .scaleByPowerOfTen(equalizer.fractionDigits.toInt()).setScale(0, RoundingMode.UNNECESSARY).intValueExact()
    if (number in equalizer.min.toInt()..equalizer.max.toInt()) number.toShort() else null
} catch (_: NumberFormatException) {
    null
} catch (_: ArithmeticException) {
    null
}
