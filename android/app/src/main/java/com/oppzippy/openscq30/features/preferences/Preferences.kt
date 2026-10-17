package com.oppzippy.openscq30.features.preferences

import android.content.Context
import android.content.SharedPreferences
import android.util.Log
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.core.content.edit
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import com.oppzippy.openscq30.ui.theme.ThemeType
import com.oppzippy.openscq30.ui.theme.prefersDarkTheme
import dagger.hilt.android.lifecycle.HiltViewModel
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

@Singleton
class Preferences @Inject constructor(@ApplicationContext context: Context) {
    companion object {
        const val TAG = "Preferences"

        private const val PREFERENCE_AUTO_CONNECT = "autoConnect"
        private const val PREFERENCE_THEME = "theme"
        private const val PREFERENCE_DYNAMIC_COLOR = "dynamicColor"
        private const val PREFERENCE_EARBUDS_COLOR = "earbudsColor"
        private const val PREFERENCE_LAST_DEVICE = "lastDeviceMacAddress"
        private const val PREFERENCE_SOUND_MODES_VIEW = "soundModesView"
        private const val PREFERENCE_ONE_EARBUD_BADGE = "showOneEarbudBadge"
        private const val PREFIX_OVERVIEW_LAYOUT = "overviewLayout."
        private const val PREFIX_QUICK_TOGGLES = "quickToggles."
    }

    private val preferences = context.getSharedPreferences("preferences", Context.MODE_PRIVATE)

    private val autoConnectPreference = Preference(
        get = { preferences.getBoolean(PREFERENCE_AUTO_CONNECT, false) },
        set = { preferences.edit { putBoolean(PREFERENCE_AUTO_CONNECT, it) } },
    )
    private val themePreference = Preference(
        get = {
            preferences.getString(PREFERENCE_THEME, null)?.let { themeName ->
                try {
                    ThemeType.valueOf(themeName)
                } catch (ex: IllegalArgumentException) {
                    Log.e(TAG, "error parsing theme: $themeName", ex)
                    null
                }
            }
        },
        set = {
            preferences.edit {
                if (it != null) {
                    putString(PREFERENCE_THEME, it.name)
                } else {
                    remove(PREFERENCE_THEME)
                }
            }
        },
    )
    private val dynamicColorPreference = Preference(
        get = { preferences.getBoolean(PREFERENCE_DYNAMIC_COLOR, false) },
        set = { preferences.edit { putBoolean(PREFERENCE_DYNAMIC_COLOR, it) } },
    )

    // null follows the theme: silver earbuds in light themes, black ones in dark themes
    private val earbudsColorPreference = Preference(
        get = {
            preferences.getString(PREFERENCE_EARBUDS_COLOR, null)?.let { name ->
                try {
                    EarbudsColor.valueOf(name)
                } catch (ex: IllegalArgumentException) {
                    Log.e(TAG, "error parsing earbuds color: $name", ex)
                    null
                }
            }
        },
        set = {
            preferences.edit {
                if (it != null) {
                    putString(PREFERENCE_EARBUDS_COLOR, it.name)
                } else {
                    remove(PREFERENCE_EARBUDS_COLOR)
                }
            }
        },
    )

    // the device the user opened most recently, shown at the top of the device list while nothing is connected
    private val lastDevicePreference = Preference(
        get = { preferences.getString(PREFERENCE_LAST_DEVICE, null) },
        set = {
            preferences.edit {
                if (it != null) putString(PREFERENCE_LAST_DEVICE, it) else remove(PREFERENCE_LAST_DEVICE)
            }
        },
    )

    private val soundModesViewPreference = Preference(
        get = {
            preferences.getString(PREFERENCE_SOUND_MODES_VIEW, null)?.let { name ->
                SoundModesView.entries.firstOrNull { it.name == name }
            } ?: SoundModesView.Tiles
        },
        set = { preferences.edit { putString(PREFERENCE_SOUND_MODES_VIEW, it.name) } },
    )
    private val oneEarbudBadgePreference = Preference(
        get = { preferences.getBoolean(PREFERENCE_ONE_EARBUD_BADGE, true) },
        set = { preferences.edit { putBoolean(PREFERENCE_ONE_EARBUD_BADGE, it) } },
    )

    // values stored for each device model, created when first asked for
    private val perModelPreferences = mutableMapOf<String, Preference<String?>>()

    private fun perModel(prefix: String, model: String): Preference<String?> {
        val key = prefix + model
        return synchronized(perModelPreferences) {
            perModelPreferences.getOrPut(key) {
                Preference(
                    get = { preferences.getString(key, null) },
                    set = { value -> preferences.edit { if (value != null) putString(key, value) else remove(key) } },
                )
            }
        }
    }

    private val preferenceKeysToPreferences = mapOf(
        PREFERENCE_AUTO_CONNECT to autoConnectPreference,
        PREFERENCE_THEME to themePreference,
        PREFERENCE_DYNAMIC_COLOR to dynamicColorPreference,
        PREFERENCE_EARBUDS_COLOR to earbudsColorPreference,
        PREFERENCE_LAST_DEVICE to lastDevicePreference,
        PREFERENCE_SOUND_MODES_VIEW to soundModesViewPreference,
        PREFERENCE_ONE_EARBUD_BADGE to oneEarbudBadgePreference,
    )

    // Marked as -keepclassmembers in proguard-rules so that it doesn't get optimized out. This would lead to it getting
    // garbage collected, since registerOnSharedPreferenceChangeListener only stores a weak reference.
    //
    // From decompiled release apk with onChangeListener being private (see the new object being passed directly to
    // registerOnSharedPreferenceChangeListener):
    //
    // Without proguard rule:
    //    sharedPreferences.registerOnSharedPreferenceChangeListener(new SharedPreferences.OnSharedPreferenceChangeListener() {
    //        @Override // android.content.SharedPreferences.OnSharedPreferenceChangeListener
    //        public final void onSharedPreferenceChanged(SharedPreferences sharedPreferences2, String str) {
    //            g12 g12Var4 = (g12) this.a.e.get(str);
    //            if (g12Var4 != null) {
    //                g12Var4.c.j(g12Var4.a.a());
    //            }
    //        }
    //    });
    // With proguard rule:
    //        SharedPreferences.OnSharedPreferenceChangeListener onSharedPreferenceChangeListener = new SharedPreferences.OnSharedPreferenceChangeListener() { // from class: m12
    //            @Override // android.content.SharedPreferences.OnSharedPreferenceChangeListener
    //            public final void onSharedPreferenceChanged(SharedPreferences sharedPreferences2, String str) {
    //                g12 g12Var4 = (g12) this.a.e.get(str);
    //                if (g12Var4 != null) {
    //                    g12Var4.c.j(g12Var4.a.a());
    //                }
    //            }
    //        };
    //        this.onChangeListener = onSharedPreferenceChangeListener;
    //        sharedPreferences.registerOnSharedPreferenceChangeListener(onSharedPreferenceChangeListener);
    private val onChangeListener =
        SharedPreferences.OnSharedPreferenceChangeListener { _, key ->
            preferenceKeysToPreferences[key]?.refresh()
            synchronized(perModelPreferences) { perModelPreferences[key] }?.refresh()
        }

    init {
        preferences.registerOnSharedPreferenceChangeListener(onChangeListener)
    }

    val autoConnectFlow = autoConnectPreference.flow
    var autoConnect: Boolean
        get() = autoConnectPreference.flow.value
        set(value) = autoConnectPreference.set(value)

    val themeFlow = themePreference.flow
    var theme: ThemeType?
        get() = themePreference.flow.value
        set(theme) = themePreference.set(theme)

    val dynamicColorFlow = dynamicColorPreference.flow
    var dynamicColor: Boolean
        get() = dynamicColorPreference.flow.value
        set(theme) = dynamicColorPreference.set(theme)

    val earbudsColorFlow = earbudsColorPreference.flow
    var earbudsColor: EarbudsColor?
        get() = earbudsColorPreference.flow.value
        set(color) = earbudsColorPreference.set(color)

    val lastDeviceMacAddressFlow = lastDevicePreference.flow
    var lastDeviceMacAddress: String?
        get() = lastDevicePreference.flow.value
        set(macAddress) = lastDevicePreference.set(macAddress)

    val soundModesViewFlow = soundModesViewPreference.flow
    var soundModesView: SoundModesView
        get() = soundModesViewPreference.flow.value
        set(view) = soundModesViewPreference.set(view)

    val oneEarbudBadgeFlow = oneEarbudBadgePreference.flow
    var showOneEarbudBadge: Boolean
        get() = oneEarbudBadgePreference.flow.value
        set(show) = oneEarbudBadgePreference.set(show)

    /** Blocks of the device overview and their order, as stored by [com.oppzippy.openscq30.ui.sound.OverviewLayout]. */
    fun overviewLayoutFlow(model: String): StateFlow<String?> = perModel(PREFIX_OVERVIEW_LAYOUT, model).flow
    fun setOverviewLayout(model: String, layout: String?) = perModel(PREFIX_OVERVIEW_LAYOUT, model).set(layout)

    /** Setting ids of the quick toggles on the device card, comma separated; null until the user changes them. */
    fun quickTogglesFlow(model: String): StateFlow<String?> = perModel(PREFIX_QUICK_TOGGLES, model).flow
    fun setQuickToggles(model: String, settingIds: String?) = perModel(PREFIX_QUICK_TOGGLES, model).set(settingIds)
}

/** How the sound modes are shown: a tile for each mode, or one scale through transparency, normal and noise canceling. */
enum class SoundModesView {
    Tiles,
    Scale,
}

/** Which earbuds the device overview shows, independently of the theme. */
enum class EarbudsColor {
    Silver,
    Black,
}

private data class Preference<T>(val get: () -> T, val set: (T) -> Unit) {
    private val _flow = MutableStateFlow(get())
    val flow = _flow.asStateFlow()

    fun refresh() {
        _flow.value = get()
    }
}

@HiltViewModel
class ThemeViewModel @Inject constructor(preferences: Preferences) : ViewModel() {
    val themeFlow = preferences.themeFlow
    val dynamicColorFlow = preferences.dynamicColorFlow
}

@Composable
fun prefersDarkTheme(themeViewModel: ThemeViewModel = hiltViewModel()): Boolean =
    themeViewModel.themeFlow.collectAsState().value.prefersDarkTheme()

@Composable
fun prefersDynamicColor(themeViewModel: ThemeViewModel = hiltViewModel()): Boolean =
    themeViewModel.dynamicColorFlow.collectAsState().value

@HiltViewModel
class EarbudsColorViewModel @Inject constructor(preferences: Preferences) : ViewModel() {
    val earbudsColorFlow = preferences.earbudsColorFlow
}

/** The earbuds color chosen in the settings, or null to follow the theme. */
@Composable
fun preferredEarbudsColor(viewModel: EarbudsColorViewModel = hiltViewModel()): EarbudsColor? =
    viewModel.earbudsColorFlow.collectAsState().value

@HiltViewModel
class DeviceUiPreferencesViewModel @Inject constructor(private val preferences: Preferences) : ViewModel() {
    val soundModesViewFlow = preferences.soundModesViewFlow
    val oneEarbudBadgeFlow = preferences.oneEarbudBadgeFlow
    fun overviewLayoutFlow(model: String) = preferences.overviewLayoutFlow(model)
    fun setOverviewLayout(model: String, layout: String?) = preferences.setOverviewLayout(model, layout)
    fun quickTogglesFlow(model: String) = preferences.quickTogglesFlow(model)
    fun setQuickToggles(model: String, settingIds: String?) = preferences.setQuickToggles(model, settingIds)
}
