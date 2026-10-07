package dev.wochap.wobook.service

import android.content.Context
import android.net.wifi.WifiManager
import android.util.Log
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import dev.wochap.wobook.data.AppRepository
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch

/**
 * Process lifecycle (design D8): started → multicast lock + networking on;
 * stopped → networking off + lock released. No foreground service.
 */
class ForegroundSyncLifecycle(
    private val context: Context,
    private val repository: AppRepository,
) : DefaultLifecycleObserver {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private var lock: WifiManager.MulticastLock? = null
    private var stopping: Job? = null

    override fun onStart(owner: LifecycleOwner) {
        stopping?.cancel()
        acquire()
        scope.launch {
            runCatching {
                repository.setForeground(true)
                repository.syncNow()
            }.onFailure { Log.w("wobook", "foreground sync start failed", it) }
        }
    }

    override fun onStop(owner: LifecycleOwner) {
        // Grace period: push changes made just before leaving (share sheet
        // saves, then finishes) before networking goes off.
        stopping = scope.launch {
            runCatching { repository.current()?.syncNow() }
            delay(GRACE_MS)
            runCatching { repository.current()?.setForeground(false) }
            release()
        }
    }

    private companion object {
        const val GRACE_MS = 10_000L
    }

    private fun acquire() {
        if (lock?.isHeld == true) return
        val wifi = context.applicationContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager ?: return
        lock = wifi.createMulticastLock("wobook-mdns").apply {
            setReferenceCounted(false)
            runCatching { acquire() }
        }
    }

    private fun release() {
        lock?.let { if (it.isHeld) it.release() }
        lock = null
    }
}
