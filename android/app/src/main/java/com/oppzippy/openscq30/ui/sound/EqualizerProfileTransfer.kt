package com.oppzippy.openscq30.ui.sound

import com.oppzippy.openscq30.features.equalizer.storage.LegacyEqualizerProfile
import com.oppzippy.openscq30.lib.wrapper.Equalizer
import kotlin.math.floor
import kotlin.math.pow
import kotlin.math.roundToInt
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json

/** A custom equalizer profile in the library's import and export format, with values in decibels. */
@Serializable
internal data class TransferProfile(val name: String, val volumeAdjustments: List<Double>)

private val transferJson = Json { ignoreUnknownKeys = true }

/** The profiles in an exported file, or null when the text isn't one. */
internal fun parseProfiles(text: String): List<TransferProfile>? = try {
    transferJson.decodeFromString<List<TransferProfile>>(text.trim())
} catch (_: SerializationException) {
    null
} catch (_: IllegalArgumentException) {
    null
}

internal fun profilesJson(profiles: List<TransferProfile>): String = transferJson.encodeToString(profiles)

internal enum class ImportStatus { New, Replaces, WrongBandCount, OutOfRange }

internal data class ImportPreviewItem(val profile: TransferProfile, val status: ImportStatus) {
    val isValid get() = status == ImportStatus.New || status == ImportStatus.Replaces
}

/**
 * What importing would do with each profile, checked the way the library checks it: the device's number of bands, and
 * each value within its range once scaled to the device's precision. Without the device's equalizer (while only one
 * earbud is connected) only the names are compared, and the library checks the rest.
 */
internal fun previewImport(
    profiles: List<TransferProfile>,
    existingNames: Collection<String>,
    equalizer: Equalizer?,
): List<ImportPreviewItem> = profiles.map { profile ->
    val status = when {
        equalizer != null && profile.volumeAdjustments.size != equalizer.bandHz.size -> ImportStatus.WrongBandCount
        equalizer != null && profile.volumeAdjustments.any { !it.fitsIn(equalizer) } -> ImportStatus.OutOfRange
        profile.name in existingNames -> ImportStatus.Replaces
        else -> ImportStatus.New
    }
    ImportPreviewItem(profile, status)
}

private fun Double.fitsIn(equalizer: Equalizer): Boolean {
    val scaled = scaledToDevice(equalizer.fractionDigits)
    // a negated contains, so that NaN is rejected as well, as the library does
    return scaled in equalizer.min.toDouble()..equalizer.max.toDouble()
}

// Rounds halves away from zero, as the library does.
private fun Double.scaledToDevice(fractionDigits: Short): Double {
    val value = this * 10.0.pow(fractionDigits.toInt())
    return if (value < 0) -floor(-value + 0.5) else floor(value + 0.5)
}

/** Decibels to the device's units, for drawing a profile with the device's equalizer range. */
internal fun TransferProfile.deviceValues(fractionDigits: Short): List<Short> =
    volumeAdjustments.map { it.scaledToDevice(fractionDigits).roundToInt().toShort() }

internal fun LegacyEqualizerProfile.toTransferProfile() = TransferProfile(name, getVolumeAdjustments())

/** Old profiles still to move over: one that has a custom profile of the same name was moved already. */
internal fun unmigratedLegacyProfiles(
    legacy: List<LegacyEqualizerProfile>,
    customNames: Collection<String>,
): List<LegacyEqualizerProfile> = legacy.filter { it.name !in customNames }
