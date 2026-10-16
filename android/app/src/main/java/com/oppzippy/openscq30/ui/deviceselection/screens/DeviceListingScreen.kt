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
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.lib.bindings.translateDeviceModel
import com.oppzippy.openscq30.lib.wrapper.PairedDevice
import com.oppzippy.openscq30.ui.theme.OpenSCQ30Theme
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DeviceListingScreen(
    devices: List<PairedDevice>?,
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
                contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = 20.dp, bottom = 32.dp),
                verticalArrangement = Arrangement.spacedBy(20.dp),
            ) {
                item {
                    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Text(
                            stringResource(R.string.sound_your_devices),
                            style = MaterialTheme.typography.headlineMedium,
                        )
                        Text(
                            stringResource(R.string.sound_your_devices_description),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
                item {
                    Surface(
                        modifier = Modifier.fillMaxWidth(),
                        shape = MaterialTheme.shapes.extraLarge,
                        color = MaterialTheme.colorScheme.secondaryContainer,
                        contentColor = MaterialTheme.colorScheme.onSecondaryContainer,
                    ) {
                        Box(Modifier.height(164.dp), contentAlignment = Alignment.Center) {
                            Icon(
                                painterResource(R.drawable.earbuds_2_24px),
                                contentDescription = null,
                                modifier = Modifier.size(88.dp),
                            )
                        }
                    }
                }
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
                } else if (devices.isEmpty()) {
                    item {
                        Surface(
                            shape = MaterialTheme.shapes.medium,
                            color = MaterialTheme.colorScheme.surfaceContainerLowest,
                        ) {
                            Column(
                                Modifier.fillMaxWidth().padding(24.dp),
                                verticalArrangement = Arrangement.spacedBy(8.dp),
                            ) {
                                Text(
                                    stringResource(R.string.sound_no_devices_yet),
                                    style = MaterialTheme.typography.titleLarge,
                                )
                                Text(
                                    stringResource(R.string.sound_add_first_device),
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        }
                    }
                } else {
                    items(devices, key = { "${it.model}:${it.macAddress}:${it.isDemo}" }) { device ->
                        SavedDeviceCard(
                            device = device,
                            onClick = { onDeviceClick(device) },
                            onUnpair = { deviceToUnpair = device },
                        )
                    }
                }
                item {
                    Button(
                        modifier = Modifier.fillMaxWidth(),
                        onClick = onAddDeviceClick,
                        colors = ButtonDefaults.buttonColors(
                            containerColor = MaterialTheme.colorScheme.primaryContainer,
                            contentColor = MaterialTheme.colorScheme.onPrimaryContainer,
                        ),
                        contentPadding = PaddingValues(horizontal = 20.dp, vertical = 16.dp),
                    ) {
                        Icon(painterResource(R.drawable.add_24px), contentDescription = stringResource(R.string.add))
                        Spacer(Modifier.size(8.dp))
                        Text(stringResource(R.string.sound_add_device))
                    }
                }
                item {
                    Text(
                        stringResource(R.string.sound_pairing_system_hint),
                        modifier = Modifier.padding(horizontal = 4.dp),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
}

@Composable
private fun SavedDeviceCard(device: PairedDevice, onClick: () -> Unit, onUnpair: () -> Unit) {
    var showMenu by remember { mutableStateOf(false) }
    val menuDescription = stringResource(R.string.sound_device_actions, translateDeviceModel(device.model))
    val iconColor = MaterialTheme.colorScheme.onSurfaceVariant
    Surface(
        modifier = Modifier.fillMaxWidth().combinedClickable(onClick = onClick, onLongClick = onUnpair),
        shape = MaterialTheme.shapes.extraLarge,
        color = MaterialTheme.colorScheme.surfaceContainerLowest,
    ) {
        Column(Modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    translateDeviceModel(device.model),
                    modifier = Modifier.weight(1f),
                    style = MaterialTheme.typography.titleLarge,
                )
                Box {
                    IconButton(onClick = { showMenu = true }) {
                        Canvas(Modifier.size(24.dp).semantics { contentDescription = menuDescription }) {
                            listOf(5f, 12f, 19f).forEach { y ->
                                drawCircle(
                                    iconColor,
                                    radius = 1.7.dp.toPx(),
                                    center = Offset(
                                        size.width / 2,
                                        size.height * y / 24,
                                    ),
                                )
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
            Text(
                device.macAddress,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(
                    if (device.isDemo) stringResource(R.string.demo) else stringResource(R.string.sound_saved_device),
                    modifier = Modifier.weight(
                        1f,
                    ).background(MaterialTheme.colorScheme.secondaryContainer, RoundedCornerShape(16.dp))
                        .padding(horizontal = 12.dp, vertical = 8.dp),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSecondaryContainer,
                )
                Button(onClick = onClick, contentPadding = PaddingValues(horizontal = 20.dp, vertical = 14.dp)) {
                    Text(stringResource(R.string.sound_open_device))
                }
            }
        }
    }
}

@Preview(showBackground = true)
@Composable
private fun PreviewDeviceListingScreen() {
    OpenSCQ30Theme(dynamicColor = false) {
        DeviceListingScreen(listOf(PairedDevice("AC:12:2F:C8:6E:08", "SoundcoreA3954", false)))
    }
}

@Preview(showBackground = true)
@Composable
private fun PreviewEmptyListingScreen() {
    OpenSCQ30Theme(dynamicColor = false) { DeviceListingScreen(emptyList()) }
}
