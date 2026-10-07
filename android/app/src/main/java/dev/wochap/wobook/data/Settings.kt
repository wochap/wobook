package dev.wochap.wobook.data

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
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
)

/** DataStore preferences: onboarding, background sync, tap behaviour, auto-fetch. */
class Settings(private val context: Context) {
    private object Keys {
        val onboarding = booleanPreferencesKey("onboarding_done")
        val background = booleanPreferencesKey("background_sync")
        val tapOpens = booleanPreferencesKey("tap_opens_browser")
        val autoFetch = booleanPreferencesKey("auto_fetch")
    }

    val state: Flow<SettingsState> = context.store.data.map { p ->
        SettingsState(
            onboardingDone = p[Keys.onboarding] ?: false,
            backgroundSync = p[Keys.background] ?: true,
            tap = if (p[Keys.tapOpens] == true) TapBehaviour.Open else TapBehaviour.Detail,
            autoFetch = p[Keys.autoFetch] ?: true,
        )
    }

    suspend fun current(): SettingsState = state.first()

    suspend fun setOnboardingDone(done: Boolean) = context.store.edit { it[Keys.onboarding] = done }
    suspend fun setBackgroundSync(on: Boolean) = context.store.edit { it[Keys.background] = on }
    suspend fun setTap(tap: TapBehaviour) = context.store.edit { it[Keys.tapOpens] = tap == TapBehaviour.Open }
    suspend fun setAutoFetch(on: Boolean) = context.store.edit { it[Keys.autoFetch] = on }
}
