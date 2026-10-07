package dev.wochap.wobook.service

import android.content.Context
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.ProcessLifecycleOwner
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import dev.wochap.wobook.ffi.SyncState
import dev.wochap.wobook.wobook
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import java.util.concurrent.TimeUnit

/**
 * Periodic background sync (design D8): every 15 min on unmetered networks,
 * networking on, `sync_now`, wait for UpToDate or 60 s, networking off.
 */
class BackgroundSyncWorker(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result {
        val repo = applicationContext.wobook.repository
        val foreground = withContext(Dispatchers.Main) {
            ProcessLifecycleOwner.get().lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED)
        }
        return try {
            val app = repo.app()
            app.setForeground(true)
            app.syncNow()
            withTimeoutOrNull(60_000) {
                // Give peers a moment to connect before trusting "up to date".
                delay(3_000)
                while (true) {
                    val status = app.syncStatus()
                    if (status.state is SyncState.UpToDate) break
                    delay(1_000)
                }
            }
            Result.success()
        } catch (e: Exception) {
            Result.retry()
        } finally {
            if (!foreground) runCatching { repo.current()?.setForeground(false) }
        }
    }

    companion object {
        private const val NAME = "wobook-background-sync"

        fun schedule(context: Context, enabled: Boolean) {
            val wm = WorkManager.getInstance(context)
            if (!enabled) {
                wm.cancelUniqueWork(NAME)
                return
            }
            val request = PeriodicWorkRequestBuilder<BackgroundSyncWorker>(15, TimeUnit.MINUTES)
                .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.UNMETERED).build())
                .build()
            wm.enqueueUniquePeriodicWork(NAME, ExistingPeriodicWorkPolicy.UPDATE, request)
        }
    }
}
