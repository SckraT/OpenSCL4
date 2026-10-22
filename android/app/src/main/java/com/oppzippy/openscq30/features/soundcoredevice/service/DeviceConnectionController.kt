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
    private var sessionStatusJob: Job? = null

    fun connect(macAddress: String) = connect(macAddress, replaceLost = false)

    /**
     * A manual retry only applies after [ConnectionStatus.Lost]. During [ConnectionStatus.Recovering] the
     * native session is already retrying and a second session must not be opened.
     */
    fun retry() {
        val current = connectionStatusFlow.value
        if (current is ConnectionStatus.Lost) connect(current.macAddress, replaceLost = true)
    }

    private fun connect(macAddress: String, replaceLost: Boolean) {
        val current = connectionStatusFlow.value
        if (!replaceLost && current.isSameSession(macAddress)) return

        val previousJob = connectionJob
        previousJob?.cancel()
        sessionStatusJob?.cancel()
        connectionStatusFlow.value = ConnectionStatus.Connecting(macAddress)
        current.deviceManager?.close()

        val job = scope.launch(start = CoroutineStart.LAZY) {
            var manager: DeviceConnectionManager? = null
            try {
                // A superseded native connection must finish its cancellation before opening another one.
                withContext(NonCancellable) { previousJob?.join() }
                currentCoroutineContext().ensureActive()
                manager = connectDevice(macAddress)
                currentCoroutineContext().ensureActive()
                val connectedManager = checkNotNull(manager)
                connectionStatusFlow.value = ConnectionStatus.Connected(connectedManager, macAddress)
                monitorSession(connectedManager, macAddress)
                manager = null // Ownership has moved to connectionStatusFlow.
            } catch (ex: CancellationException) {
                throw ex
            } catch (ex: Exception) {
                currentCoroutineContext().ensureActive()
                // The initial connect shares the recovery budget, so its failure is lost, not an explicit close.
                connectionStatusFlow.value = ConnectionStatus.Lost(
                    deviceManager = null,
                    macAddress = macAddress,
                    reason = ex.message ?: "connection failed",
                )
                onConnectionFailure(ex)
            } finally {
                // A native backend may finish after cancellation; never leak its returned handle.
                manager?.close()
            }
        }
        connectionJob = job
        job.start()
    }

    private fun monitorSession(manager: DeviceConnectionManager, macAddress: String) {
        sessionStatusJob = scope.launch {
            manager.sessionStatusFlow.collect { state ->
                // A monitor for a superseded manager must not overwrite the current connection.
                if (connectionStatusFlow.value.deviceManager !== manager) return@collect
                connectionStatusFlow.value = when (state) {
                    SessionConnectionState.Connected -> ConnectionStatus.Connected(manager, macAddress)

                    is SessionConnectionState.Recovering -> ConnectionStatus.Recovering(
                        manager,
                        macAddress,
                        state.attempt,
                        state.reason,
                    )

                    is SessionConnectionState.Lost -> ConnectionStatus.Lost(manager, macAddress, state.reason)

                    SessionConnectionState.Connecting, SessionConnectionState.Closed -> connectionStatusFlow.value
                }
            }
        }
    }

    override fun close() {
        connectionJob?.cancel()
        sessionStatusJob?.cancel()
        // Keep the cancelled job so a retry on a still-bound service waits for native cleanup.
        val current = connectionStatusFlow.value
        connectionStatusFlow.value = ConnectionStatus.Disconnected
        current.deviceManager?.close()
    }
}

private fun ConnectionStatus.isSameSession(macAddress: String): Boolean = when (this) {
    is ConnectionStatus.Connecting -> this.macAddress.equals(macAddress, ignoreCase = true)
    is ConnectionStatus.Connected -> this.macAddress.equals(macAddress, ignoreCase = true)
    is ConnectionStatus.Recovering -> this.macAddress.equals(macAddress, ignoreCase = true)
    else -> false
}
