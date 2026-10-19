package com.oppzippy.openscq30.ui.sound

import com.oppzippy.openscq30.features.equalizer.storage.LegacyEqualizerProfile
import com.oppzippy.openscq30.lib.bindings.ClientTransferProfile
import com.oppzippy.openscq30.lib.bindings.clientEqualizerDeviceValues
import com.oppzippy.openscq30.lib.bindings.clientParseProfiles
import com.oppzippy.openscq30.lib.bindings.clientPreviewImport
import com.oppzippy.openscq30.lib.bindings.clientProfilesJson
import com.oppzippy.openscq30.lib.wrapper.Equalizer

internal typealias TransferProfile = ClientTransferProfile

internal fun parseProfiles(text: String): List<TransferProfile>? = clientParseProfiles(text)

internal fun profilesJson(profiles: List<TransferProfile>): String = clientProfilesJson(profiles)

internal enum class ImportStatus { New, Replaces, WrongBandCount, OutOfRange }

internal data class ImportPreviewItem(val profile: TransferProfile, val status: ImportStatus) {
    val isValid get() = status == ImportStatus.New || status == ImportStatus.Replaces
}

internal fun previewImport(
    profiles: List<TransferProfile>,
    existingNames: Collection<String>,
    equalizer: Equalizer?,
): List<ImportPreviewItem> = clientPreviewImport(profiles, existingNames.toList(), equalizer)
    .mapIndexed { index, status -> ImportPreviewItem(profiles[index], ImportStatus.valueOf(status)) }

internal fun TransferProfile.deviceValues(fractionDigits: Short): List<Short> =
    clientEqualizerDeviceValues(this, fractionDigits)

internal fun LegacyEqualizerProfile.toTransferProfile() = TransferProfile(name, getVolumeAdjustments())

internal fun unmigratedLegacyProfiles(
    legacy: List<LegacyEqualizerProfile>,
    customNames: Collection<String>,
): List<LegacyEqualizerProfile> = legacy.filter { it.name !in customNames }
