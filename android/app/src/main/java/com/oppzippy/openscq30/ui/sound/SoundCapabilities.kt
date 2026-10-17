package com.oppzippy.openscq30.ui.sound

/** UI grouping only: protocol identifiers and settings are left intact. */
internal object SoundCapabilities {
    val comfort = setOf(
        "wearingDetection", "autoPlayPause", "wearingTone", "touchLock", "touchTone", "autoPowerOff",
        "autoPowerOffPrompt",
        "lowBatteryPrompt", "voicePrompt", "easyChat", "easyChatWaitTime", "soundLeakCompensation", "volumeBalance",
        "limitHighVolume", "limitHighVolumeDbLimit", "limitHighVolumeRefreshRate", "sideTone",
    )
    val spatial =
        setOf(
            "spatialAudio",
            "spatialAudioMode",
            "spatialAudioMusicMode",
            "immersiveExperience",
            "headTracking",
            "dolbyAudio",
        )
    val connections = setOf("dualConnections", "dualConnectionsDevices", "ldac")
    val case =
        setOf(
            "caseBatteryLevel",
            "caseLanguage",
            "caseSerialNumber",
            "caseFirmwareVersion",
            "atmospheric",
            "remoteCamera",
            "findDevice",
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
 * Whether a page shows every setting of a category, so that the category can open the page instead of the plain list
 * of its settings. An empty category has nothing to show on the page.
 */
internal fun coveredByPage(categorySettingIds: Collection<String>, pageSettingIds: Set<String>): Boolean =
    categorySettingIds.isNotEmpty() && pageSettingIds.containsAll(categorySettingIds)
