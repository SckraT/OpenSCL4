package com.oppzippy.openscq30.ui.sound

/** UI grouping only: protocol identifiers and settings are left intact. The groups are in SettingGroups.kt. */
internal object SoundCapabilities {
    val case =
        setOf(
            "caseBatteryLevel",
            "caseLanguage",
            "caseSerialNumber",
            "caseFirmwareVersion",
            "atmospheric",
            "remoteCamera",
            "findDevice",
            "caseSpatialAudio",
            "airPressure",
        )
    val modes = setOf(
        "ambientSoundMode", "transparencyMode", "noiseCancelingMode", "customNoiseCanceling", "adaptiveNoiseCanceling",
        "manualNoiseCanceling", "manualTransparency", "airplaneMode", "windNoiseSuppression", "windNoiseDetected",
        "adaptiveNoiseCancelingSensitivityLevel", "multiSceneNoiseCanceling", "transportationMode",
        "environmentDetection",
        "realTimeAdaptiveNoiseCanceling", "ancPersonalizedToEarCanal",
    )
    fun isControl(id: String): Boolean = id.contains("Press") || id.contains("Slide") || id in setOf(
        "singlePress",
        "doublePress",
        "buttonsEnabled",
        "resetButtonsToDefault",
        "normalModeInCycle",
        "transparencyModeInCycle",
        "noiseCancelingModeInCycle",
    )
}

/**
 * Whether the equalizer opens the equalizer page, which shows only some kinds of equalizer settings; a device with
 * others gets the plain list of them. While only one earbud is connected the device offers no equalizer settings, but
 * the page still says why, and saved profiles can still be imported and exported there.
 */
internal fun equalizerOpensItsPage(categorySettingIds: Collection<String>, oneEarbudOnly: Boolean): Boolean =
    if (categorySettingIds.isEmpty()) oneEarbudOnly else BOTH_EARBUDS_SETTING_IDS.containsAll(categorySettingIds)
