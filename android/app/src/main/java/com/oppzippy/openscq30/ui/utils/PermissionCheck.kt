package com.oppzippy.openscq30.ui.utils

import android.Manifest
import android.content.Intent
import android.net.Uri
import android.provider.Settings
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.google.accompanist.permissions.ExperimentalPermissionsApi
import com.google.accompanist.permissions.isGranted
import com.google.accompanist.permissions.rememberPermissionState
import com.oppzippy.openscq30.R

@OptIn(ExperimentalPermissionsApi::class)
@Composable
fun PermissionCheck(
    permission: String,
    prompt: String,
    onPermissionGranted: () -> Unit = {},
    content: @Composable () -> Unit,
) {
    val permissionState = rememberPermissionState(permission)
    var hasRequested by rememberSaveable(permission) { mutableStateOf(false) }
    val context = LocalContext.current

    LaunchedEffect(permissionState.status.isGranted) {
        if (permissionState.status.isGranted) {
            onPermissionGranted()
        }
    }

    if (!permissionState.status.isGranted) {
        val isNotification = permission == Manifest.permission.POST_NOTIFICATIONS
        Column(
            modifier = Modifier.fillMaxSize()
                .then(if (isNotification) Modifier else Modifier.safeDrawingPadding())
                .verticalScroll(rememberScrollState())
                .padding(24.dp),
            verticalArrangement = Arrangement.spacedBy(24.dp, Alignment.CenterVertically),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Surface(
                shape = RoundedCornerShape(32.dp),
                color = MaterialTheme.colorScheme.primaryContainer,
                contentColor = MaterialTheme.colorScheme.onPrimaryContainer,
            ) {
                Icon(
                    painterResource(if (isNotification) R.drawable.notifications_24px else R.drawable.bluetooth_24px),
                    contentDescription = null,
                    modifier = Modifier.padding(28.dp).size(48.dp),
                )
            }
            Column(
                verticalArrangement = Arrangement.spacedBy(12.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Text(
                    prompt,
                    modifier = Modifier.semantics { heading() },
                    style = MaterialTheme.typography.headlineMedium,
                    textAlign = TextAlign.Center,
                )
                Text(
                    stringResource(
                        if (isNotification) {
                            R.string.sound_permission_notification_purpose
                        } else {
                            R.string.sound_permission_bluetooth_purpose
                        },
                    ),
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center,
                )
            }
            Column(
                modifier = Modifier.fillMaxWidth(),
                verticalArrangement = Arrangement.spacedBy(8.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Button(
                    modifier = Modifier.fillMaxWidth().heightIn(min = 56.dp),
                    onClick = {
                        hasRequested = true
                        permissionState.launchPermissionRequest()
                    },
                ) {
                    Text(stringResource(R.string.request_permission))
                }
                if (hasRequested) {
                    Text(
                        stringResource(R.string.sound_permission_settings_hint),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        textAlign = TextAlign.Center,
                    )
                    TextButton(onClick = {
                        context.startActivity(
                            Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS).apply {
                                data = Uri.fromParts("package", context.packageName, null)
                            },
                        )
                    }) {
                        Text(stringResource(R.string.sound_permission_open_settings))
                    }
                }
            }
        }
    } else {
        content()
    }
}
