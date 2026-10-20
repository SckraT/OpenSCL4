@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)

package com.oppzippy.openscq30.ui.sound

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.clickable
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
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Checkbox
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.dp
import androidx.core.content.getSystemService
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.features.equalizer.storage.LegacyEqualizerProfile
import com.oppzippy.openscq30.lib.wrapper.Equalizer
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.lib.wrapper.Value
import com.oppzippy.openscq30.lib.wrapper.toValue
import com.oppzippy.openscq30.ui.devicesettings.composables.ReadOnlyEqualizer
import java.io.ByteArrayOutputStream
import kotlin.math.abs
import kotlin.math.pow
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private const val EXPORT_SELECTION = "exportCustomEqualizerProfiles"
private const val EXPORT_OUTPUT = "exportCustomEqualizerProfilesOutput"
private const val EXPORT_FILE_NAME = "openscq30-equalizer-profiles.json"
private const val MAX_IMPORT_BYTES = 1024 * 1024

/**
 * The saved profiles with their values. The library only gives the values of the profiles selected for export, so
 * every profile is kept selected there and the app picks what to export itself.
 */
@Composable
internal fun rememberSavedProfiles(
    importExportSettings: List<Pair<String, Setting>>,
    setSettings: (List<Pair<String, Value>>) -> Unit,
): Map<String, TransferProfile> {
    val values = importExportSettings.toMap()
    val selection = values[EXPORT_SELECTION] as? Setting.MultiSelectSetting
    val output = (values[EXPORT_OUTPUT] as? Setting.InformationSetting)?.value
    LaunchedEffect(selection?.setting?.options, selection?.values) {
        val all = selection?.setting?.options ?: return@LaunchedEffect
        if (selection.values.toSet() != all.toSet()) setSettings(listOf(EXPORT_SELECTION to all.toValue()))
    }
    return remember(output) { output?.let(::parseProfiles).orEmpty().associateBy { it.name } }
}

/**
 * A profile's curve drawn small beside its name. It only decorates: the name says which profile it is. The scale fits
 * the curve, around 0 dB and at least ±2 dB, since across the device's whole range most profiles would look flat.
 */
@Composable
internal fun ProfileCurve(values: List<Short>, equalizer: Equalizer, modifier: Modifier = Modifier) {
    if (values.size < 2 || equalizer.min >= equalizer.max) return
    val clamped = values.map { it.coerceIn(equalizer.min, equalizer.max) }
    val minimumSpan = 2 * 10.0.pow(equalizer.fractionDigits.toInt()).toInt()
    val span = maxOf(clamped.maxOf { abs(it.toInt()) }, minimumSpan).toShort()
    ReadOnlyEqualizer(
        modifier = modifier.size(width = 72.dp, height = 28.dp).clearAndSetSemantics {},
        color = MaterialTheme.colorScheme.primary,
        bands = equalizer.bandHz,
        values = clamped,
        minValue = (-span).toShort(),
        maxValue = span,
        fractionDigits = equalizer.fractionDigits,
        drawHorizontalGuide = false,
        strokeWidthDp = 1.5.dp,
    )
}

/** Profiles from OpenSCQ30 1.x that haven't been moved to this device's saved profiles yet. */
@Composable
internal fun LegacyProfilesBanner(
    profiles: List<LegacyEqualizerProfile>,
    equalizer: Equalizer,
    onMigrate: (List<LegacyEqualizerProfile>) -> Unit,
) {
    var expanded by rememberSaveable { mutableStateOf(false) }
    Card(
        shape = RoundedCornerShape(24.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.tertiaryContainer),
    ) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Icon(painterResource(R.drawable.update_24px), contentDescription = null)
                Text(
                    pluralStringResource(R.plurals.sound_profile_legacy_found, profiles.size, profiles.size),
                    style = MaterialTheme.typography.titleMedium,
                    modifier = Modifier.weight(1f),
                )
            }
            Text(stringResource(R.string.sound_profile_legacy_description), style = MaterialTheme.typography.bodyMedium)
            if (expanded) {
                profiles.forEach { profile ->
                    Row(
                        Modifier.fillMaxWidth(),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Text(profile.name, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
                        ProfileCurve(profile.toTransferProfile().deviceValues(equalizer.fractionDigits), equalizer)
                        TextButton(onClick = { onMigrate(listOf(profile)) }) { Text(stringResource(R.string.migrate)) }
                    }
                }
            }
            FlowRow(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
                itemVerticalAlignment = Alignment.CenterVertically,
            ) {
                TextButton(onClick = { expanded = !expanded }) {
                    Text(
                        stringResource(
                            if (expanded) R.string.sound_profile_legacy_hide else R.string.sound_profile_legacy_show,
                        ),
                    )
                }
                Button(onClick = { onMigrate(profiles) }) { Text(stringResource(R.string.sound_profile_migrate_all)) }
            }
        }
    }
}

/** One saved profile: tapping it applies it, and its menu shares or deletes it. */
@Composable
internal fun SavedProfileRow(
    label: String,
    inEffect: Boolean,
    curve: List<Short>?,
    equalizer: Equalizer?,
    onSelect: () -> Unit,
    onShare: (() -> Unit)?,
    onDelete: () -> Unit,
) {
    var menu by remember { mutableStateOf(false) }
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        TextButton(modifier = Modifier.weight(1f).heightIn(min = 48.dp), onClick = onSelect) {
            if (inEffect) {
                Icon(painterResource(R.drawable.check_24px), contentDescription = stringResource(R.string.in_effect))
            }
            Text(label, modifier = Modifier.weight(1f).padding(start = if (inEffect) 8.dp else 0.dp))
        }
        if (curve != null && equalizer != null) ProfileCurve(curve, equalizer)
        Box {
            IconButton(onClick = { menu = true }) {
                Icon(
                    painterResource(R.drawable.sound_more),
                    contentDescription = stringResource(R.string.sound_profile_more_named, label),
                )
            }
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                if (onShare != null) {
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.sound_profile_share)) },
                        leadingIcon = { Icon(painterResource(R.drawable.share_24px), contentDescription = null) },
                        onClick = {
                            menu = false
                            onShare()
                        },
                    )
                }
                DropdownMenuItem(
                    text = { Text(stringResource(R.string.delete)) },
                    leadingIcon = { Icon(painterResource(R.drawable.delete_24px), contentDescription = null) },
                    onClick = {
                        menu = false
                        onDelete()
                    },
                )
            }
        }
    }
}

internal fun shareProfiles(context: Context, profiles: List<TransferProfile>) {
    val send = Intent(Intent.ACTION_SEND)
        .setType("text/plain")
        .putExtra(Intent.EXTRA_TEXT, profilesJson(profiles))
        .putExtra(Intent.EXTRA_TITLE, EXPORT_FILE_NAME)
    context.startActivity(Intent.createChooser(send, context.getString(R.string.sound_profile_share)))
}

private data class TransferMessage(val text: String, val isError: Boolean)

/** Export chosen profiles by sharing, saving or copying them, and import a file with a look at it first. */
@Composable
internal fun ProfileTransferSection(
    profileNames: List<String>,
    savedProfiles: Map<String, TransferProfile>,
    equalizer: Equalizer?,
    onImport: suspend (String) -> Result<Unit>,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var expanded by rememberSaveable { mutableStateOf(false) }
    var chosen by rememberSaveable { mutableStateOf(emptyList<String>()) }
    var exportMessage by remember { mutableStateOf<TransferMessage?>(null) }
    var importMessage by remember { mutableStateOf<TransferMessage?>(null) }
    var previewText by rememberSaveable { mutableStateOf<String?>(null) }
    var pendingFile by rememberSaveable { mutableStateOf<String?>(null) }
    val selected = profileNames.filter { it in chosen }.mapNotNull { savedProfiles[it] }

    fun preview(text: String?) {
        val profiles = text?.let(::parseProfiles)
        if (profiles.isNullOrEmpty()) {
            importMessage = TransferMessage(context.getString(R.string.sound_profile_not_profiles), isError = true)
        } else {
            importMessage = null
            previewText = text
        }
    }

    val createDocument = ActivityResultContracts.CreateDocument("application/json")
    val saveLauncher = rememberLauncherForActivityResult(createDocument) { uri ->
        val json = pendingFile
        pendingFile = null
        if (uri != null && json != null) {
            scope.launch {
                val saved = withContext(Dispatchers.IO) { writeText(context, uri, json) }
                val text = if (saved) R.string.sound_profile_file_saved else R.string.sound_profile_file_save_failed
                exportMessage = TransferMessage(context.getString(text), isError = !saved)
            }
        }
    }
    val openLauncher = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) {
            scope.launch {
                when (val read = withContext(Dispatchers.IO) { readText(context, uri) }) {
                    is ReadResult.Text -> preview(read.text)

                    ReadResult.TooLarge ->
                        importMessage =
                            TransferMessage(context.getString(R.string.sound_profile_file_too_large), isError = true)

                    ReadResult.Failed ->
                        importMessage =
                            TransferMessage(context.getString(R.string.sound_profile_file_read_failed), isError = true)
                }
            }
        }
    }

    Card(
        shape = RoundedCornerShape(24.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerLow),
    ) {
        Row(
            Modifier.fillMaxWidth()
                .clickable(role = Role.Button) { expanded = !expanded }
                .padding(16.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(
                    stringResource(R.string.sound_profile_transfer_title),
                    style = MaterialTheme.typography.titleMedium,
                )
                Text(
                    stringResource(R.string.sound_profile_transfer_description),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Icon(
                painterResource(R.drawable.expand_more_24px),
                contentDescription = null,
                modifier = Modifier.rotate(if (expanded) 180f else 0f),
            )
        }
        AnimatedVisibility(expanded) {
            Column(
                Modifier.padding(start = 16.dp, end = 16.dp, bottom = 16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Text(stringResource(R.string.sound_profile_export_title), style = MaterialTheme.typography.titleSmall)
                if (profileNames.isEmpty()) {
                    Text(
                        stringResource(R.string.sound_profile_export_empty),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                } else {
                    CheckRow(
                        label = stringResource(R.string.sound_profile_select_all),
                        checked = profileNames.all { it in chosen },
                        onCheckedChange = { chosen = if (it) profileNames else emptyList() },
                    )
                    profileNames.forEach { name ->
                        CheckRow(
                            label = name,
                            checked = name in chosen,
                            onCheckedChange = { chosen = if (it) chosen + name else chosen - name },
                        )
                    }
                    FlowRow(
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        Button(enabled = selected.isNotEmpty(), onClick = { shareProfiles(context, selected) }) {
                            ActionLabel(R.drawable.share_24px, R.string.sound_profile_share)
                        }
                        OutlinedButton(
                            enabled = selected.isNotEmpty(),
                            onClick = {
                                pendingFile = profilesJson(selected)
                                saveLauncher.launch(EXPORT_FILE_NAME)
                            },
                        ) { ActionLabel(R.drawable.file_download_24px, R.string.sound_profile_save_file) }
                        OutlinedButton(
                            enabled = selected.isNotEmpty(),
                            onClick = {
                                val label = context.getString(R.string.sound_profile_transfer_title)
                                context.getSystemService<ClipboardManager>()
                                    ?.setPrimaryClip(ClipData.newPlainText(label, profilesJson(selected)))
                                // Android 13 and later show what was copied themselves
                                if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
                                    val copied = context.getString(R.string.sound_profile_copied)
                                    exportMessage = TransferMessage(copied, isError = false)
                                }
                            },
                        ) { ActionLabel(R.drawable.content_copy_24px, R.string.sound_profile_copy) }
                    }
                    exportMessage?.let { MessageText(it) }
                }

                HorizontalDivider(Modifier.padding(vertical = 8.dp))
                Text(stringResource(R.string.sound_profile_import_title), style = MaterialTheme.typography.titleSmall)
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    OutlinedButton(onClick = {
                        val clip = context.getSystemService<ClipboardManager>()?.primaryClip
                        val text = clip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(context)?.toString()
                        if (text.isNullOrBlank()) {
                            val text = context.getString(R.string.sound_profile_clipboard_empty)
                            importMessage = TransferMessage(text, isError = true)
                        } else {
                            preview(text)
                        }
                    }) { ActionLabel(R.drawable.content_paste_24px, R.string.sound_profile_paste) }
                    OutlinedButton(onClick = { openLauncher.launch(arrayOf("application/json", "text/*")) }) {
                        ActionLabel(R.drawable.folder_open_24px, R.string.sound_profile_open_file)
                    }
                }
                importMessage?.let { MessageText(it) }
            }
        }
    }

    previewText?.let { text ->
        ImportPreviewDialog(
            items = previewImport(parseProfiles(text).orEmpty(), profileNames, equalizer),
            equalizer = equalizer,
            onDismiss = { previewText = null },
            onImport = { count ->
                scope.launch {
                    importMessage = onImport(text).fold(
                        onSuccess = {
                            TransferMessage(
                                context.resources.getQuantityString(R.plurals.sound_profile_imported, count, count),
                                isError = false,
                            )
                        },
                        onFailure = {
                            TransferMessage(
                                context.getString(R.string.sound_import_failed, it.message ?: it.toString()),
                                isError = true,
                            )
                        },
                    )
                    previewText = null
                }
            },
        )
    }
}

@Composable
private fun ImportPreviewDialog(
    items: List<ImportPreviewItem>,
    equalizer: Equalizer?,
    onDismiss: () -> Unit,
    onImport: (Int) -> Unit,
) {
    var importing by remember { mutableStateOf(false) }
    val canImport = items.isNotEmpty() && items.all { it.isValid }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.sound_profile_import_preview_title)) },
        text = {
            Column(
                Modifier.heightIn(max = 360.dp).verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                items.forEach { item ->
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Column(Modifier.weight(1f)) {
                            Text(item.profile.name, style = MaterialTheme.typography.bodyLarge)
                            Text(
                                when (item.status) {
                                    ImportStatus.New -> stringResource(R.string.sound_profile_import_new)

                                    ImportStatus.Replaces -> stringResource(R.string.sound_profile_import_replaces)

                                    ImportStatus.WrongBandCount -> stringResource(
                                        R.string.sound_profile_import_wrong_bands,
                                        item.profile.volumeAdjustments.size,
                                        equalizer?.bandHz?.size ?: 0,
                                    )

                                    ImportStatus.OutOfRange ->
                                        stringResource(R.string.sound_profile_import_out_of_range)
                                },
                                style = MaterialTheme.typography.bodySmall,
                                color = when (item.status) {
                                    ImportStatus.New -> MaterialTheme.colorScheme.onSurfaceVariant
                                    ImportStatus.Replaces -> MaterialTheme.colorScheme.tertiary
                                    else -> MaterialTheme.colorScheme.error
                                },
                            )
                        }
                        if (item.isValid && equalizer != null) {
                            ProfileCurve(item.profile.deviceValues(equalizer.fractionDigits), equalizer)
                        }
                    }
                }
                if (!canImport) {
                    Text(
                        stringResource(R.string.sound_profile_import_blocked),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.error,
                    )
                }
            }
        },
        confirmButton = {
            TextButton(
                enabled = canImport && !importing,
                onClick = {
                    importing = true
                    onImport(items.size)
                },
            ) { Text(pluralStringResource(R.plurals.sound_profile_import_button, items.size, items.size)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

@Composable
private fun CheckRow(label: String, checked: Boolean, onCheckedChange: (Boolean) -> Unit) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 48.dp)
            .toggleable(value = checked, role = Role.Checkbox, onValueChange = onCheckedChange),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Checkbox(checked = checked, onCheckedChange = null)
        Text(label, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
    }
}

@Composable
private fun ActionLabel(icon: Int, label: Int) {
    Icon(painterResource(icon), contentDescription = null, modifier = Modifier.size(18.dp))
    Text(stringResource(label), modifier = Modifier.padding(start = 8.dp))
}

@Composable
private fun MessageText(message: TransferMessage) {
    Text(
        message.text,
        style = MaterialTheme.typography.bodyMedium,
        color = if (message.isError) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary,
    )
}

private sealed interface ReadResult {
    data class Text(val text: String) : ReadResult
    data object TooLarge : ReadResult
    data object Failed : ReadResult
}

private fun readText(context: Context, uri: Uri): ReadResult = try {
    context.contentResolver.openInputStream(uri)?.use { stream ->
        val out = ByteArrayOutputStream()
        val buffer = ByteArray(8192)
        var tooLarge = false
        while (true) {
            val read = stream.read(buffer)
            if (read < 0) break
            if (out.size() + read > MAX_IMPORT_BYTES) {
                tooLarge = true
                break
            }
            out.write(buffer, 0, read)
        }
        if (tooLarge) ReadResult.TooLarge else ReadResult.Text(out.toString(Charsets.UTF_8.name()))
    } ?: ReadResult.Failed
} catch (_: java.io.IOException) {
    ReadResult.Failed
} catch (_: SecurityException) {
    ReadResult.Failed
}

private fun writeText(context: Context, uri: Uri, text: String): Boolean = try {
    context.contentResolver.openOutputStream(uri, "wt")?.use { it.write(text.toByteArray(Charsets.UTF_8)) } != null
} catch (_: java.io.IOException) {
    false
} catch (_: SecurityException) {
    false
}
