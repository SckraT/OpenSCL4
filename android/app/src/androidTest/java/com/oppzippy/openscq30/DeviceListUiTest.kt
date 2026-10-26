package com.oppzippy.openscq30

import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import com.oppzippy.openscq30.features.phonelink.BluetoothAccess
import com.oppzippy.openscq30.features.phonelink.PhoneLinkSignals
import com.oppzippy.openscq30.lib.wrapper.PairedDevice
import com.oppzippy.openscq30.ui.deviceselection.AppControl
import com.oppzippy.openscq30.ui.deviceselection.AppControlSnapshot
import com.oppzippy.openscq30.ui.deviceselection.DeviceListPresenter
import com.oppzippy.openscq30.ui.deviceselection.screens.DeviceListingScreen
import com.oppzippy.openscq30.ui.theme.OpenSCQ30Theme
import dagger.hilt.android.testing.HiltAndroidTest
import org.junit.Assert.assertTrue
import org.junit.Test

@HiltAndroidTest
class DeviceListUiTest : EmptyActivityTestBase() {
    private val controlled = PairedDevice("AA:BB:CC:DD:EE:01", "SoundcoreA3954", false)
    private val linked = PairedDevice("AA:BB:CC:DD:EE:02", "SoundcoreA3028", false)

    @Test
    fun featuredShowsBothAxesAndTheLinkedRowShowsTheDivergence() {
        show(
            DeviceListPresenter.present(
                devices = listOf(controlled, linked),
                // an app session alone leaves the link unconfirmed, so the controlled device is seen by a scan too
                phoneLink = PhoneLinkSignals(
                    access = BluetoothAccess.Ready,
                    profileAddresses = setOf(controlled.macAddress, linked.macAddress),
                ),
                appControl = AppControlSnapshot(controlled.macAddress, AppControl.Controlling),
            ),
        )

        composeRule.onNodeWithText(getString(R.string.sound_device_phone_link_connected)).assertExists()
        composeRule.onNodeWithText(getString(R.string.sound_device_control_active)).assertExists()
        composeRule.onNodeWithText(getString(R.string.sound_device_row_linked_without_control)).assertExists()
        composeRule.onNodeWithText(getString(R.string.sound_device_not_connected)).assertDoesNotExist()
    }

    @Test
    fun theBluetoothBannerKeepsTheSavedListAndNeverClaimsNotConnected() {
        show(
            DeviceListPresenter.present(
                devices = listOf(controlled, linked),
                phoneLink = PhoneLinkSignals(access = BluetoothAccess.Off),
                appControl = AppControlSnapshot(),
            ),
        )

        composeRule.onNodeWithText(getString(R.string.sound_permission_bluetooth_disabled)).assertExists()
        composeRule.onNodeWithText(getString(R.string.sound_device_bluetooth_unknown)).assertDoesNotExist()
        assertTrue(
            composeRule.onAllNodesWithText(getString(R.string.sound_device_phone_link_unknown))
                .fetchSemanticsNodes().isNotEmpty(),
        )
        assertTrue(
            composeRule.onAllNodesWithText(getString(R.string.sound_device_row_bluetooth_unknown))
                .fetchSemanticsNodes().isNotEmpty(),
        )
        composeRule.onNodeWithText(getString(R.string.sound_device_not_connected)).assertDoesNotExist()
    }

    private fun show(state: com.oppzippy.openscq30.ui.deviceselection.DeviceListState) {
        // the rule sets the content on the main thread and waits for it to settle
        composeRule.setContent {
            OpenSCQ30Theme(dynamicColor = false) {
                DeviceListingScreen(state = state)
            }
        }
    }
}
