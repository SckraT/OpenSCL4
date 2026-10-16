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

    private val preferenceKeysToPreferences = mapOf(
        PREFERENCE_AUTO_CONNECT to autoConnectPreference,
        PREFERENCE_THEME to themePreference,
        PREFERENCE_DYNAMIC_COLOR to dynamicColorPreference,
        PREFERENCE_EARBUDS_COLOR to earbudsColorPreference,
        PREFERENCE_LAST_DEVICE to lastDevicePreference,
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
        SharedPreferences.OnSharedPreferenceChangeListener { _, key -> preferenceKeysToPreferences[key]?.refresh() }

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
