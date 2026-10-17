package com.oppzippy.openscq30.ui.deviceselection.screens

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
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
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
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
import androidx.compose.ui.unit.sp
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.lib.bindings.translateDeviceModel
import com.oppzippy.openscq30.lib.wrapper.PairedDevice
import com.oppzippy.openscq30.ui.deviceselection.ActiveDevice
import com.oppzippy.openscq30.ui.deviceselection.ActiveDeviceState
import com.oppzippy.openscq30.ui.deviceselection.DeviceBattery
import com.oppzippy.openscq30.ui.deviceselection.featuredDevice
import com.oppzippy.openscq30.ui.deviceselection.splitBrand
import com.oppzippy.openscq30.ui.sound.DeviceArtOrIcon
import com.oppzippy.openscq30.ui.theme.OpenSCQ30Theme
import kotlinx.coroutines.launch

private val ConnectedDot = Color(0xFF3F9A52)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DeviceListingScreen(
    devices: List<PairedDevice>?,
    activeDevice: ActiveDevice? = null,
    lastDeviceMacAddress: String? = null,
    onRefreshClick: () -> Unit = {},
    onInfoClick: () -> Unit = {},
    onDeviceClick: (PairedDevice) -> Unit = {},
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
                val featured = devices?.let { featuredDevice(it, activeDevice?.macAddress, lastDeviceMacAddress) }
                if (devices == null) {
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
                    val featuredState = activeDevice
                        ?.takeIf { it.macAddress.equals(featured.macAddress, ignoreCase = true) }
                    item(key = "featured") {
                        FeaturedDeviceCard(
                            device = featured,
                            state = featuredState?.state,
                            batteries = featuredState?.batteries.orEmpty(),
                            onOpen = { onDeviceClick(featured) },
                            onUnpair = { deviceToUnpair = featured },
                        )
                    }
                    val others = devices.filter { it != featured }
                    if (others.isNotEmpty()) {
                        item {
                            Text(
                                stringResource(R.string.sound_other_devices),
                                modifier = Modifier.padding(start = 8.dp, top = 8.dp),
                                style = MaterialTheme.typography.labelLarge,
                                color = MaterialTheme.colorScheme.primary,
                            )
                        }
                        items(others, key = { "${it.model}:${it.macAddress}:${it.isDemo}" }) { device ->
                            DeviceRow(
                                device = device,
                                onClick = { onDeviceClick(device) },
                                onUnpair = { deviceToUnpair = device },
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

/** The device the app is connected to, or otherwise the one used last, with its state and what to do next. */
@Composable
private fun FeaturedDeviceCard(
    device: PairedDevice,
    state: ActiveDeviceState?,
    batteries: List<DeviceBattery>,
    onOpen: () -> Unit,
    onUnpair: () -> Unit,
) {
    val (brand, name) = splitBrand(translateDeviceModel(device.model))
    // Tapping anywhere on the card does what its button does, as on the rows below it.
    Surface(
        onClick = onOpen,
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(28.dp),
        color = MaterialTheme.colorScheme.secondaryContainer,
        contentColor = MaterialTheme.colorScheme.onSecondaryContainer,
    ) {
        Box {
            Column(Modifier.padding(start = 20.dp, end = 20.dp, top = 16.dp, bottom = 20.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        DeviceBrand(brand, device.isDemo)
                        Text(
                            name,
                            style = MaterialTheme.typography.headlineMedium.copy(fontSize = 26.sp, lineHeight = 30.sp),
                            fontWeight = FontWeight.Medium,
                        )
                        Spacer(Modifier.size(6.dp))
                        DeviceStatusPill(state)
                    }
                    DeviceArtOrIcon(
                        device.model,
                        dimmed = state != ActiveDeviceState.Connected,
                        modifier = Modifier.size(132.dp),
                    )
                }
                when {
                    state == ActiveDeviceState.Connected && batteries.isNotEmpty() -> BatteryTiles(batteries)
                    state == null -> Text(
                        stringResource(R.string.sound_device_not_connected_hint),
                        modifier = Modifier.padding(vertical = 14.dp),
                        style = MaterialTheme.typography.bodyMedium,
                    )
                    else -> Spacer(Modifier.size(16.dp))
                }
                Button(
                    onClick = onOpen,
                    enabled = state != ActiveDeviceState.Connecting,
                    modifier = Modifier.fillMaxWidth().heightIn(min = 52.dp),
                ) {
                    Text(
                        stringResource(
                            when (state) {
                                ActiveDeviceState.Connected -> R.string.sound_open_device
                                ActiveDeviceState.Connecting -> R.string.sound_device_connecting
                                null -> R.string.sound_device_connect
                            },
                        ),
                    )
                }
            }
            DeviceMenu(device, onUnpair, Modifier.align(Alignment.TopEnd).padding(4.dp))
        }
    }
}

@Composable
private fun DeviceBrand(brand: String?, isDemo: Boolean) {
    val text = listOfNotNull(brand, if (isDemo) stringResource(R.string.demo) else null).joinToString(" · ")
    if (text.isNotEmpty()) {
        Text(
            text.uppercase(),
            style = MaterialTheme.typography.labelSmall,
            modifier = Modifier.alpha(0.75f),
        )
    }
}

@Composable
private fun DeviceStatusPill(state: ActiveDeviceState?) {
    Row(
        Modifier.background(
            MaterialTheme.colorScheme.onSecondaryContainer.copy(alpha = 0.1f),
            RoundedCornerShape(14.dp),
        ).padding(horizontal = 11.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(7.dp),
    ) {
        when (state) {
            ActiveDeviceState.Connected -> Box(Modifier.size(8.dp).background(ConnectedDot, CircleShape))
            ActiveDeviceState.Connecting -> CircularProgressIndicator(
                modifier = Modifier.size(11.dp),
                strokeWidth = 2.dp,
                color = MaterialTheme.colorScheme.onSecondaryContainer,
            )
            null -> Box(
                Modifier.size(9.dp).border(
                    1.5.dp,
                    MaterialTheme.colorScheme.onSecondaryContainer.copy(alpha = 0.7f),
                    CircleShape,
                ),
            )
        }
        Text(
            stringResource(
                when (state) {
                    ActiveDeviceState.Connected -> R.string.connected
                    ActiveDeviceState.Connecting -> R.string.sound_device_connecting
                    null -> R.string.sound_device_not_connected
                },
            ),
            style = MaterialTheme.typography.labelMedium,
        )
    }
}

@Composable
private fun BatteryTiles(batteries: List<DeviceBattery>) {
    Row(
        Modifier.fillMaxWidth().padding(vertical = 14.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        batteries.take(3).forEach { battery ->
            Column(
                Modifier.weight(1f)
                    .background(
                        MaterialTheme.colorScheme.surfaceContainerLowest.copy(alpha = 0.7f),
                        RoundedCornerShape(14.dp),
                    )
                    .padding(horizontal = 10.dp, vertical = 8.dp),
                verticalArrangement = Arrangement.spacedBy(2.dp),
            ) {
                Text(stringResource(battery.label), style = MaterialTheme.typography.labelSmall)
                Text(battery.value, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
                battery.progress?.let { progress ->
                    LinearProgressIndicator(
                        progress = { progress },
                        modifier = Modifier.fillMaxWidth().padding(top = 4.dp),
                        color = MaterialTheme.colorScheme.onSecondaryContainer,
                        trackColor = MaterialTheme.colorScheme.onSecondaryContainer.copy(alpha = 0.15f),
                        drawStopIndicator = {},
                    )
                }
            }
        }
    }
}

/** Other saved devices, compact; tapping one connects to it. */
@Composable
private fun DeviceRow(device: PairedDevice, onClick: () -> Unit, onUnpair: () -> Unit) {
    val (_, name) = splitBrand(translateDeviceModel(device.model))
    val unpairLabel = stringResource(R.string.unpair_device)
    Surface(
        modifier = Modifier.fillMaxWidth()
            .combinedClickable(onClick = onClick, onLongClick = onUnpair)
            .semantics { customActions = listOf(CustomAccessibilityAction(unpairLabel) { onUnpair(); true }) },
        shape = RoundedCornerShape(22.dp),
        color = MaterialTheme.colorScheme.surfaceContainerLowest,
    ) {
        Row(
            Modifier.padding(start = 12.dp, top = 10.dp, bottom = 10.dp, end = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Box(
                Modifier.size(48.dp).background(MaterialTheme.colorScheme.secondaryContainer, RoundedCornerShape(14.dp)),
                contentAlignment = Alignment.Center,
            ) {
                DeviceArtOrIcon(device.model, Modifier.size(44.dp))
            }
            Column(Modifier.weight(1f)) {
                Text(name, style = MaterialTheme.typography.titleMedium)
                Text(
                    stringResource(if (device.isDemo) R.string.demo else R.string.sound_device_not_connected),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            DeviceMenu(device, onUnpair)
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
            Icon(painterResource(R.drawable.headphones_24px), contentDescription = null, modifier = Modifier.size(72.dp))
            Text(stringResource(R.string.sound_no_devices_title), style = MaterialTheme.typography.headlineSmall)
            Text(
                stringResource(R.string.sound_pairing_system_hint),
                style = MaterialTheme.typography.bodyMedium,
                textAlign = TextAlign.Center,
            )
            Button(onClick = onAddDeviceClick, modifier = Modifier.fillMaxWidth().padding(top = 6.dp).heightIn(min = 52.dp)) {
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
    OpenSCQ30Theme(dynamicColor = false) {
        DeviceListingScreen(
            listOf(
                PairedDevice("AC:12:2F:C8:6E:08", "SoundcoreA3028", false),
                PairedDevice("AC:12:2F:C8:6E:09", "SoundcoreA3027", false),
            ),
            activeDevice = ActiveDevice("AC:12:2F:C8:6E:08", ActiveDeviceState.Connected),
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun PreviewEmptyListingScreen() {
    OpenSCQ30Theme(dynamicColor = false) { DeviceListingScreen(emptyList()) }
}
