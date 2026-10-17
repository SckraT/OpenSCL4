@file:OptIn(ExperimentalMaterial3Api::class)

package com.oppzippy.openscq30.ui.devicesettings

import android.net.Uri
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.requiredWidth
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.features.equalizer.storage.LegacyEqualizerProfile
import com.oppzippy.openscq30.features.soundcoredevice.service.ConnectionStatus
import com.oppzippy.openscq30.lib.bindings.translateCategoryId
import com.oppzippy.openscq30.lib.wrapper.QuickPreset
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.lib.wrapper.Value
import com.oppzippy.openscq30.ui.devicesettings.screens.MigrateLegacyEqualizerProfilesScreen
import com.oppzippy.openscq30.ui.devicesettings.screens.StatusNotificationScreen
import com.oppzippy.openscq30.ui.settings.SettingsPage
import com.oppzippy.openscq30.ui.sound.BOTH_EARBUDS_SETTING_IDS
import com.oppzippy.openscq30.ui.sound.LocalSettingValueSetter
import com.oppzippy.openscq30.ui.sound.SoundCapabilities
import com.oppzippy.openscq30.ui.sound.SoundCasePage
import com.oppzippy.openscq30.ui.sound.SoundComfortPage
import com.oppzippy.openscq30.ui.sound.SoundConnectionStateScreen
import com.oppzippy.openscq30.ui.sound.SoundConnectionsPage
import com.oppzippy.openscq30.ui.sound.SoundControlsPage
import com.oppzippy.openscq30.ui.sound.SoundDashboardLink
import com.oppzippy.openscq30.ui.sound.SoundEqualizerPage
import com.oppzippy.openscq30.ui.sound.SoundModesPage
import com.oppzippy.openscq30.ui.sound.SoundOverview
import com.oppzippy.openscq30.ui.sound.SoundPresetEditorPage
import com.oppzippy.openscq30.ui.sound.SoundOneEarbudNotice
import com.oppzippy.openscq30.ui.sound.SoundPresetsPage
import com.oppzippy.openscq30.ui.sound.SoundSettingsListPage
import com.oppzippy.openscq30.ui.sound.SoundSpatialPage
import com.oppzippy.openscq30.ui.sound.categoryNeedsBothEarbuds
import com.oppzippy.openscq30.ui.sound.isOnlyOneEarbudConnected
import com.oppzippy.openscq30.ui.sound.OverviewBlock
import com.oppzippy.openscq30.ui.sound.OverviewLayoutEditor
import com.oppzippy.openscq30.ui.sound.presetsUseBothEarbudSettings
import com.oppzippy.openscq30.ui.utils.NavTransitions
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flowOf

private const val HOME = "overview"
private const val PRESETS = "presets"
private const val APP_SETTINGS = "appSettings"
private const val OVERVIEW_LAYOUT = "overviewLayout"

@Composable
fun DeviceSettingsScreen(
    onBack: () -> Unit = {},
    connectionStatus: ConnectionStatus,
    setSettingValues: (List<Pair<String, Value>>) -> Unit,
    setSettingValueForResult: suspend (String, Value) -> Result<Unit>,
    allSettingsFlow: Flow<List<Pair<String, Setting>>>,
    categoryIdsFlow: Flow<List<String>>,
    getSettingsInCategoryFlow: (String) -> Flow<List<Pair<String, Setting>>>,
    quickPresetSlotsFlow: Flow<List<String?>>,
    onQuickPresetSlotChange: (Int, String?) -> Unit,
    quickPresetsFlow: Flow<List<QuickPreset>>,
    activateQuickPreset: (String) -> Unit,
    createQuickPreset: (String) -> Unit,
    deleteQuickPreset: (String) -> Unit,
    toggleQuickPresetSetting: (String, String, Boolean) -> Unit,
    featuredSettingSlotsFlow: Flow<List<String?>>,
    onFeaturedSettingSlotChange: (Int, String?) -> Unit,
    onQuickPresetLoadCurrentSettings: (String) -> Unit,
    legacyEqualizerProfilesFlow: Flow<List<LegacyEqualizerProfile>>,
    onMigrateLegacyEqualizerProfile: (LegacyEqualizerProfile) -> Unit,
    onDisconnect: () -> Unit = {},
) {
    val fontScale = LocalDensity.current.fontScale
    val connection = connectionStatus as? ConnectionStatus.Connected
    if (connection == null) {
        SoundConnectionStateScreen(connectionStatus, onBack = onBack)
        return
    }
    val loadedSettings by remember(connection.deviceManager) { allSettingsFlow }.collectAsState(initial = null)
    val settings = loadedSettings.orEmpty()
    val oneEarbudOnly = isOnlyOneEarbudConnected(settings.toMap())
    val categories by remember(connection.deviceManager) { categoryIdsFlow }.collectAsState(emptyList())
    val byCategory by remember(categories, connection.deviceManager) {
        if (categories.isEmpty()) {
            flowOf(emptyMap<String, List<Pair<String, Setting>>>())
        } else {
            combine(categories.map { getSettingsInCategoryFlow(it) }) { lists ->
                categories.zip(lists.toList()).toMap()
            }
        }
    }.collectAsState(emptyMap())
    val presets by remember(connection.deviceManager) { quickPresetsFlow }.collectAsState(emptyList())
    val modeSettings = settings.filter {
        it.first in SoundCapabilities.modes ||
            byCategory["soundModes"].orEmpty().any { s -> s.first == it.first }
    }
    val controlSettings = settings.filter {
        SoundCapabilities.isControl(it.first) ||
            byCategory["buttonConfiguration"].orEmpty().any { s -> s.first == it.first }
    }
    val comfortSettings = settings.filter { it.first in SoundCapabilities.comfort }
    val connectionSettings = settings.filter { it.first in SoundCapabilities.connections }
    val caseSettings = settings.filter {
        it.first in SoundCapabilities.case ||
            byCategory["case"].orEmpty().any { s -> s.first == it.first }
    }
    val spatialSettings = settings.filter { it.first in SoundCapabilities.spatial }
    val navController = rememberNavController()
    val entry by navController.currentBackStackEntryAsState()
    val route = entry?.destination?.route ?: HOME
    val rootRoute = route in setOf(HOME, PRESETS, APP_SETTINGS)
    var showMenu by remember { mutableStateOf(false) }
    var confirmDisconnect by remember { mutableStateOf(false) }
    // Which presets match the device is derived from its settings, but several can match at once, so the one the user
    // applied last is remembered separately. Survives rotation, and starts over for a new connection.
    var lastAppliedPreset by rememberSaveable(connection.deviceManager) { mutableStateOf<String?>(null) }

    fun openCategory(id: String) = navController.navigate("category/${Uri.encode(id)}")
    if (confirmDisconnect) {
        AlertDialog(
            onDismissRequest = { confirmDisconnect = false },
            title = { Text(stringResource(R.string.sound_overview_disconnect_confirm)) },
            text = { Text(stringResource(R.string.sound_overview_disconnect_hint)) },
            confirmButton = {
                TextButton(onClick = {
                    confirmDisconnect = false
                    onDisconnect()
                }) { Text(stringResource(R.string.disconnect)) }
            },
            dismissButton = {
                TextButton(onClick = { confirmDisconnect = false }) { Text(stringResource(R.string.cancel)) }
            },
        )
    }
    val title = when (route) {
        HOME -> stringResource(R.string.app_name)
        PRESETS -> stringResource(R.string.quick_presets)
        APP_SETTINGS -> stringResource(R.string.settings)
        OVERVIEW_LAYOUT -> stringResource(R.string.sound_overview_edit)
        "sound" -> stringResource(R.string.sound_overview_sound_mode)
        "equalizer" -> stringResource(R.string.equalizer)
        "controls" -> stringResource(R.string.sound_overview_controls)
        "comfort" -> stringResource(R.string.sound_overview_comfort)
        "connections" -> stringResource(R.string.sound_overview_connections)
        "case" -> stringResource(R.string.sound_overview_case)
        "spatial" -> stringResource(R.string.sound_overview_spatial)
        "notifications" -> stringResource(R.string.sound_overview_notifications)
        "legacy" -> stringResource(R.string.sound_overview_legacy)
        "preset/{name}" -> entry?.arguments?.getString("name") ?: stringResource(R.string.edit)
        "category/{id}" -> entry?.arguments?.getString("id")?.let { translateCategoryId(it) }.orEmpty()
        else -> stringResource(R.string.sound_overview_all_settings)
    }
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                navigationIcon = {
                    if (!rootRoute) {
                        IconButton(onClick = { navController.popBackStack() }) {
                            Icon(painterResource(R.drawable.arrow_back_24px), stringResource(R.string.back))
                        }
                    }
                },
                actions = {
                    if (route == HOME) {
                        IconButton(onClick = onBack) {
                            Icon(
                                painterResource(R.drawable.earbuds_2_24px),
                                stringResource(R.string.sound_overview_devices),
                            )
                        }
                    }
                    IconButton(onClick = { showMenu = true }) {
                        Icon(
                            painterResource(R.drawable.sound_more),
                            stringResource(R.string.sound_overview_device_actions),
                        )
                    }
                    DropdownMenu(expanded = showMenu, onDismissRequest = { showMenu = false }) {
                        DropdownMenuItem(text = { Text(stringResource(R.string.sound_overview_devices)) }, onClick = {
                            showMenu =
                                false
                            onBack()
                        })
                        DropdownMenuItem(text = {
                            Text(stringResource(R.string.sound_overview_all_settings))
                        }, onClick = {
                            showMenu =
                                false
                            navController.navigate("allSettings")
                        })
                        if (route == HOME) {
                            DropdownMenuItem(text = { Text(stringResource(R.string.sound_overview_edit)) }, onClick = {
                                showMenu = false
                                navController.navigate(OVERVIEW_LAYOUT)
                            })
                        }
                        DropdownMenuItem(text = {
                            Text(stringResource(R.string.sound_overview_disconnect))
                        }, onClick = {
                            showMenu =
                                false
                            confirmDisconnect = true
                        })
                    }
                },
            )
        },
        bottomBar = {
            if (rootRoute) {
                BoxWithConstraints {
                    val labelWidth = (maxWidth / 3).coerceAtLeast(48.dp)
                    NavigationBar {
                        listOf(
                            Triple(HOME, R.string.sound_overview_device, R.drawable.earbuds_2_24px),
                            Triple(PRESETS, R.string.quick_presets, R.drawable.bolt_24px),
                            Triple(APP_SETTINGS, R.string.settings, R.drawable.settings_24px),
                        ).forEach { (destination, label, icon) ->
                            NavigationBarItem(
                                modifier = Modifier.heightIn(min = (80 + 48 * (fontScale - 1).coerceAtLeast(0f)).dp),
                                selected = route == destination,
                                onClick = {
                                    navController.navigate(destination) {
                                        popUpTo(HOME) { saveState = true }
                                        launchSingleTop = true
                                        restoreState = true
                                    }
                                },
                                icon = { Icon(painterResource(icon), null) },
                                label = {
                                    Text(
                                        stringResource(label),
                                        modifier = Modifier.requiredWidth(labelWidth),
                                        style = if (fontScale > 1.4f) {
                                            MaterialTheme.typography.labelSmall
                                        } else {
                                            MaterialTheme.typography.labelMedium
                                        },
                                        minLines = if (fontScale > 1.4f) 2 else 1,
                                        maxLines = 2,
                                        overflow = TextOverflow.Ellipsis,
                                        textAlign = TextAlign.Center,
                                    )
                                },
                            )
                        }
                    }
                }
            }
        },
    ) { padding ->
        NavHost(
            navController,
            startDestination = HOME,
            modifier = Modifier.padding(padding),
            enterTransition = NavTransitions.enter,
            exitTransition = NavTransitions.exit,
            popEnterTransition = NavTransitions.popEnter,
            popExitTransition = NavTransitions.popExit,
            predictivePopEnterTransition = NavTransitions.predictivePopEnter,
            predictivePopExitTransition = NavTransitions.predictivePopExit,
        ) {
            composable(HOME) {
                if (loadedSettings == null) {
                    Column(
                        Modifier.fillMaxSize(),
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(16.dp, Alignment.CenterVertically),
                    ) {
                        CircularProgressIndicator()
                        Text(stringResource(R.string.loading), style = MaterialTheme.typography.bodyLarge)
                    }
                } else {
                    SoundOverview(
                        deviceModel = connection.deviceManager.device.model(),
                        settings = settings,
                        hasSound = modeSettings.isNotEmpty(),
                        hasControls = controlSettings.isNotEmpty(),
                        hasComfort = comfortSettings.isNotEmpty(),
                        hasConnections = connectionSettings.isNotEmpty(),
                        hasCase = caseSettings.isNotEmpty(),
                        hasEqualizerCategory = "equalizer" in categories,
                        macAddress = connection.macAddress,
                        quickPresets = presets.map { it.name },
                        activeQuickPreset = lastAppliedPreset,
                        onActivateQuickPreset = {
                            activateQuickPreset(it)
                            lastAppliedPreset = it
                        },
                        onEditLayout = { navController.navigate(OVERVIEW_LAYOUT) },
                        onSound = {
                            navController.navigate("sound")
                        }, onEqualizer = { navController.navigate("equalizer") },
                        onControls = {
                            navController.navigate("controls")
                        }, onComfort = { navController.navigate("comfort") },
                        onConnections = {
                            navController.navigate("connections")
                        }, onSpatial = { navController.navigate("spatial") },
                        onCase = {
                            navController.navigate("case")
                        }, onAllSettings = { navController.navigate("allSettings") },
                        setSettings = setSettingValues,
                    )
                }
            }
            composable("sound") {
                SoundModesPage(connection.deviceManager.device.model(), modeSettings, setSettingValues)
            }
            composable(OVERVIEW_LAYOUT) {
                val available = buildSet {
                    if (modeSettings.isNotEmpty()) add(OverviewBlock.SoundMode)
                    if ("equalizer" in categories || settings.any { it.first in BOTH_EARBUDS_SETTING_IDS }) {
                        add(OverviewBlock.Equalizer)
                    }
                    // presets can be added later, so the block can be turned on before there are any
                    add(OverviewBlock.QuickPresets)
                    if (controlSettings.isNotEmpty()) add(OverviewBlock.Controls)
                    if (comfortSettings.isNotEmpty()) add(OverviewBlock.Comfort)
                    if (settings.any { it.first == "spatialAudio" }) add(OverviewBlock.Spatial)
                    if (connectionSettings.isNotEmpty()) add(OverviewBlock.Connections)
                    if (caseSettings.isNotEmpty()) add(OverviewBlock.Case)
                }
                OverviewLayoutEditor(connection.deviceManager.device.model(), available)
            }
            composable("equalizer") {
                // the earbuds can disconnect from each other while this page is open
                if (oneEarbudOnly && settings.none { it.first in BOTH_EARBUDS_SETTING_IDS }) {
                    SoundOneEarbudNotice(Modifier.padding(20.dp))
                } else {
                    SoundEqualizerPage(settings, setSettingValues, onImportExport = {
                        openCategory("equalizerImportExport")
                    })
                }
            }
            composable("controls") { SoundControlsPage(controlSettings, setSettingValues) }
            composable("comfort") { SoundComfortPage(comfortSettings, setSettingValues) }
            composable("connections") { SoundConnectionsPage(connectionSettings, setSettingValues) }
            composable("case") { SoundCasePage(caseSettings, setSettingValues) }
            composable("spatial") { SoundSpatialPage(spatialSettings, setSettingValues) }
            composable(PRESETS) {
                SoundPresetsPage(
                    presets,
                    settings,
                    lastAppliedPreset,
                    onActivate = {
                        lastAppliedPreset = it
                        activateQuickPreset(it)
                    },
                    createQuickPreset,
                    onEdit = { navController.navigate("preset/${Uri.encode(it)}") },
                    onDelete = deleteQuickPreset,
                    notice = if (oneEarbudOnly && presetsUseBothEarbudSettings(presets)) {
                        { SoundOneEarbudNotice(message = stringResource(R.string.sound_overview_one_earbud_presets)) }
                    } else {
                        null
                    },
                )
            }
            composable(
                "preset/{name}",
                arguments = listOf(
                    navArgument("name") {
                        type = NavType.StringType
                    },
                ),
            ) { current ->
                val preset = presets.find { it.name == current.arguments?.getString("name") }
                if (preset != null) {
                    SoundPresetEditorPage(
                        preset,
                        settings.toMap(),
                        toggleQuickPresetSetting,
                        onLoadCurrentSettings = { onQuickPresetLoadCurrentSettings(preset.name) },
                    )
                } else {
                    Text(stringResource(R.string.sound_overview_preset_missing), Modifier.padding(20.dp))
                }
            }
            composable(APP_SETTINGS) {
                SettingsPage(
                    onBackClick = { navController.popBackStack() },
                    embedded = true,
                    onNotificationSettings = { navController.navigate("notifications") },
                )
            }
            composable("allSettings") {
                Column(
                    Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(20.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Text(
                        stringResource(R.string.sound_overview_all_settings),
                        style = MaterialTheme.typography.headlineMedium,
                    )
                    if (oneEarbudOnly) SoundOneEarbudNotice()
                    categories.forEach { id ->
                        val unavailable = oneEarbudOnly && categoryNeedsBothEarbuds(id) &&
                            byCategory[id].isNullOrEmpty()
                        SoundDashboardLink(
                            translateCategoryId(
                                id,
                            ),
                            if (unavailable) stringResource(R.string.sound_overview_needs_both_earbuds) else null,
                            Screen.SettingsCategory(id).screenInfo().icon,
                            onClick = {
                                openCategory(id)
                            },
                            enabled = !unavailable,
                        )
                    }
                    SoundDashboardLink(
                        stringResource(
                            R.string.sound_overview_notifications,
                        ),
                        null,
                        R.drawable.notifications_24px,
                        onClick = {
                            navController.navigate("notifications")
                        },
                    )
                    SoundDashboardLink(
                        stringResource(
                            R.string.sound_overview_legacy,
                        ),
                        null,
                        R.drawable.update_24px,
                        onClick = {
                            navController.navigate("legacy")
                        },
                    )
                }
            }
            composable(
                "category/{id}",
                arguments = listOf(
                    navArgument("id") {
                        type = NavType.StringType
                    },
                ),
            ) { current ->
                val id = current.arguments?.getString("id").orEmpty()
                val categorySettings = byCategory[id].orEmpty()
                if (categorySettings.isEmpty() && oneEarbudOnly && categoryNeedsBothEarbuds(id)) {
                    SoundOneEarbudNotice(Modifier.padding(20.dp))
                } else if (categorySettings.isEmpty()) {
                    Text(stringResource(R.string.sound_overview_no_settings), Modifier.padding(20.dp))
                } else {
                    CompositionLocalProvider(LocalSettingValueSetter provides setSettingValueForResult) {
                        SoundSettingsListPage(categorySettings, setSettingValues)
                    }
                }
            }
            composable("notifications") {
                val featuredSlots by remember(connection.deviceManager) {
                    featuredSettingSlotsFlow
                }.collectAsState(emptyList())
                val presetSlots by remember(connection.deviceManager) {
                    quickPresetSlotsFlow
                }.collectAsState(emptyList())
                StatusNotificationScreen(
                    settingIds = settings.map { it.first },
                    featuredSettingSlots = featuredSlots,
                    onFeaturedSettingSlotChange = onFeaturedSettingSlotChange,
                    quickPresets = presets.map { it.name },
                    quickPresetSlots = presetSlots,
                    onQuickPresetSlotChange = onQuickPresetSlotChange,
                )
            }
            composable("legacy") {
                val legacy by remember(connection.deviceManager) {
                    legacyEqualizerProfilesFlow
                }.collectAsState(emptyList())
                MigrateLegacyEqualizerProfilesScreen(legacy, onMigrateLegacyEqualizerProfile)
            }
        }
    }
}
