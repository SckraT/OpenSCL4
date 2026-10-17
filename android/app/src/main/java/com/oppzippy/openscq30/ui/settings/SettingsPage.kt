package com.oppzippy.openscq30.ui.settings

import android.appwidget.AppWidgetManager
import android.content.ActivityNotFoundException
import android.content.ComponentName
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings as AndroidSettings
import androidx.annotation.DrawableRes
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.features.preferences.EarbudsColor
import com.oppzippy.openscq30.features.preferences.SoundModesView
import com.oppzippy.openscq30.ui.theme.ThemeType
import com.oppzippy.openscq30.widget.SettingWidgetReceiver

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsPage(onBackClick: () -> Unit, embedded: Boolean = false, onNotificationSettings: (() -> Unit)? = null) {
    if (embedded) {
        SettingsContent(onNotificationSettings = onNotificationSettings)
    } else {
        Scaffold(
            containerColor = MaterialTheme.colorScheme.background,
            topBar = {
                TopAppBar(
                    title = { Text(stringResource(R.string.settings), maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    colors = TopAppBarDefaults.topAppBarColors(containerColor = MaterialTheme.colorScheme.background),
                    navigationIcon = {
                        IconButton(onClick = onBackClick) {
                            Icon(
                                painterResource(R.drawable.arrow_back_24px),
                                contentDescription = stringResource(R.string.back),
                            )
                        }
                    },
                )
            },
        ) { innerPadding ->
            Box(Modifier.padding(innerPadding)) {
                SettingsContent(onNotificationSettings = onNotificationSettings)
            }
        }
    }
}

@Composable
private fun SettingsContent(onNotificationSettings: (() -> Unit)?, viewModel: SettingsViewModel = hiltViewModel()) {
    val autoConnect by viewModel.autoConnect.collectAsState()
    val theme by viewModel.theme.collectAsState()
    val dynamicColor by viewModel.dynamicColorEnabled.collectAsState()
    val earbudsColor by viewModel.earbudsColor.collectAsState()
    val soundModesView by viewModel.soundModesView.collectAsState()
    val showOneEarbudBadge by viewModel.showOneEarbudBadge.collectAsState()
    val context = LocalContext.current
    val locale = LocalConfiguration.current.locales[0]
    var showWidgetHelp by remember { mutableStateOf(false) }
    if (showWidgetHelp) {
        AlertDialog(
            onDismissRequest = { showWidgetHelp = false },
            title = { Text(stringResource(R.string.sound_widget)) },
            text = { Text(stringResource(R.string.sound_widget_help)) },
            confirmButton = {
                TextButton(onClick = { showWidgetHelp = false }) { Text(stringResource(R.string.close)) }
            },
        )
    }
    LazyColumn(
        modifier = Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background),
        contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = 20.dp, bottom = 32.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        item {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(stringResource(R.string.sound_settings_headline), style = MaterialTheme.typography.headlineMedium)
                Text(
                    stringResource(R.string.sound_settings_description),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        item {
            Surface(
                shape = MaterialTheme.shapes.medium,
                color = MaterialTheme.colorScheme.primaryContainer,
                contentColor = MaterialTheme.colorScheme.onPrimaryContainer,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Row(
                    Modifier.padding(20.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(16.dp),
                ) {
                    Box(
                        Modifier.size(60.dp).background(MaterialTheme.colorScheme.primary, CircleShape),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            painterResource(R.drawable.light_mode_24px),
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.onPrimary,
                            modifier = Modifier.size(28.dp),
                        )
                    }
                    Text(stringResource(R.string.sound_your_color), style = MaterialTheme.typography.bodyLarge)
                }
            }
        }
        item {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                SettingsSectionTitle(stringResource(R.string.appearance))
                ThemeSelection(theme = theme, onThemeChange = viewModel::setTheme)
                EarbudsColorSelection(color = earbudsColor, onColorChange = viewModel::setEarbudsColor)
                SoundModesViewSelection(view = soundModesView, onViewChange = viewModel::setSoundModesView)
                SettingsGroup {
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                        SettingsToggle(
                            title = stringResource(R.string.sound_wallpaper_colors),
                            description = stringResource(R.string.sound_wallpaper_colors_description),
                            icon = R.drawable.light_mode_24px,
                            checked = dynamicColor,
                            onCheckedChange = viewModel::setDynamicColor,
                        )
                        SettingsDivider()
                    }
                    SettingsToggle(
                        title = stringResource(R.string.sound_one_earbud_badge_setting),
                        description = stringResource(R.string.sound_one_earbud_badge_setting_description),
                        icon = R.drawable.earbuds_2_24px,
                        checked = showOneEarbudBadge,
                        onCheckedChange = viewModel::setShowOneEarbudBadge,
                    )
                    SettingsDivider()
                    SettingsAction(
                        title = stringResource(R.string.sound_app_language),
                        description = locale.getDisplayName(locale).replaceFirstChar { it.titlecase(locale) },
                        icon = R.drawable.devices_other_24px,
                        onClick = {
                            val detailsIntent = Intent(
                                AndroidSettings.ACTION_APPLICATION_DETAILS_SETTINGS,
                                Uri.fromParts("package", context.packageName, null),
                            )
                            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                                try {
                                    context.startActivity(
                                        Intent(
                                            AndroidSettings.ACTION_APP_LOCALE_SETTINGS,
                                            Uri.fromParts("package", context.packageName, null),
                                        ),
                                    )
                                } catch (_: ActivityNotFoundException) {
                                    context.startActivity(detailsIntent)
                                }
                            } else {
                                context.startActivity(detailsIntent)
                            }
                        },
                    )
                }
            }
        }
        item {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                SettingsSectionTitle(stringResource(R.string.sound_quick_access))
                SettingsGroup {
                    if (onNotificationSettings != null) {
                        SettingsAction(
                            title = stringResource(R.string.sound_notification_controls),
                            description = stringResource(R.string.sound_notification_controls_description),
                            icon = R.drawable.notifications_24px,
                            onClick = onNotificationSettings,
                        )
                        SettingsDivider()
                    }
                    SettingsAction(
                        title = stringResource(R.string.sound_widget),
                        description = stringResource(R.string.sound_widget_description),
                        icon = R.drawable.view_agenda_24px,
                        onClick = {
                            val manager = AppWidgetManager.getInstance(context)
                            val pinned = manager.isRequestPinAppWidgetSupported && manager.requestPinAppWidget(
                                ComponentName(context, SettingWidgetReceiver::class.java),
                                null,
                                null,
                            )
                            if (!pinned) showWidgetHelp = true
                        },
                    )
                    SettingsDivider()
                    SettingsToggle(
                        title = stringResource(R.string.auto_connect),
                        description = stringResource(R.string.sound_auto_connect_description),
                        icon = R.drawable.bluetooth_24px,
                        checked = autoConnect,
                        onCheckedChange = viewModel::setAutoConnect,
                    )
                }
            }
        }
        item {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                SettingsSectionTitle(stringResource(R.string.sound_diagnostics))
                SettingsGroup {
                    SettingsAction(
                        title = stringResource(R.string.copy_logs_to_clipboard),
                        description = stringResource(R.string.sound_logs_description),
                        icon = R.drawable.content_copy_24px,
                        onClick = viewModel::copyLogs,
                    )
                    SettingsDivider()
                    SettingsAction(
                        title = stringResource(R.string.copy_logs_to_clipboard_unfiltered),
                        icon = R.drawable.content_copy_24px,
                        onClick = viewModel::copyLogsUnfiltered,
                    )
                }
            }
        }
    }
}

@Composable
private fun SettingsSectionTitle(text: String) {
    Text(
        text,
        modifier = Modifier.padding(start = 12.dp),
        style = MaterialTheme.typography.labelLarge,
        color = MaterialTheme.colorScheme.primary,
    )
}

@Composable
private fun ThemeSelection(theme: ThemeType?, onThemeChange: (ThemeType?) -> Unit) {
    SegmentedSelection(
        options = listOf(
            null to stringResource(R.string.system_theme),
            ThemeType.Light to stringResource(R.string.light),
            ThemeType.Dark to stringResource(R.string.dark),
        ),
        selected = theme,
        onSelect = onThemeChange,
    )
}

@Composable
private fun EarbudsColorSelection(color: EarbudsColor?, onColorChange: (EarbudsColor?) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(
            stringResource(R.string.sound_earbuds_color),
            modifier = Modifier.padding(start = 12.dp),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        SegmentedSelection(
            options = listOf(
                null to stringResource(R.string.sound_earbuds_color_theme),
                EarbudsColor.Silver to stringResource(R.string.sound_earbuds_color_silver),
                EarbudsColor.Black to stringResource(R.string.sound_earbuds_color_black),
            ),
            selected = color,
            onSelect = onColorChange,
        )
    }
}

@Composable
private fun SoundModesViewSelection(view: SoundModesView, onViewChange: (SoundModesView) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(
            stringResource(R.string.sound_modes_view),
            modifier = Modifier.padding(start = 12.dp),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        SegmentedSelection(
            options = listOf(
                SoundModesView.Tiles to stringResource(R.string.sound_modes_view_tiles),
                SoundModesView.Scale to stringResource(R.string.sound_modes_view_scale),
            ),
            selected = view,
            onSelect = onViewChange,
        )
    }
}

@Composable
private fun <T> SegmentedSelection(options: List<Pair<T, String>>, selected: T, onSelect: (T) -> Unit) {
    Row(
        Modifier.fillMaxWidth().clip(MaterialTheme.shapes.medium)
            .background(MaterialTheme.colorScheme.surfaceContainer).padding(4.dp).selectableGroup(),
    ) {
        options.forEach { (value, label) ->
            Surface(
                modifier = Modifier.weight(1f).clip(RoundedCornerShape(20.dp)).selectable(
                    selected = selected == value,
                    role = Role.RadioButton,
                    onClick = { onSelect(value) },
                ),
                shape = RoundedCornerShape(20.dp),
                color = if (selected ==
                    value
                ) {
                    MaterialTheme.colorScheme.surfaceContainerLowest
                } else {
                    MaterialTheme.colorScheme.surfaceContainer
                },
            ) {
                Text(
                    label,
                    modifier = Modifier.heightIn(min = 48.dp).padding(horizontal = 6.dp, vertical = 14.dp),
                    style = MaterialTheme.typography.labelMedium,
                    textAlign = TextAlign.Center,
                )
            }
        }
    }
}

@Composable
private fun SettingsGroup(content: @Composable () -> Unit) {
    Surface(
        modifier = Modifier.fillMaxWidth(),
        color = MaterialTheme.colorScheme.surfaceContainerLowest,
        shape = MaterialTheme.shapes.medium,
    ) {
        Column { content() }
    }
}

@Composable
private fun SettingsDivider() {
    HorizontalDivider(Modifier.padding(start = 64.dp), color = MaterialTheme.colorScheme.surfaceContainer)
}

@Composable
private fun SettingsAction(title: String, description: String? = null, @DrawableRes icon: Int, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().clickable(role = Role.Button, onClick = onClick).heightIn(min = 80.dp).padding(16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        SettingsIcon(icon)
        SettingsLabel(title, description, Modifier.weight(1f))
        Icon(
            painterResource(R.drawable.arrow_back_24px),
            contentDescription = null,
            modifier = Modifier.size(20.dp).rotate(180f),
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun SettingsToggle(
    title: String,
    description: String,
    @DrawableRes icon: Int,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
) {
    Row(
        Modifier.fillMaxWidth().toggleable(value = checked, role = Role.Switch, onValueChange = onCheckedChange)
            .heightIn(min = 80.dp).padding(16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        SettingsIcon(icon)
        SettingsLabel(title, description, Modifier.weight(1f))
        Switch(checked = checked, onCheckedChange = null)
    }
}

@Composable
private fun SettingsIcon(@DrawableRes icon: Int) {
    Box(
        modifier = Modifier.size(
            38.dp,
        ).background(MaterialTheme.colorScheme.surfaceContainer, RoundedCornerShape(13.dp)),
        contentAlignment = Alignment.Center,
    ) {
        Icon(painterResource(icon), contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
private fun SettingsLabel(title: String, description: String?, modifier: Modifier) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(title, style = MaterialTheme.typography.bodyMedium)
        if (description != null) {
            Text(
                description,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}
