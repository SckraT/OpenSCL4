package com.oppzippy.openscq30.features.phonelink

import java.util.Locale

/** Whether this phone currently has a Bluetooth link to a device. */
enum class PhoneBluetoothLink { Linked, NotLinked, Unknown }

/** Why the phone link can or cannot be observed right now. */
enum class BluetoothAccess { Ready, Off, PermissionDenied, Unsupported, Unknown }

/** Raw phone-link signals: a profile scan seed and live ACL events. */
data class PhoneLinkSignals(
    val access: BluetoothAccess = BluetoothAccess.Unknown,
    val profileAddresses: Set<String> = emptySet(),
    val aclAddresses: Set<String> = emptySet(),
)

/**
 * The effective phone link. [linkedAddresses] contains only addresses observed by a profile scan or ACL event.
 * [controlledAddress] is an address with an active app session: it corroborates a link but does not claim one was
 * observed, so it is reported as [PhoneBluetoothLink.Unknown] rather than [PhoneBluetoothLink.Linked].
 */
data class PhoneLinkSnapshot(
    val access: BluetoothAccess,
    val linkedAddresses: Set<String>,
    val controlledAddress: String? = null,
) {
    fun linkOf(macAddress: String): PhoneBluetoothLink {
        val mac = canonicalMacAddress(macAddress)
        return when {
            access != BluetoothAccess.Ready -> PhoneBluetoothLink.Unknown
            mac in linkedAddresses -> PhoneBluetoothLink.Linked
            mac == controlledAddress -> PhoneBluetoothLink.Unknown
            else -> PhoneBluetoothLink.NotLinked
        }
    }
}

object PhoneLinkReducer {
    fun reduce(
        access: BluetoothAccess,
        profileAddresses: Collection<String>,
        aclAddresses: Collection<String>,
        controlledAddress: String?,
    ): PhoneLinkSnapshot {
        val controlled = controlledAddress?.let(::canonicalMacAddress)
        if (access != BluetoothAccess.Ready) {
            return PhoneLinkSnapshot(access, emptySet(), controlled)
        }
        val linkedAddresses = buildSet {
            profileAddresses.mapTo(this) { canonicalMacAddress(it) }
            aclAddresses.mapTo(this) { canonicalMacAddress(it) }
        }
        return PhoneLinkSnapshot(access, linkedAddresses, controlled)
    }
}

internal fun canonicalMacAddress(macAddress: String): String = macAddress.trim().lowercase(Locale.ROOT)
