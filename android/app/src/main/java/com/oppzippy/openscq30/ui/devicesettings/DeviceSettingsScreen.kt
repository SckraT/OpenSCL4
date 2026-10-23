@file:OptIn(ExperimentalMaterial3Api::class)

package com.oppzippy.openscq30.ui.devicesettings

import android.net.Uri
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.requiredWidth
import androidx.compose.foundation.layout.size
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
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
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
import com.oppzippy.openscq30.features.soundcoredevice.service.textResource
import com.oppzippy.openscq30.lib.bindings.SessionFailureReason
import com.oppzippy.openscq30.lib.bindings.SettingCategory
import com.oppzippy.openscq30.lib.bindings.WriteOutcome
import com.oppzippy.openscq30.lib.wrapper.QuickPreset
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.lib.wrapper.Value
import com.oppzippy.openscq30.lib.wrapper.isAvailable
import com.oppzippy.openscq30.lib.wrapper.toValue
import com.oppzippy.openscq30.ui.PendingSettingWrite
import com.oppzippy.openscq30.ui.devicesettings.screens.MigrateLegacyEqualizerProfilesScreen
import com.oppzippy.openscq30.ui.devicesettings.screens.StatusNotificationScreen
import com.oppzippy.openscq30.ui.settings.SettingsPage
import com.oppzippy.openscq30.ui.sound.BOTH_EARBUDS_SETTING_IDS
import com.oppzippy.openscq30.ui.sound.LocalPendingSettingWrites
import com.oppzippy.openscq30.ui.sound.LocalSettingAvailability
import com.oppzippy.openscq30.ui.sound.LocalSettingValueSetter
import com.oppzippy.openscq30.ui.sound.LocalSettingWriteErrors
import com.oppzippy.openscq30.ui.sound.LocalSettingsReadOnly
import com.oppzippy.openscq30.ui.sound.OverviewBlock
import com.oppzippy.openscq30.ui.sound.OverviewLayoutEditor
import com.oppzippy.openscq30.ui.sound.ProfileTransferSection
import com.oppzippy.openscq30.ui.sound.SettingGroup
import com.oppzippy.openscq30.ui.sound.SoundCasePage
import com.oppzippy.openscq30.ui.sound.SoundComfortPage
import com.oppzippy.openscq30.ui.sound.SoundConnectionStateScreen
import com.oppzippy.openscq30.ui.sound.SoundConnectionsPage
import com.oppzippy.openscq30.ui.sound.SoundControlsPage
import com.oppzippy.openscq30.ui.sound.SoundDashboardLink
import com.oppzippy.openscq30.ui.sound.SoundEffectsPage
import com.oppzippy.openscq30.ui.sound.SoundEqualizerPage
import com.oppzippy.openscq30.ui.sound.SoundModesPage
import com.oppzippy.openscq30.ui.sound.SoundOneEarbudNotice
import com.oppzippy.openscq30.ui.sound.SoundOverview
import com.oppzippy.openscq30.ui.sound.SoundPowerPage
import com.oppzippy.openscq30.ui.sound.SoundPresetEditorPage
import com.oppzippy.openscq30.ui.sound.SoundPresetsPage
import com.oppzippy.openscq30.ui.sound.SoundSettingsListPage
import com.oppzippy.openscq30.ui.sound.equalizerOpensItsPage
import com.oppzippy.openscq30.ui.sound.groupSettings
import com.oppzippy.openscq30.ui.sound.isOnlyOneEarbudConnected
import com.oppzippy.openscq30.ui.sound.presetsUseBlockedSettings
import com.oppzippy.openscq30.ui.sound.rememberSavedProfiles
import com.oppzippy.openscq30.ui.sound.settingAvailability
import com.oppzippy.openscq30.ui.sound.title
import com.oppzippy.openscq30.ui.utils.NavTransitions
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow

private const val HOME = "overview"
private const val PRESETS = "presets"
private const val APP_SETTINGS = "appSettings"
private const val OVERVIEW_LAYOUT = "overviewLayout"
private const val ALL_SETTINGS = "allSettings"
private const val POWER = "power"

@Composable
fun DeviceSettingsScreen(
    onBack: () -> Unit = {},
    onRetry: () -> Unit = {},
    connectionStatus: ConnectionStatus,
    setSettingValues: (List<Pair<String, Value>>) -> Unit,
    setSettingValueForResult: suspend (String, Value) -> Result<Unit>,
    pendingWritesFlow: Flow<Map<String, PendingSettingWrite>>,
    settingWriteErrorsFlow: Flow<Map<String, WriteOutcome>>,
    settingsByCategoryFlow: Flow<List<SettingCategory>?>,
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
    onMigrateLegacyEqualizerProfiles: (List<LegacyEqualizerProfile>) -> Unit,
    onDisconnect: () -> Unit = {},
) {
    val fontScale = LocalDensity.current.fontScale
    val deviceManager = connectionStatus.deviceManager
    if (deviceManager == null) {
        SoundConnectionStateScreen(connectionStatus, onBack = onBack, onRetry = onRetry)
        return
    }
    val lost = connectionStatus as? ConnectionStatus.Lost
    val readOnly = connectionStatus is ConnectionStatus.Recovering || lost != null
    val loadedSettings by remember(deviceManager) { settingsByCategoryFlow }.collectAsState(initial = null)
    val pendingWrites by remember(deviceManager) { pendingWritesFlow }.collectAsState(emptyMap())
    val settingWriteErrors by remember(deviceManager) { settingWriteErrorsFlow }.collectAsState(emptyMap())
    val byCategory = remember(loadedSettings) {
        loadedSettings.orEmpty().associate { category ->
            category.categoryId to category.settings.map { it.settingId to it.setting }
        }
    }
    val categories = byCategory.keys.toList()
    val settings = remember(byCategory) { byCategory.values.flatten().distinctBy { it.first } }
    val settingsById = remember(settings) { settings.toMap() }
    val availability = remember(settingsById) { settingAvailability(settingsById) }
    // the equalizer settings are not reported while one earbud is connected, so ask about the setting that describes it
    val equalizerAvailability = remember(settingsById) { settingAvailability(settingsById, "volumeAdjustments") }
    val equalizerAvailable = equalizerAvailability.isAvailable()
    val oneEarbudOnly = isOnlyOneEarbudConnected(settingsById)
    val presets by remember(deviceManager) { quickPresetsFlow }.collectAsState(emptyList())
    val legacyProfiles by remember(deviceManager) { legacyEqualizerProfilesFlow }.collectAsState(emptyList())
    // every setting has one group, and the group's page is where it is shown
    val grouped = remember(settings, byCategory) { groupSettings(settings, byCategory) }
    fun groupSettingsOf(group: SettingGroup) = grouped[group].orEmpty()
    val modeSettings = groupSettingsOf(SettingGroup.SoundModes)
    val controlSettings = groupSettingsOf(SettingGroup.Controls)
    val comfortSettings = groupSettingsOf(SettingGroup.Comfort)
    val connectionSettings = groupSettingsOf(SettingGroup.Connections)
    val caseSettings = groupSettingsOf(SettingGroup.Case)
    val effectSettings = groupSettingsOf(SettingGroup.SoundEffects)
    val powerSettings = groupSettingsOf(SettingGroup.Power)
    val navController = rememberNavController()
    val entry by navController.currentBackStackEntryAsState()
    val route = entry?.destination?.route ?: HOME
    val rootRoute = route in setOf(HOME, PRESETS, APP_SETTINGS)
    var showMenu by remember { mutableStateOf(false) }
    var confirmDisconnect by remember { mutableStateOf(false) }
    val lastAppliedPreset by deviceManager.lastAppliedPreset.collectAsState()

    val equalizerPage = equalizerOpensItsPage(byCategory["equalizer"].orEmpty().map { it.first }, oneEarbudOnly)

    // the page of a group: the one the device screen opens too, or else the plain list of the group's settings
    fun groupRoute(group: SettingGroup): String = when (group) {
        SettingGroup.SoundModes -> "sound"
        SettingGroup.Equalizer -> if (equalizerPage) "equalizer" else "group/${group.id}"
        SettingGroup.SoundEffects -> "spatial"
        SettingGroup.Controls -> "controls"
        SettingGroup.Comfort -> "comfort"
        SettingGroup.Power -> POWER
        SettingGroup.Connections -> "connections"
        SettingGroup.Case -> "case"
        else -> "group/${group.id}"
    }
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

        "spatial" -> stringResource(R.string.sound_overview_effects)

        POWER -> stringResource(R.string.sound_overview_power)

        "notifications" -> stringResource(R.string.sound_notification_controls)

        "legacy" -> stringResource(R.string.sound_overview_legacy)

        "preset/{name}" -> entry?.arguments?.getString("name") ?: stringResource(R.string.edit)

        "group/{id}" -> {
            val id = entry?.arguments?.getString("id")
            SettingGroup.entries.firstOrNull { it.id == id }?.title().orEmpty()
        }

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
                        // the device screen has a button for my devices already
                        if (route != HOME) {
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.sound_overview_devices)) },
                                onClick = {
                                    showMenu = false
                                    onBack()
                                },
                            )
                        }
                        if (route != ALL_SETTINGS) {
                            DropdownMenuItem(text = {
                                Text(stringResource(R.string.sound_overview_all_settings))
                            }, onClick = {
                                showMenu = false
                                navController.navigate(ALL_SETTINGS)
                            })
                        }
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
        val recovering = connectionStatus is ConnectionStatus.Recovering
        var showReconnecting by remember { mutableStateOf(false) }
        LaunchedEffect(recovering) {
            showReconnecting = false
            if (recovering) {
                delay(1_000)
                showReconnecting = true
            }
        }
        Column(Modifier.padding(padding)) {
            if (showReconnecting) ReconnectingBanner()
            if (lost != null) LostBanner(lost.reason, onRetry)
            CompositionLocalProvider(
                LocalPendingSettingWrites provides pendingWrites,
                LocalSettingWriteErrors provides settingWriteErrors,
                LocalSettingsReadOnly provides readOnly,
                LocalSettingAvailability provides availability,
            ) {
                NavHost(
                    navController,
                    startDestination = HOME,
                    modifier = Modifier.weight(1f).alpha(if (lost != null) 0.55f else 1f),
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
                                deviceModel = deviceManager.device.model(),
                                settings = settings,
                                hasSound = modeSettings.isNotEmpty(),
                                hasControls = controlSettings.isNotEmpty(),
                                hasComfort = comfortSettings.isNotEmpty(),
                                hasConnections = connectionSettings.isNotEmpty(),
                                hasCase = caseSettings.isNotEmpty(),
                                hasEffects = effectSettings.isNotEmpty(),
                                hasPower = powerSettings.isNotEmpty(),
                                hasEqualizerCategory = "equalizer" in categories,
                                macAddress = connectionStatus.macAddress,
                                quickPresets = presets.map { it.name },
                                activeQuickPreset = lastAppliedPreset,
                                onActivateQuickPreset = activateQuickPreset,
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
                                onPower = { navController.navigate(POWER) },
                                onCase = {
                                    navController.navigate("case")
                                }, onAllSettings = { navController.navigate(ALL_SETTINGS) },
                                setSettings = setSettingValues,
                            )
                        }
                    }
                    composable("sound") {
                        SoundModesPage(deviceManager.device.model(), modeSettings, setSettingValues)
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
                            if (effectSettings.isNotEmpty()) add(OverviewBlock.Spatial)
                            if (powerSettings.isNotEmpty()) add(OverviewBlock.Power)
                            if (connectionSettings.isNotEmpty()) add(OverviewBlock.Connections)
                            if (caseSettings.isNotEmpty()) add(OverviewBlock.Case)
                        }
                        OverviewLayoutEditor(deviceManager.device.model(), available)
                    }
                    composable("equalizer") {
                        val transferSettings = byCategory["equalizerImportExport"].orEmpty()
                        val importProfiles: suspend (String) -> Result<Unit> = {
                            setSettingValueForResult("importCustomEqualizerProfiles", it.toValue())
                        }
                        // the earbuds can disconnect from each other while this page is open
                        if (!equalizerAvailable && settings.none { it.first in BOTH_EARBUDS_SETTING_IDS }) {
                            // profiles are kept on the phone, so they can still be imported and exported
                            Column(
                                Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(20.dp),
                                verticalArrangement = Arrangement.spacedBy(16.dp),
                            ) {
                                SoundOneEarbudNotice()
                                if (transferSettings.isNotEmpty()) {
                                    val savedProfiles = rememberSavedProfiles(transferSettings, setSettingValues)
                                    ProfileTransferSection(
                                        savedProfiles.keys.toList(),
                                        savedProfiles,
                                        null,
                                        importProfiles,
                                    )
                                }
                            }
                        } else {
                            SoundEqualizerPage(
                                settings = settings,
                                setSettings = setSettingValues,
                                importExportSettings = transferSettings,
                                legacyProfiles = legacyProfiles,
                                onMigrateLegacyProfiles = onMigrateLegacyEqualizerProfiles,
                                onImport = importProfiles,
                            )
                        }
                    }
                    composable("controls") { SoundControlsPage(controlSettings, setSettingValues) }
                    composable("comfort") { SoundComfortPage(comfortSettings, setSettingValues) }
                    composable("connections") { SoundConnectionsPage(connectionSettings, setSettingValues) }
                    composable("case") { SoundCasePage(caseSettings, setSettingValues) }
                    composable("spatial") { SoundEffectsPage(effectSettings, setSettingValues) }
                    composable(POWER) { SoundPowerPage(powerSettings, setSettingValues) }
                    composable(PRESETS) {
                        SoundPresetsPage(
                            presets,
                            settings,
                            lastAppliedPreset,
                            onActivate = activateQuickPreset,
                            createQuickPreset,
                            onEdit = { navController.navigate("preset/${Uri.encode(it)}") },
                            onDelete = deleteQuickPreset,
                            notice = if (presetsUseBlockedSettings(presets, settingsById)) {
                                {
                                    SoundOneEarbudNotice(
                                        message = stringResource(R.string.sound_overview_one_earbud_presets),
                                    )
                                }
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
                    composable(ALL_SETTINGS) {
                        Column(
                            Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(20.dp),
                            verticalArrangement = Arrangement.spacedBy(12.dp),
                        ) {
                            Text(
                                stringResource(R.string.sound_overview_all_settings),
                                style = MaterialTheme.typography.headlineMedium,
                            )
                            if (oneEarbudOnly) SoundOneEarbudNotice()
                            // the equalizer page is there with one earbud as well, to say why and to import and export
                            SettingGroup.entries.filter {
                                grouped[it].orEmpty().isNotEmpty() || (it == SettingGroup.Equalizer && equalizerPage)
                            }.forEach { group ->
                                val needsBothEarbuds = group == SettingGroup.Equalizer && !equalizerAvailable &&
                                    byCategory["equalizer"].isNullOrEmpty()
                                SoundDashboardLink(
                                    group.title(),
                                    if (needsBothEarbuds) {
                                        stringResource(
                                            R.string.sound_overview_needs_both_earbuds,
                                        )
                                    } else {
                                        null
                                    },
                                    group.icon,
                                    onClick = { navController.navigate(groupRoute(group)) },
                                )
                            }
                            // the equalizer page offers to move old profiles over itself
                            if (!equalizerPage && legacyProfiles.isNotEmpty()) {
                                SoundDashboardLink(
                                    stringResource(R.string.sound_overview_legacy),
                                    null,
                                    R.drawable.update_24px,
                                    onClick = { navController.navigate("legacy") },
                                )
                            }
                        }
                    }
                    composable(
                        "group/{id}",
                        arguments = listOf(
                            navArgument("id") {
                                type = NavType.StringType
                            },
                        ),
                    ) { current ->
                        val group = SettingGroup.entries.firstOrNull { it.id == current.arguments?.getString("id") }
                        val groupSettings = group?.let { groupSettingsOf(it) }.orEmpty()
                        if (groupSettings.isEmpty()) {
                            Text(stringResource(R.string.sound_overview_no_settings), Modifier.padding(20.dp))
                        } else {
                            CompositionLocalProvider(LocalSettingValueSetter provides setSettingValueForResult) {
                                SoundSettingsListPage(groupSettings, setSettingValues)
                            }
                        }
                    }
                    composable("notifications") {
                        val featuredSlots by remember(deviceManager) {
                            featuredSettingSlotsFlow
                        }.collectAsState(emptyList())
                        val presetSlots by remember(deviceManager) {
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
                        MigrateLegacyEqualizerProfilesScreen(legacyProfiles) {
                            onMigrateLegacyEqualizerProfiles(listOf(it))
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun LostBanner(reason: SessionFailureReason, onRetry: () -> Unit) {
    Surface(color = MaterialTheme.colorScheme.errorContainer, modifier = Modifier.fillMaxWidth()) {
        Row(
            Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(
                stringResource(R.string.disconnected),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onErrorContainer,
            )
            Text(
                stringResource(reason.textResource()),
                modifier = Modifier.weight(1f),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onErrorContainer,
            )
            TextButton(onClick = onRetry) {
                Text(stringResource(R.string.sound_overview_retry))
            }
        }
    }
}

@Composable
private fun ReconnectingBanner() {
    Surface(color = MaterialTheme.colorScheme.secondaryContainer, modifier = Modifier.fillMaxWidth()) {
        Row(
            Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
            Text(
                stringResource(R.string.sound_overview_reconnecting),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSecondaryContainer,
            )
        }
    }
}
