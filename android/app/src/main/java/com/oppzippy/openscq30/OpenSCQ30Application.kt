package com.oppzippy.openscq30

import android.app.Application
import com.oppzippy.openscq30.features.autoconnect.startAutoConnectService
import com.oppzippy.openscq30.features.preferences.Preferences
import com.oppzippy.openscq30.lib.bindings.LanguageIdentifier
import dagger.hilt.android.HiltAndroidApp
import javax.inject.Inject

@HiltAndroidApp
class OpenSCQ30Application : Application() {

    @Inject
    lateinit var preferences: Preferences

    init {
        Native.initialize()
    }

    override fun onCreate() {
        super.onCreate()
        initializeI18n()
        if (preferences.autoConnect) {
            // Refused when the process starts in the background; MainActivity retries in the foreground.
            startAutoConnectService(this)
        }
    }

    private fun initializeI18n() {
        val locales = resources.configuration.locales
        val languageIdentifiers = (0..<locales.size()).map { i ->
            val locale = locales[i]
            LanguageIdentifier(
                language = locale.language,
                script = locale.script.ifEmpty { null },
                region = locale.country.ifEmpty { null },
                variants = if (locale.variant.isEmpty()) emptyList() else listOf(locale.variant),
            )
        }
        Native.initializeI18n(languageIdentifiers)
    }
}
