package com.oppzippy.openscq30.ui.sound

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.lib.bindings.translateDeviceModel
import com.oppzippy.openscq30.lib.bindings.translateSettingId
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.ui.deviceselection.ActiveDeviceState
import com.oppzippy.openscq30.ui.deviceselection.DeviceBattery
import com.oppzippy.openscq30.ui.deviceselection.splitBrand
import com.oppzippy.openscq30.ui.utils.sharedDeviceCard
import com.oppzippy.openscq30.ui.utils.sharedDevicePart

private val ConnectedDot = Color(0xFF3F9A52)

/**
 * The device's card, the same on My devices and on top of the device overview: brand, name, connection state,
 * illustration and batteries, with what each screen adds below it in [footer].
 */
@Composable
fun DeviceHeroCard(
    model: String,
    isDemo: Boolean,
    state: ActiveDeviceState?,
    batteries: List<DeviceBattery>,
    modifier: Modifier = Modifier,
    macAddress: String? = null,
    oneEarbudBadge: Boolean = false,
    hint: String? = null,
    onClick: (() -> Unit)? = null,
    topEnd: @Composable () -> Unit = {},
    footer: @Composable ColumnScope.() -> Unit = {},
) {
    val (brand, name) = splitBrand(translateDeviceModel(model))
    val largeText = LocalDensity.current.fontScale > 1.4f
    val content: @Composable () -> Unit = {
        Box {
            Column(Modifier.padding(start = 20.dp, end = 20.dp, top = 16.dp, bottom = 20.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        DeviceBrand(brand, isDemo)
                        Text(
                            name,
                            modifier = Modifier.sharedDevicePart(macAddress, "name"),
                            style = MaterialTheme.typography.headlineMedium.copy(fontSize = 26.sp, lineHeight = 30.sp),
                            fontWeight = FontWeight.Medium,
                        )
                        Spacer(Modifier.size(6.dp))
                        // the badge keeps a full-size touch target, so the row centers its items instead of lining up
                        // their tops
                        FlowRow(
                            horizontalArrangement = Arrangement.spacedBy(6.dp),
                            verticalArrangement = Arrangement.spacedBy(6.dp),
                            itemVerticalAlignment = Alignment.CenterVertically,
                        ) {
                            DeviceStatusPill(state)
                            if (oneEarbudBadge) OneEarbudBadge()
                        }
                    }
                    if (!largeText) {
                        DeviceArtOrIcon(
                            model,
                            dimmed = state != ActiveDeviceState.Connected,
                            modifier = Modifier.sharedDevicePart(macAddress, "art").size(132.dp),
                        )
                    }
                }
                when {
                    batteries.isNotEmpty() -> BatteryTiles(batteries)

                    hint != null -> Text(
                        hint,
                        modifier = Modifier.padding(vertical = 14.dp),
                        style = MaterialTheme.typography.bodyMedium,
                    )

                    else -> Spacer(Modifier.size(16.dp))
                }
                footer()
            }
            Box(Modifier.align(Alignment.TopEnd).padding(4.dp)) { topEnd() }
        }
    }
    val cardModifier = modifier.sharedDeviceCard(macAddress).fillMaxWidth()
    val shape = RoundedCornerShape(28.dp)
    if (onClick != null) {
        Surface(
            onClick = onClick,
            modifier = cardModifier,
            shape = shape,
            color = MaterialTheme.colorScheme.secondaryContainer,
            contentColor = MaterialTheme.colorScheme.onSecondaryContainer,
            content = content,
        )
    } else {
        Surface(
            modifier = cardModifier,
            shape = shape,
            color = MaterialTheme.colorScheme.secondaryContainer,
            contentColor = MaterialTheme.colorScheme.onSecondaryContainer,
            content = content,
        )
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
internal fun DeviceStatusPill(state: ActiveDeviceState?) {
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

/** "1 earbud" next to the connection state; tapping it explains what is unavailable. */
@Composable
private fun OneEarbudBadge() {
    var explain by remember { mutableStateOf(false) }
    if (explain) {
        AlertDialog(
            onDismissRequest = { explain = false },
            title = { Text(stringResource(R.string.sound_overview_one_earbud_title)) },
            text = { Text(stringResource(R.string.sound_overview_one_earbud_message)) },
            confirmButton = { TextButton(onClick = { explain = false }) { Text(stringResource(R.string.close)) } },
        )
    }
    Surface(
        onClick = { explain = true },
        shape = RoundedCornerShape(14.dp),
        color = MaterialTheme.colorScheme.tertiaryContainer,
        contentColor = MaterialTheme.colorScheme.onTertiaryContainer,
    ) {
        // the same size as the status pill beside it
        Row(
            Modifier.padding(horizontal = 11.dp, vertical = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Icon(painterResource(R.drawable.earbuds_2_24px), null, Modifier.size(14.dp))
            Text(stringResource(R.string.sound_one_earbud_badge), style = MaterialTheme.typography.labelMedium)
        }
    }
}

@Composable
private fun BatteryTiles(batteries: List<DeviceBattery>) {
    Row(
        Modifier.fillMaxWidth().padding(vertical = 14.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        batteries.take(3).forEach { battery ->
            val label = stringResource(battery.label)
            Column(
                Modifier.weight(1f)
                    .background(
                        MaterialTheme.colorScheme.surfaceContainerLowest.copy(alpha = 0.7f),
                        RoundedCornerShape(14.dp),
                    )
                    .padding(horizontal = 10.dp, vertical = 8.dp)
                    .clearAndSetSemantics { contentDescription = "$label: ${battery.value}" },
                verticalArrangement = Arrangement.spacedBy(2.dp),
            ) {
                Text(label, style = MaterialTheme.typography.labelSmall)
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

/** On/off settings as chips under the batteries, and a chip to choose which ones. */
@Composable
internal fun QuickToggleRow(
    toggles: List<Pair<String, Setting.ToggleSetting>>,
    available: List<String>,
    onToggle: (String, Boolean) -> Unit,
    onChoose: (List<String>) -> Unit,
) {
    var choosing by remember { mutableStateOf(false) }
    if (choosing) {
        QuickTogglePicker(
            available = available,
            selected = toggles.map { it.first },
            onDismiss = { choosing = false },
            onConfirm = {
                choosing = false
                onChoose(it)
            },
        )
    }
    FlowRow(
        Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        // the card is the color a selected chip would have, so an enabled toggle is filled with the primary color
        val colors = FilterChipDefaults.filterChipColors(
            selectedContainerColor = MaterialTheme.colorScheme.primary,
            selectedLabelColor = MaterialTheme.colorScheme.onPrimary,
            selectedLeadingIconColor = MaterialTheme.colorScheme.onPrimary,
        )
        toggles.forEach { (id, setting) ->
            FilterChip(
                selected = setting.value,
                onClick = { onToggle(id, !setting.value) },
                label = { Text(translateSettingId(id)) },
                leadingIcon = if (setting.value) {
                    { Icon(painterResource(R.drawable.check_24px), null, Modifier.size(18.dp)) }
                } else {
                    null
                },
                colors = colors,
            )
        }
        FilterChip(
            selected = false,
            onClick = { choosing = true },
            label = { Text(if (toggles.isEmpty()) stringResource(R.string.sound_quick_toggles_add) else "+") },
        )
    }
}

@Composable
private fun QuickTogglePicker(
    available: List<String>,
    selected: List<String>,
    onDismiss: () -> Unit,
    onConfirm: (List<String>) -> Unit,
) {
    var chosen by remember { mutableStateOf(selected) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.sound_quick_toggles)) },
        text = {
            LazyColumn {
                items(available) { id ->
                    val checked = id in chosen
                    Row(
                        Modifier.fillMaxWidth().toggleable(checked, role = Role.Checkbox) {
                            chosen = if (it) chosen + id else chosen - id
                        }.padding(vertical = 4.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Checkbox(checked = checked, onCheckedChange = null)
                        Text(translateSettingId(id), Modifier.padding(start = 12.dp))
                    }
                }
            }
        },
        confirmButton = {
            // keep the order of the settings list, which the device defines
            TextButton(onClick = { onConfirm(available.filter { it in chosen }) }) {
                Text(stringResource(R.string.sound_done))
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}
