package com.oppzippy.openscq30.features.phonelink

import org.junit.Assert.assertEquals
import org.junit.Test

class PhoneLinkReducerTest {
    private val liberty = "F4:9D:8A:7C:99:06"
    private val q30 = "AC:12:2F:C8:6E:08"

    @Test
    fun profileAndAclLinksAreMerged() {
        val snapshot = reduce(profile = setOf(liberty), acl = setOf(q30))

        assertEquals(BluetoothAccess.Ready, snapshot.access)
        assertEquals(setOf("f4:9d:8a:7c:99:06", "ac:12:2f:c8:6e:08"), snapshot.linkedAddresses)
    }

    @Test
    fun macAddressesAreCaseFolded() {
        val snapshot = reduce(profile = setOf(liberty, liberty.lowercase()), acl = setOf(q30.lowercase()))

        assertEquals(setOf("f4:9d:8a:7c:99:06", "ac:12:2f:c8:6e:08"), snapshot.linkedAddresses)
        assertEquals(PhoneBluetoothLink.Linked, snapshot.linkOf("f4:9D:8a:7C:99:06"))
        assertEquals(PhoneBluetoothLink.NotLinked, snapshot.linkOf("00:00:00:00:00:00"))
    }

    @Test
    fun anActiveSessionCorroboratesALinkWithoutClaimingItWasObserved() {
        val snapshot = reduce(profile = emptySet(), acl = emptySet(), controlled = liberty)

        assertEquals(emptySet<String>(), snapshot.linkedAddresses)
        assertEquals(PhoneBluetoothLink.Unknown, snapshot.linkOf(liberty))
        assertEquals(PhoneBluetoothLink.NotLinked, snapshot.linkOf(q30))
    }

    @Test
    fun accessOtherThanReadyMakesEveryLinkUnknown() {
        val inaccessible = listOf(
            BluetoothAccess.Off,
            BluetoothAccess.PermissionDenied,
            BluetoothAccess.Unsupported,
            BluetoothAccess.Unknown,
        )

        for (access in inaccessible) {
            val snapshot = reduce(access = access, profile = setOf(liberty), acl = setOf(q30), controlled = liberty)

            assertEquals(access, snapshot.access)
            assertEquals(emptySet<String>(), snapshot.linkedAddresses)
            assertEquals(PhoneBluetoothLink.Unknown, snapshot.linkOf(liberty))
            assertEquals(PhoneBluetoothLink.Unknown, snapshot.linkOf(q30))
        }
    }

    private fun reduce(
        access: BluetoothAccess = BluetoothAccess.Ready,
        profile: Set<String>,
        acl: Set<String> = emptySet(),
        controlled: String? = null,
    ): PhoneLinkSnapshot = PhoneLinkReducer.reduce(
        access = access,
        profileAddresses = profile,
        aclAddresses = acl,
        controlledAddress = controlled,
    )
}
