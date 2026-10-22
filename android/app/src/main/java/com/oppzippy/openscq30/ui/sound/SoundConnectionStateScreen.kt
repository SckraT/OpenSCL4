package com.oppzippy.openscq30.ui.sound

import android.Manifest
import android.annotation.SuppressLint
import android.bluetooth.BluetoothManager
import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.provider.Settings
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
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
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.features.soundcoredevice.service.ConnectionStatus

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SoundConnectionStateScreen(status: ConnectionStatus, onBack: () -> Unit, onRetry: () -> Unit = {}) {
    val connecting = status is ConnectionStatus.Connecting ||
        status is ConnectionStatus.Recovering ||
        status == ConnectionStatus.AwaitingConnection
    val lost = status as? ConnectionStatus.Lost
    val bluetoothDisabled = !connecting && lost == null && rememberBluetoothDisabled()
    val context = LocalContext.current
    Scaffold(topBar = {
        TopAppBar(title = {
            Text(stringResource(R.string.app_name), maxLines = 1, overflow = TextOverflow.Ellipsis)
        }, navigationIcon = {
            IconButton(onClick = onBack) {
                Icon(painterResource(R.drawable.arrow_back_24px), stringResource(R.string.back))
            }
        })
    }) { padding ->
        Column(
            Modifier.padding(padding).fillMaxSize().verticalScroll(rememberScrollState()).padding(32.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(24.dp, Alignment.CenterVertically),
        ) {
            Surface(shape = RoundedCornerShape(32.dp), color = MaterialTheme.colorScheme.primaryContainer) {
                if (connecting) {
                    CircularProgressIndicator(Modifier.padding(30.dp).size(38.dp))
                } else {
                    Icon(
                        painterResource(R.drawable.bluetooth_24px),
                        null,
                        Modifier.padding(28.dp).size(44.dp),
                        tint = MaterialTheme.colorScheme.onPrimaryContainer,
                    )
                }
            }
            Text(
                stringResource(
                    when {
                        connecting -> R.string.sound_overview_connecting
                        bluetoothDisabled -> R.string.sound_permission_bluetooth_disabled
                        else -> R.string.sound_overview_disconnected
                    },
                ),
                style = MaterialTheme.typography.headlineMedium,
            )
            Text(
                stringResource(
                    when {
                        connecting -> R.string.sound_overview_connecting_hint
                        bluetoothDisabled -> R.string.sound_permission_bluetooth_disabled_hint
                        else -> R.string.sound_overview_disconnected_hint
                    },
                ),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            if (lost != null && lost.reason.isNotBlank()) {
                Text(
                    stringResource(R.string.sound_overview_lost_reason, lost.reason),
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            if (bluetoothDisabled) {
                OutlinedButton(onClick = {
                    try {
                        context.startActivity(Intent(Settings.ACTION_BLUETOOTH_SETTINGS))
                    } catch (_: ActivityNotFoundException) {
                        context.startActivity(Intent(Settings.ACTION_SETTINGS))
                    }
                }) {
                    Text(stringResource(R.string.sound_permission_open_bluetooth_settings))
                }
            }
            if (!connecting) Button(onClick = onRetry) { Text(stringResource(R.string.sound_overview_retry)) }
            TextButton(onClick = onBack) { Text(stringResource(R.string.sound_overview_devices)) }
        }
    }
}

/** Recheck after returning from system settings or a permission change. Demo connections are never gated. */
@Composable
private fun rememberBluetoothDisabled(): Boolean {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current
    var disabled by remember(context) { mutableStateOf(isBluetoothDisabled(context)) }
    DisposableEffect(context, lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) disabled = isBluetoothDisabled(context)
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }
    return disabled
}

@SuppressLint("MissingPermission") // Checked before accessing the adapter, including after returning from Settings.
private fun isBluetoothDisabled(context: Context): Boolean {
    val permission = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
        Manifest.permission.BLUETOOTH_CONNECT
    } else {
        Manifest.permission.BLUETOOTH
    }
    if (ContextCompat.checkSelfPermission(context, permission) != PackageManager.PERMISSION_GRANTED) return false
    return try {
        context.getSystemService(BluetoothManager::class.java)?.adapter?.isEnabled == false
    } catch (_: SecurityException) {
        // Permission can be revoked between the check and adapter access.
        false
    }
}
