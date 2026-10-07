package dev.wochap.wobook

import android.app.Application
import android.util.Log
import androidx.lifecycle.ProcessLifecycleOwner
import dev.wochap.wobook.crypto.KeystoreSecureStore
import dev.wochap.wobook.data.AppRepository
import dev.wochap.wobook.data.Settings
import dev.wochap.wobook.ffi.AppConfig
import dev.wochap.wobook.ffi.WobookApp
import dev.wochap.wobook.ffi.WobookException
import dev.wochap.wobook.service.BackgroundSyncWorker
import dev.wochap.wobook.service.ForegroundSyncLifecycle
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import java.io.File

/** Owns the single Rust `WobookApp` for the process, opened lazily. */
class WobookApplication : Application() {
    lateinit var repository: AppRepository
        private set
    lateinit var settings: Settings
        private set
    val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    override fun onCreate() {
        super.onCreate()
        settings = Settings(this)
        val keyStore = KeystoreSecureStore(filesDir)
        val dataDir = File(filesDir, "wobook")
        repository = AppRepository { listener ->
            val config = AppConfig(
                dataDir = dataDir.absolutePath,
                deviceName = "",
                enableNetwork = false,
                allowTailnet = true,
            )
            try {
                WobookApp.open(config, keyStore, listener)
            } catch (e: WobookException.IdentityLost) {
                // Keystore key lost (reinstall/restore): this phone becomes a
                // new device. Keep the old data aside, onboard again.
                Log.w(TAG, "identity lost; starting as a new device", e)
                dataDir.renameTo(File(filesDir, "wobook-lost-${System.currentTimeMillis()}"))
                keyStore.clear()
                runBlocking { settings.setOnboardingDone(false) }
                WobookApp.open(config, keyStore, listener)
            }
        }
        ProcessLifecycleOwner.get().lifecycle.addObserver(ForegroundSyncLifecycle(this, repository))
        scope.launch {
            settings.state.map { it.backgroundSync }.distinctUntilChanged().collect { on ->
                BackgroundSyncWorker.schedule(this@WobookApplication, on)
            }
        }
    }

    companion object {
        const val TAG = "wobook"
    }
}

val android.content.Context.wobook: WobookApplication
    get() = applicationContext as WobookApplication
