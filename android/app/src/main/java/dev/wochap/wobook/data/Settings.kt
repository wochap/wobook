package dev.wochap.wobook.data

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.longPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map

private val Context.store: DataStore<Preferences> by preferencesDataStore(name = "settings")

enum class TapBehaviour { Detail, Open }

data class SettingsState(
    val onboardingDone: Boolean = false,
    val backgroundSync: Boolean = true,
    val tap: TapBehaviour = TapBehaviour.Detail,
    val autoFetch: Boolean = true,
    val loadIcons: Boolean = true,
    val lastIconRefreshMs: Long? = null,
)

/** DataStore preferences: onboarding, background sync, tap behaviour, auto-fetch, site icons. */
class Settings(private val context: Context) {
    private object Keys {
        val onboarding = booleanPreferencesKey("onboarding_done")
        val background = booleanPreferencesKey("background_sync")
        val tapOpens = booleanPreferencesKey("tap_opens_browser")
        val autoFetch = booleanPreferencesKey("auto_fetch")
        val loadIcons = booleanPreferencesKey("load_site_icons")
        val lastIconRefresh = longPreferencesKey("last_icon_refresh_ms")
    }

    val state: Flow<SettingsState> = context.store.data.map { p ->
        SettingsState(
            onboardingDone = p[Keys.onboarding] ?: false,
            backgroundSync = p[Keys.background] ?: true,
            tap = if (p[Keys.tapOpens] == true) TapBehaviour.Open else TapBehaviour.Detail,
            autoFetch = p[Keys.autoFetch] ?: true,
            loadIcons = p[Keys.loadIcons] ?: true,
            lastIconRefreshMs = p[Keys.lastIconRefresh],
        )
    }

    suspend fun current(): SettingsState = state.first()

    suspend fun setOnboardingDone(done: Boolean) = context.store.edit { it[Keys.onboarding] = done }
    suspend fun setBackgroundSync(on: Boolean) = context.store.edit { it[Keys.background] = on }
    suspend fun setTap(tap: TapBehaviour) = context.store.edit { it[Keys.tapOpens] = tap == TapBehaviour.Open }
    suspend fun setAutoFetch(on: Boolean) = context.store.edit { it[Keys.autoFetch] = on }
    suspend fun setLoadIcons(on: Boolean) = context.store.edit { it[Keys.loadIcons] = on }
    suspend fun setLastIconRefreshMs(ms: Long) = context.store.edit { it[Keys.lastIconRefresh] = ms }
}
