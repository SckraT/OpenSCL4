package com.oppzippy.openscq30

import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import com.oppzippy.openscq30.actions.addAndConnectToDemoDevice
import com.oppzippy.openscq30.features.equalizer.storage.LegacyEqualizerProfile
import com.oppzippy.openscq30.features.equalizer.storage.LegacyEqualizerProfileDao
import com.oppzippy.openscq30.lib.bindings.translateDeviceModel
import dagger.hilt.android.testing.HiltAndroidTest
import javax.inject.Inject
import kotlinx.coroutines.runBlocking
import org.junit.Test

@HiltAndroidTest
class MigrateLegacyEqualizerProfilesTest : OpenSCQ30RootTestBase() {
    @Inject
    lateinit var legacyEqualizerProfileDao: LegacyEqualizerProfileDao

    @Test
    fun testMigrateLegacyProfile(): Unit = runBlocking {
        legacyEqualizerProfileDao.insert(
            LegacyEqualizerProfile(
                "Test Legacy Profile",
                10,
                -20,
                30,
                40,
                50,
                60,
                70,
                80,
            ),
        )
        addAndConnectToDemoDevice(composeRule, translateDeviceModel("SoundcoreA3028"))

        composeRule.waitUntil(uiTimeoutMillis(10_000)) {
            composeRule.onAllNodesWithContentDescription(getString(R.string.sound_overview_device_actions))
                .fetchSemanticsNodes().isNotEmpty()
        }
        // the equalizer page offers to move the old profile over
        composeRule.waitUntil(uiTimeoutMillis()) {
            composeRule.onAllNodes(hasText(getString(R.string.equalizer))).fetchSemanticsNodes().isNotEmpty()
        }
        composeRule.onNodeWithText(getString(R.string.equalizer)).performScrollTo().performClick()
        val found = composeRule.activity.resources.getQuantityString(R.plurals.sound_profile_legacy_found, 1, 1)
        composeRule.waitUntil(uiTimeoutMillis()) {
            composeRule.onAllNodes(hasText(found)).fetchSemanticsNodes().isNotEmpty()
        }
        composeRule.onNodeWithText(getString(R.string.sound_profile_migrate_all)).performClick()
        // once moved, the profile is a saved one and the offer is gone
        composeRule.waitUntil(uiTimeoutMillis()) {
            composeRule.onAllNodes(
                hasContentDescription(getString(R.string.sound_profile_more_named, "Test Legacy Profile")),
            ).fetchSemanticsNodes().isNotEmpty()
        }
        composeRule.onNodeWithText(found).assertDoesNotExist()
        composeRule.onNodeWithText("Test Legacy Profile").performScrollTo().performClick()
        composeRule.waitUntil(uiTimeoutMillis()) {
            composeRule.onAllNodes(
                hasContentDescription(getString(R.string.in_effect)),
            ).fetchSemanticsNodes().isNotEmpty()
        }
        composeRule.onNodeWithText(getString(R.string.sound_profile_enter_values)).performScrollTo().performClick()
        composeRule.onNode(hasText(getString(R.string.hz, 100)) and hasSetTextAction()).assertTextContains("1")
        composeRule.onNode(hasText(getString(R.string.hz, 200)) and hasSetTextAction()).assertTextContains("-2")
    }
}
