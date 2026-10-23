package com.oppzippy.openscq30.ui.deviceselection.screens

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.pulltorefresh.rememberPullToRefreshState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.features.phonelink.BluetoothAccess
import com.oppzippy.openscq30.features.phonelink.PhoneLinkSignals
import com.oppzippy.openscq30.lib.bindings.translateDeviceModel
import com.oppzippy.openscq30.lib.wrapper.PairedDevice
import com.oppzippy.openscq30.ui.deviceselection.ActiveDevice
import com.oppzippy.openscq30.ui.deviceselection.ActiveDeviceState
import com.oppzippy.openscq30.ui.deviceselection.AppControl
import com.oppzippy.openscq30.ui.deviceselection.AppControlSnapshot
import com.oppzippy.openscq30.ui.deviceselection.DeviceBattery
import com.oppzippy.openscq30.ui.deviceselection.DeviceListLabels
import com.oppzippy.openscq30.ui.deviceselection.DeviceListPresenter
import com.oppzippy.openscq30.ui.deviceselection.DeviceListState
import com.oppzippy.openscq30.ui.deviceselection.DeviceRowState
import com.oppzippy.openscq30.ui.deviceselection.TapAction
import com.oppzippy.openscq30.ui.deviceselection.splitBrand
import com.oppzippy.openscq30.ui.sound.DeviceArtOrIcon
import com.oppzippy.openscq30.ui.sound.DeviceHeroCard
import com.oppzippy.openscq30.ui.theme.OpenSCQ30Theme
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DeviceListingScreen(
    state: DeviceListState,
    activeDevice: ActiveDevice? = null,
    onRefreshClick: () -> Unit = {},
    onInfoClick: () -> Unit = {},
    onDeviceTap: (DeviceRowState) -> Unit = {},
    onBannerAction: (BluetoothAccess) -> Unit = {},
    onSettingsClick: () -> Unit = {},
    onUnpair: (PairedDevice) -> Unit = {},
    onAddDeviceClick: () -> Unit = {},
) {
    var deviceToUnpair by remember { mutableStateOf<PairedDevice?>(null) }
    deviceToUnpair?.let { device ->
        AlertDialog(
            onDismissRequest = { deviceToUnpair = null },
            title = { Text(stringResource(R.string.unpair_device)) },
            text = { Text(stringResource(R.string.unpairing_from_device_name, translateDeviceModel(device.model))) },
            dismissButton = {
                TextButton(onClick = { deviceToUnpair = null }) { Text(stringResource(R.string.cancel)) }
            },
            confirmButton = {
                TextButton(onClick = {
                    onUnpair(device)
                    deviceToUnpair = null
                }) { Text(stringResource(R.string.unpair)) }
            },
        )
    }
    Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            TopAppBar(
                title = {
                    Text(stringResource(R.string.sound_my_devices), maxLines = 1, overflow = TextOverflow.Ellipsis)
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = MaterialTheme.colorScheme.background),
                actions = {
                    IconButton(onClick = onInfoClick) {
                        Icon(painterResource(R.drawable.info_24px), contentDescription = stringResource(R.string.info))
                    }
                    IconButton(onClick = onSettingsClick) {
                        Icon(
                            painterResource(R.drawable.settings_24px),
                            contentDescription = stringResource(R.string.settings),
                        )
                    }
                },
            )
        },
    ) { innerPadding ->
        val refreshState = rememberPullToRefreshState()
        val scope = rememberCoroutineScope()
        PullToRefreshBox(
            modifier = Modifier.padding(innerPadding).fillMaxSize(),
            isRefreshing = false,
            onRefresh = {
                onRefreshClick()
                scope.launch { refreshState.animateToHidden() }
            },
            state = refreshState,
        ) {
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 32.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                if (!state.isLoading && state.access != BluetoothAccess.Ready) {
                    item(key = "bluetooth-access") {
                        BluetoothAccessBanner(state.access, onBannerAction)
                    }
                }
                val featured = state.featured
                if (state.isLoading) {
                    item {
                        Column(
                            Modifier.fillMaxWidth().padding(20.dp),
                            horizontalAlignment = Alignment.CenterHorizontally,
                            verticalArrangement = Arrangement.spacedBy(12.dp),
                        ) {
                            CircularProgressIndicator()
                            Text(stringResource(R.string.loading))
                        }
                    }
                } else if (featured == null) {
                    item { NoDevicesCard(onAddDeviceClick) }
                } else {
                    item(key = "featured") {
                        FeaturedDeviceCard(
                            row = featured,
                            activeDevice = activeDevice,
                            onTap = { onDeviceTap(featured) },
                            onUnpair = { deviceToUnpair = featured.device },
                        )
                    }
                    if (state.others.isNotEmpty()) {
                        item {
                            Text(
                                stringResource(R.string.sound_other_devices),
                                modifier = Modifier.padding(start = 8.dp, top = 8.dp),
                                style = MaterialTheme.typography.labelLarge,
                                color = MaterialTheme.colorScheme.primary,
                            )
                        }
                        items(state.others, key = {
                            "${it.device.model}:${it.device.macAddress}:${it.device.isDemo}"
                        }) { row ->
                            DeviceRow(
                                row = row,
                                onTap = { onDeviceTap(row) },
                                onUnpair = { deviceToUnpair = row.device },
                            )
                        }
                    }
                    item {
                        OutlinedButton(
                            modifier = Modifier.fillMaxWidth().heightIn(min = 52.dp),
                            onClick = onAddDeviceClick,
                        ) {
                            Icon(painterResource(R.drawable.add_24px), contentDescription = null)
                            Spacer(Modifier.size(8.dp))
                            Text(stringResource(R.string.sound_add_device))
                        }
                    }
                }
            }
        }
    }
}

/** The featured device with both status axes, its batteries while controlled and the next action. */
@Composable
private fun FeaturedDeviceCard(
    row: DeviceRowState,
    activeDevice: ActiveDevice?,
    onTap: () -> Unit,
    onUnpair: () -> Unit,
) {
    val controlling = row.control == AppControl.Controlling
    val active = activeDevice?.takeIf {
        it.macAddress.equals(row.device.macAddress, ignoreCase = true) && it.state == ActiveDeviceState.Connected
    }
    val showBatteries = controlling || row.device.isDemo
    val state = when {
        row.device.isDemo -> null
        row.control == AppControl.Controlling -> ActiveDeviceState.Connected
        row.control == AppControl.Connecting -> ActiveDeviceState.Connecting
        else -> null
    }
    DeviceHeroCard(
        model = row.device.model,
        isDemo = row.device.isDemo,
        state = state,
        batteries = if (showBatteries) active?.batteries.orEmpty() else emptyList(),
        macAddress = row.device.macAddress,
        oneEarbudBadge = showBatteries && active?.oneEarbudOnly == true,
        linkBadge = row.linkBadge.takeUnless { row.device.isDemo },
        controlBadge = row.controlBadge.takeUnless { row.device.isDemo },
        hint = if (!controlling && !row.device.isDemo) {
            stringResource(R.string.sound_device_not_connected_hint)
        } else {
            null
        },
        onClick = if (row.tapAction != TapAction.None) onTap else null,
        topEnd = { DeviceMenu(row.device, onUnpair) },
    ) {
        Button(
            onClick = onTap,
            enabled = row.tapAction != TapAction.None,
            modifier = Modifier.fillMaxWidth().heightIn(min = 52.dp),
        ) {
            Text(
                stringResource(
                    when (row.tapAction) {
                        TapAction.Open -> R.string.sound_open_device
                        TapAction.None -> R.string.sound_device_connecting
                        TapAction.Connect, TapAction.ResolveBluetoothAccess -> R.string.sound_device_connect
                    },
                ),
            )
        }
    }
}

@Composable
private fun DeviceRow(row: DeviceRowState, onTap: () -> Unit, onUnpair: () -> Unit) {
    val (_, name) = splitBrand(translateDeviceModel(row.device.model))
    val status = stringResource(DeviceListLabels.rowStatus(row.status))
    val unpairLabel = stringResource(R.string.unpair_device)
    val enabled = row.tapAction != TapAction.None
    Surface(
        modifier = Modifier.fillMaxWidth()
            .combinedClickable(enabled = enabled, onClick = onTap, onLongClick = onUnpair)
            .semantics(mergeDescendants = true) {
                contentDescription = "$name, $status"
                customActions = listOf(
                    CustomAccessibilityAction(unpairLabel) {
                        onUnpair()
                        true
                    },
                )
            },
        shape = RoundedCornerShape(22.dp),
        color = MaterialTheme.colorScheme.surfaceContainerLowest,
    ) {
        Row(
            Modifier.padding(start = 12.dp, top = 10.dp, bottom = 10.dp, end = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Box(
                Modifier.size(
                    48.dp,
                ).background(MaterialTheme.colorScheme.secondaryContainer, RoundedCornerShape(14.dp)),
                contentAlignment = Alignment.Center,
            ) {
                DeviceArtOrIcon(row.device.model, Modifier.size(44.dp))
            }
            Column(Modifier.weight(1f)) {
                Text(name, style = MaterialTheme.typography.titleMedium)
                Text(
                    status,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            DeviceMenu(row.device, onUnpair)
        }
    }
}

/** Global Bluetooth access state: the saved list stays visible and every row says "unknown", never "not linked". */
@Composable
private fun BluetoothAccessBanner(access: BluetoothAccess, onAction: (BluetoothAccess) -> Unit) {
    val title = DeviceListLabels.accessTitle(access) ?: return
    val hint = DeviceListLabels.accessHint(access)
    val action = DeviceListLabels.accessAction(access)
    Surface(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(22.dp),
        color = MaterialTheme.colorScheme.errorContainer,
        contentColor = MaterialTheme.colorScheme.onErrorContainer,
    ) {
        Row(
            Modifier.padding(start = 16.dp, top = 12.dp, bottom = 12.dp, end = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(
                    stringResource(title),
                    style = MaterialTheme.typography.titleSmall,
                    fontWeight = FontWeight.SemiBold,
                )
                hint?.let {
                    Text(stringResource(it), style = MaterialTheme.typography.bodySmall)
                }
            }
            action?.let {
                TextButton(onClick = { onAction(access) }) {
                    Text(stringResource(it))
                }
            }
        }
    }
}

@Composable
private fun DeviceMenu(device: PairedDevice, onUnpair: () -> Unit, modifier: Modifier = Modifier) {
    var showMenu by remember { mutableStateOf(false) }
    val menuDescription = stringResource(R.string.sound_device_actions, translateDeviceModel(device.model))
    val iconColor = MaterialTheme.colorScheme.onSurfaceVariant
    Box(modifier) {
        IconButton(onClick = { showMenu = true }) {
            Canvas(Modifier.size(24.dp).semantics { contentDescription = menuDescription }) {
                listOf(5f, 12f, 19f).forEach { y ->
                    drawCircle(iconColor, radius = 1.7.dp.toPx(), center = Offset(size.width / 2, size.height * y / 24))
                }
            }
        }
        DropdownMenu(expanded = showMenu, onDismissRequest = { showMenu = false }) {
            DropdownMenuItem(
                text = { Text(stringResource(R.string.unpair_device)) },
                onClick = {
                    showMenu = false
                    onUnpair()
                },
            )
        }
    }
}

@Composable
private fun NoDevicesCard(onAddDeviceClick: () -> Unit) {
    Surface(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(28.dp),
        color = MaterialTheme.colorScheme.secondaryContainer,
        contentColor = MaterialTheme.colorScheme.onSecondaryContainer,
    ) {
        Column(
            Modifier.padding(horizontal = 20.dp, vertical = 24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Icon(
                painterResource(R.drawable.headphones_24px),
                contentDescription = null,
                modifier = Modifier.size(72.dp),
            )
            Text(stringResource(R.string.sound_no_devices_title), style = MaterialTheme.typography.headlineSmall)
            Text(
                stringResource(R.string.sound_pairing_system_hint),
                style = MaterialTheme.typography.bodyMedium,
                textAlign = TextAlign.Center,
            )
            Button(
                onClick = onAddDeviceClick,
                modifier = Modifier.fillMaxWidth().padding(top = 6.dp).heightIn(min = 52.dp),
            ) {
                Icon(painterResource(R.drawable.add_24px), contentDescription = null)
                Spacer(Modifier.size(8.dp))
                Text(stringResource(R.string.sound_add_device))
            }
        }
    }
}

@Preview(showBackground = true)
@Composable
private fun PreviewDeviceListingScreen() {
    val first = PairedDevice("AC:12:2F:C8:6E:08", "SoundcoreA3028", false)
    val second = PairedDevice("AC:12:2F:C8:6E:09", "SoundcoreA3027", false)
    OpenSCQ30Theme(dynamicColor = false) {
        DeviceListingScreen(
            state = DeviceListPresenter.present(
                devices = listOf(first, second),
                phoneLink = PhoneLinkSignals(BluetoothAccess.Ready, profileAddresses = setOf(first.macAddress)),
                appControl = AppControlSnapshot(first.macAddress, AppControl.Controlling),
            ),
            activeDevice = ActiveDevice(first.macAddress, ActiveDeviceState.Connected),
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun PreviewEmptyListingScreen() {
    OpenSCQ30Theme(dynamicColor = false) {
        DeviceListingScreen(
            state = DeviceListPresenter.present(
                devices = emptyList(),
                phoneLink = PhoneLinkSignals(BluetoothAccess.Ready),
                appControl = AppControlSnapshot(),
            ),
        )
    }
}
