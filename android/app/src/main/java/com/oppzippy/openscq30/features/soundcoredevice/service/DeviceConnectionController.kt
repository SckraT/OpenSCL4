package com.oppzippy.openscq30.features.soundcoredevice.service

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Job
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** Owns the native connection for the service, independently of any bound screen. */
internal class DeviceConnectionController(
    private val scope: CoroutineScope,
    private val connectDevice: suspend (String) -> DeviceConnectionManager,
    private val onConnectionFailure: (Exception) -> Unit,
) : AutoCloseable {
    val connectionStatusFlow = MutableStateFlow<ConnectionStatus>(ConnectionStatus.AwaitingConnection)
    private var connectionJob: Job? = null

    fun connect(macAddress: String) {
        val current = connectionStatusFlow.value
        val currentAddress = when (current) {
            is ConnectionStatus.Connecting -> current.macAddress
            is ConnectionStatus.Connected -> current.macAddress
            else -> null
        }
        if (currentAddress.equals(macAddress, ignoreCase = true)) return

        val previousJob = connectionJob
        previousJob?.cancel()
        connectionStatusFlow.value = ConnectionStatus.Connecting(macAddress)
        if (current is ConnectionStatus.Connected) current.deviceManager.close()

        val job = scope.launch(start = CoroutineStart.LAZY) {
            var manager: DeviceConnectionManager? = null
            try {
                // A superseded native connection must finish its cancellation before opening another one.
                withContext(NonCancellable) { previousJob?.join() }
                currentCoroutineContext().ensureActive()
                manager = connectDevice(macAddress)
                currentCoroutineContext().ensureActive()
                connectionStatusFlow.value = ConnectionStatus.Connected(manager, macAddress)
                manager = null // Ownership has moved to connectionStatusFlow.
            } catch (ex: CancellationException) {
                throw ex
            } catch (ex: Exception) {
                currentCoroutineContext().ensureActive()
                connectionStatusFlow.value = ConnectionStatus.Disconnected
                onConnectionFailure(ex)
            } finally {
                // A native backend may finish after cancellation; never leak its returned handle.
                manager?.close()
            }
        }
        connectionJob = job
        job.start()
    }

    override fun close() {
        connectionJob?.cancel()
        // Keep the cancelled job so a retry on a still-bound service waits for native cleanup.
        val current = connectionStatusFlow.value
        connectionStatusFlow.value = ConnectionStatus.Disconnected
        if (current is ConnectionStatus.Connected) current.deviceManager.close()
    }
}
