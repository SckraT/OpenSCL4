package com.oppzippy.openscq30.ui.sound

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.lib.bindings.translateSettingId
import com.oppzippy.openscq30.lib.bindings.translateValue
import com.oppzippy.openscq30.lib.wrapper.QuickPreset
import com.oppzippy.openscq30.lib.wrapper.QuickPresetField
import com.oppzippy.openscq30.lib.wrapper.Setting

/** Old presets can repeat ids shared by categories. Native activation applies the last enabled value. */
internal fun soundPresetFields(quickPreset: QuickPreset): List<QuickPresetField> =
    quickPreset.fields.groupBy { it.settingId }.values.map { fields ->
        fields.lastOrNull { it.isEnabled } ?: fields.last()
    }

/**
 * Whether the device currently has the values of the preset's enabled fields. Several presets can match at once, for
 * example when they enable different subsets of the same settings, so this alone doesn't say which preset is applied.
 */
internal fun isSoundPresetApplied(quickPreset: QuickPreset, settings: Map<String, Setting>): Boolean {
    val enabledFields = soundPresetFields(quickPreset).filter { it.isEnabled }
    return enabledFields.isNotEmpty() && enabledFields.all { field ->
        settings[field.settingId]?.toValue() == field.value
    }
}

/** Whether any preset would change settings that are only available while both earbuds are connected. */
internal fun presetsUseBothEarbudSettings(quickPresets: List<QuickPreset>): Boolean = quickPresets.any { preset ->
    soundPresetFields(preset).any { it.isEnabled && it.settingId in BOTH_EARBUDS_SETTING_IDS }
}

internal enum class SoundPresetStatus {
    /** The preset the user applied last, and the device still has all of its values. */
    Applied,

    /** The device has the preset's values, but it wasn't the last one applied. */
    MatchesCurrent,
    NotApplied,
}

/**
 * Only the last preset the user applied can be [SoundPresetStatus.Applied]. It stops being applied as soon as one of
 * its enabled settings is changed, since then the device no longer has its values.
 */
internal fun soundPresetStatus(
    quickPreset: QuickPreset,
    settings: Map<String, Setting>,
    lastAppliedName: String?,
): SoundPresetStatus = when {
    !isSoundPresetApplied(quickPreset, settings) -> SoundPresetStatus.NotApplied
    quickPreset.name == lastAppliedName -> SoundPresetStatus.Applied
    else -> SoundPresetStatus.MatchesCurrent
}

@Composable
fun SoundPresetsPage(
    quickPresets: List<QuickPreset>,
    settings: List<Pair<String, Setting>>,
    lastAppliedName: String?,
    onActivate: (String) -> Unit,
    onCreate: (String) -> Unit,
    onEdit: (String) -> Unit,
    onDelete: (String) -> Unit,
    notice: (@Composable () -> Unit)? = null,
) {
    val settingsById = remember(settings) { settings.toMap() }
    var showCreate by rememberSaveable { mutableStateOf(false) }
    var deleteName by rememberSaveable { mutableStateOf<String?>(null) }

    if (showCreate) {
        SoundCreatePresetDialog(
            existingNames = quickPresets.map { it.name },
            onDismiss = { showCreate = false },
            onCreate = { name ->
                showCreate = false
                onCreate(name)
            },
        )
    }

    deleteName?.let { name ->
        AlertDialog(
            onDismissRequest = { deleteName = null },
            title = { Text(stringResource(R.string.sound_preset_delete_title)) },
            text = { Text(stringResource(R.string.sound_preset_delete_message, name)) },
            confirmButton = {
                TextButton(onClick = {
                    deleteName = null
                    onDelete(name)
                }) { Text(stringResource(R.string.delete)) }
            },
            dismissButton = {
                TextButton(onClick = { deleteName = null }) { Text(stringResource(R.string.cancel)) }
            },
        )
    }

    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(20.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        item {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(
                    text = stringResource(R.string.sound_preset_heading),
                    style = MaterialTheme.typography.headlineLarge,
                    fontWeight = FontWeight.SemiBold,
                )
                Text(
                    text = stringResource(R.string.sound_preset_intro),
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        if (notice != null) {
            item { notice() }
        }
        if (quickPresets.isEmpty()) {
            item {
                Card(
                    shape = RoundedCornerShape(24.dp),
                    colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.secondaryContainer),
                ) {
                    Column(
                        modifier = Modifier.padding(24.dp),
                        verticalArrangement = Arrangement.spacedBy(16.dp),
                    ) {
                        Icon(painterResource(R.drawable.equalizer_24px), contentDescription = null)
                        Text(
                            stringResource(R.string.sound_preset_empty_title),
                            style = MaterialTheme.typography.titleLarge,
                        )
                        Text(stringResource(R.string.sound_preset_empty_message))
                        Button(onClick = { showCreate = true }) {
                            Text(stringResource(R.string.sound_preset_create))
                        }
                    }
                }
            }
        } else {
            items(quickPresets, key = { it.name }) { preset ->
                SoundPresetCard(
                    preset = preset,
                    settings = settingsById,
                    status = soundPresetStatus(preset, settingsById, lastAppliedName),
                    onActivate = { onActivate(preset.name) },
                    onEdit = { onEdit(preset.name) },
                    onDelete = { deleteName = preset.name },
                )
            }
            item {
                OutlinedButton(onClick = { showCreate = true }, modifier = Modifier.fillMaxWidth()) {
                    Icon(painterResource(R.drawable.add_24px), contentDescription = null)
                    Text(stringResource(R.string.sound_preset_create), modifier = Modifier.padding(start = 8.dp))
                }
            }
        }
    }
}

@Composable
private fun SoundPresetCard(
    preset: QuickPreset,
    settings: Map<String, Setting>,
    status: SoundPresetStatus,
    onActivate: () -> Unit,
    onEdit: () -> Unit,
    onDelete: () -> Unit,
) {
    val enabledFields = remember(preset) { soundPresetFields(preset).filter { it.isEnabled } }
    val applied = status == SoundPresetStatus.Applied
    var showMenu by remember { mutableStateOf(false) }
    val menuDescription = stringResource(R.string.sound_preset_more, preset.name)
    Card(
        modifier = Modifier.fillMaxWidth().testTag("sound_preset_${preset.name}"),
        shape = RoundedCornerShape(24.dp),
        colors = CardDefaults.cardColors(
            containerColor = if (applied) {
                MaterialTheme.colorScheme.secondaryContainer
            } else {
                MaterialTheme.colorScheme.surfaceContainerLow
            },
        ),
    ) {
        Column(Modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(painterResource(R.drawable.equalizer_24px), contentDescription = null)
                Text(
                    preset.name,
                    modifier = Modifier.weight(1f).padding(start = 12.dp),
                    style = MaterialTheme.typography.titleLarge,
                    fontWeight = FontWeight.SemiBold,
                )
                Box {
                    IconButton(onClick = { showMenu = true }) {
                        Text(
                            "⋮",
                            style = MaterialTheme.typography.headlineSmall,
                            modifier = Modifier.clearAndSetSemantics { contentDescription = menuDescription },
                        )
                    }
                    DropdownMenu(expanded = showMenu, onDismissRequest = { showMenu = false }) {
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.delete)) },
                            onClick = {
                                showMenu = false
                                onDelete()
                            },
                        )
                    }
                }
            }
            if (enabledFields.isEmpty()) {
                Text(
                    stringResource(R.string.sound_preset_choose_fields),
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            } else {
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    enabledFields.take(3).forEach { field ->
                        Surface(
                            color = MaterialTheme.colorScheme.surface,
                            shape = RoundedCornerShape(12.dp),
                        ) {
                            Text(
                                stringResource(
                                    R.string.sound_preset_field_summary,
                                    translateSettingId(field.settingId),
                                    translateValue(settings[field.settingId], field.value),
                                ),
                                modifier = Modifier.padding(horizontal = 10.dp, vertical = 7.dp),
                                style = MaterialTheme.typography.labelMedium,
                            )
                        }
                    }
                    if (enabledFields.size > 3) {
                        Text(
                            stringResource(R.string.sound_preset_more_fields, enabledFields.size - 3),
                            modifier = Modifier.padding(8.dp),
                            style = MaterialTheme.typography.labelMedium,
                        )
                    }
                }
            }
            if (status != SoundPresetStatus.NotApplied) {
                Row(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    // Only the check mark says the preset was applied, since other presets can match the device too
                    if (applied) {
                        Icon(painterResource(R.drawable.check_circle_24px), contentDescription = null)
                    }
                    Text(stringResource(R.string.sound_preset_matches), style = MaterialTheme.typography.labelLarge)
                }
            }
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Button(onClick = onActivate, enabled = enabledFields.isNotEmpty() && !applied) {
                    Text(stringResource(if (applied) R.string.sound_preset_applied else R.string.sound_preset_apply))
                }
                TextButton(onClick = onEdit) { Text(stringResource(R.string.sound_preset_edit)) }
            }
        }
    }
}

@Composable
private fun SoundCreatePresetDialog(existingNames: List<String>, onDismiss: () -> Unit, onCreate: (String) -> Unit) {
    var name by rememberSaveable { mutableStateOf("") }
    var replaceName by rememberSaveable { mutableStateOf<String?>(null) }
    val trimmedName = name.trim()
    if (replaceName != null) {
        AlertDialog(
            onDismissRequest = { replaceName = null },
            title = { Text(stringResource(R.string.sound_preset_replace_title)) },
            text = { Text(stringResource(R.string.sound_preset_replace_message, replaceName.orEmpty())) },
            confirmButton = {
                TextButton(onClick = { replaceName?.let(onCreate) }) {
                    Text(stringResource(R.string.sound_preset_replace))
                }
            },
            dismissButton = {
                TextButton(onClick = { replaceName = null }) { Text(stringResource(R.string.cancel)) }
            },
        )
    } else {
        AlertDialog(
            onDismissRequest = onDismiss,
            title = { Text(stringResource(R.string.sound_preset_create)) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                    Text(stringResource(R.string.sound_preset_create_message))
                    OutlinedTextField(
                        value = name,
                        onValueChange = { name = it },
                        singleLine = true,
                        label = { Text(stringResource(R.string.name)) },
                    )
                }
            },
            confirmButton = {
                TextButton(
                    enabled = trimmedName.isNotEmpty(),
                    onClick = {
                        if (trimmedName in existingNames) replaceName = trimmedName else onCreate(trimmedName)
                    },
                ) { Text(stringResource(R.string.create)) }
            },
            dismissButton = {
                TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) }
            },
        )
    }
}

@Composable
fun SoundPresetEditorPage(
    quickPreset: QuickPreset,
    settings: Map<String, Setting>,
    onToggleSetting: (String, String, Boolean) -> Unit,
    onLoadCurrentSettings: () -> Unit,
) {
    val fields = remember(quickPreset) { soundPresetFields(quickPreset) }
    var showLoadConfirmation by rememberSaveable(quickPreset.name) { mutableStateOf(false) }
    if (showLoadConfirmation) {
        AlertDialog(
            onDismissRequest = { showLoadConfirmation = false },
            title = { Text(stringResource(R.string.sound_preset_load_title)) },
            text = { Text(stringResource(R.string.sound_preset_load_message)) },
            confirmButton = {
                TextButton(onClick = {
                    showLoadConfirmation = false
                    onLoadCurrentSettings()
                }) { Text(stringResource(R.string.sound_preset_load)) }
            },
            dismissButton = {
                TextButton(onClick = { showLoadConfirmation = false }) { Text(stringResource(R.string.cancel)) }
            },
        )
    }

    LazyColumn(
        modifier = Modifier.fillMaxSize().testTag("sound_preset_editor"),
        contentPadding = PaddingValues(20.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        item {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(quickPreset.name, style = MaterialTheme.typography.headlineMedium)
                Text(
                    stringResource(R.string.sound_preset_editor_intro),
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                OutlinedButton(onClick = { showLoadConfirmation = true }, modifier = Modifier.fillMaxWidth()) {
                    Text(stringResource(R.string.sound_preset_load_title))
                }
            }
        }
        if (fields.isEmpty()) {
            item { Text(stringResource(R.string.sound_preset_editor_empty)) }
        }
        items(fields, key = { it.settingId }) { field ->
            Card(
                modifier = Modifier.fillMaxWidth().toggleable(
                    value = field.isEnabled,
                    role = Role.Switch,
                    onValueChange = { enabled -> onToggleSetting(quickPreset.name, field.settingId, enabled) },
                ),
                shape = RoundedCornerShape(24.dp),
                colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerLow),
            ) {
                Row(
                    modifier = Modifier.padding(20.dp),
                    horizontalArrangement = Arrangement.spacedBy(16.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        Text(
                            translateSettingId(field.settingId),
                            style = MaterialTheme.typography.titleMedium,
                        )
                        Text(
                            translateValue(settings[field.settingId], field.value),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    Switch(checked = field.isEnabled, onCheckedChange = null)
                }
            }
        }
    }
}
