package com.oppzippy.openscq30.ui.deviceselection

/**
 * Where the user is between "My devices" and the device screen. Tapping a linked device on the list takes control
 * without leaving the list; the device screen opens only once the app actually controls it.
 */
data class DeviceNavigationState(
    val showDeviceList: Boolean = false,
    val requestedMacAddress: String? = null,
    val listConnectMacAddress: String? = null,
) {
    val showDevice: Boolean
        get() = !showDeviceList && when {
            listConnectMacAddress != null ->
                requestedMacAddress.equals(listConnectMacAddress, ignoreCase = true)

            else -> requestedMacAddress != null
        }

    fun openDevice(macAddress: String): DeviceNavigationState = copy(
        showDeviceList = false,
        requestedMacAddress = macAddress,
        listConnectMacAddress = null,
    )

    fun requestControl(macAddress: String): DeviceNavigationState = copy(
        showDeviceList = true,
        listConnectMacAddress = macAddress,
    )

    fun showList(): DeviceNavigationState = copy(showDeviceList = true)

    fun clearDevice(): DeviceNavigationState = copy(requestedMacAddress = null, listConnectMacAddress = null)

    fun forgetDevice(macAddress: String): DeviceNavigationState = copy(
        requestedMacAddress = requestedMacAddress?.takeUnless { it.equals(macAddress, ignoreCase = true) },
        listConnectMacAddress = listConnectMacAddress?.takeUnless { it.equals(macAddress, ignoreCase = true) },
    )

    fun onAppControl(control: AppControlSnapshot): DeviceNavigationState = when (control.phase) {
        AppControl.Controlling -> {
            val address = control.macAddress
            if (address != null && address.equals(listConnectMacAddress, ignoreCase = true)) {
                copy(
                    showDeviceList = false,
                    requestedMacAddress = address,
                    listConnectMacAddress = null,
                )
            } else {
                copy(requestedMacAddress = address ?: requestedMacAddress)
            }
        }

        AppControl.Connecting -> {
            if (listConnectMacAddress != null && control.macAddress.equals(listConnectMacAddress, true)) {
                this
            } else {
                copy(requestedMacAddress = control.macAddress ?: requestedMacAddress)
            }
        }

        AppControl.NotControlling -> if (listConnectMacAddress != null) {
            copy(listConnectMacAddress = null)
        } else {
            this
        }
    }
}
