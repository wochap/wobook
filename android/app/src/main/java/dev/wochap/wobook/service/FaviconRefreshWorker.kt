package dev.wochap.wobook.service

import android.content.Context
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import androidx.work.workDataOf
import dev.wochap.wobook.domain.Favicons
import dev.wochap.wobook.wobook
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

/**
 * "Refresh site icons" (android-favicons design D5): re-fetches the icon of
 * every distinct origin among live bookmarks, ignoring cache ages. Fetches go
 * through the shared cache, so its 4-way cap and per-host dedupe apply.
 */
class FaviconRefreshWorker(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result {
        val app = applicationContext.wobook
        if (!app.settings.current().loadIcons) return Result.success(workDataOf(KEY_FOUND to 0, KEY_NONE to 0))
        val repo = app.repository
        val origins = runCatching { repo.list() }.getOrElse { return Result.failure() }
            .mapNotNull { Favicons.origin(it.url) }
            .distinct()
        val total = origins.size
        var done = 0
        var found = 0
        val counter = Mutex()
        setProgress(workDataOf(KEY_DONE to 0, KEY_TOTAL to total))
        coroutineScope {
            origins.map { origin ->
                async {
                    if (isStopped) return@async
                    val ok = repo.favicons.refresh(origin)
                    val progress = counter.withLock {
                        done++
                        if (ok) found++
                        done
                    }
                    if (!isStopped) setProgress(workDataOf(KEY_DONE to progress, KEY_TOTAL to total))
                }
            }.awaitAll()
        }
        app.settings.setLastIconRefreshMs(System.currentTimeMillis())
        return Result.success(workDataOf(KEY_FOUND to found, KEY_NONE to done - found))
    }

    companion object {
        const val NAME = "favicon-refresh"
        const val KEY_DONE = "done"
        const val KEY_TOTAL = "total"
        const val KEY_FOUND = "found"
        const val KEY_NONE = "none"

        fun start(context: Context) {
            val request = OneTimeWorkRequestBuilder<FaviconRefreshWorker>()
                .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
                .build()
            WorkManager.getInstance(context).enqueueUniqueWork(NAME, ExistingWorkPolicy.KEEP, request)
        }

        fun cancel(context: Context) {
            WorkManager.getInstance(context).cancelUniqueWork(NAME)
        }
    }
}
