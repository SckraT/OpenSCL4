package com.oppzippy.openscq30.ui

import android.content.ComponentName
import android.content.ServiceConnection
import android.os.IBinder
import com.oppzippy.openscq30.features.soundcoredevice.service.ConnectionStatus
import com.oppzippy.openscq30.features.soundcoredevice.service.DeviceService
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch

class DeviceServiceConnection(private val onBindingLost: () -> Unit = {}) :
    ServiceConnection,
    AutoCloseable {
    val connectionStatusFlow = MutableStateFlow<ConnectionStatus>(ConnectionStatus.Disconnected)
    private var serviceConnectionScope: CoroutineScope? = null

    override fun onServiceConnected(name: ComponentName?, binder: IBinder?) {
        val myServiceBinder = binder as? DeviceService.MyBinder ?: run {
            onNullBinding(name)
            return
        }
        val service = myServiceBinder.getService()

        serviceConnectionScope?.cancel()
        serviceConnectionScope = CoroutineScope(Job() + Dispatchers.Main)

        serviceConnectionScope?.launch {
            service.connectionStatusFlow.collect { connectionStatus ->
                this@DeviceServiceConnection.connectionStatusFlow.value = connectionStatus
            }
        }
    }

    override fun onBindingDied(name: ComponentName?) {
        onServiceDisconnected(name)
    }

    override fun onNullBinding(name: ComponentName?) {
        onServiceDisconnected(name)
    }

    override fun onServiceDisconnected(name: ComponentName?) {
        close()
        onBindingLost()
    }

    override fun close() {
        serviceConnectionScope?.cancel()
        serviceConnectionScope = null
        connectionStatusFlow.value = ConnectionStatus.Disconnected
    }
}
