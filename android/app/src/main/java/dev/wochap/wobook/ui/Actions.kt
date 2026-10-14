package dev.wochap.wobook.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.widget.Toast
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.runtime.compositionLocalOf
import dev.wochap.wobook.data.AppRepository
import dev.wochap.wobook.data.userMessage
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull

val LocalSnackbar = compositionLocalOf { SnackbarHostState() }

fun openUrl(context: Context, url: String) {
    val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    runCatching { context.startActivity(intent) }
        .onFailure { Toast.makeText(context, "No app can open this link", Toast.LENGTH_SHORT).show() }
}

fun copyUrl(context: Context, url: String) {
    val cm = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
    cm.setPrimaryClip(ClipData.newPlainText("URL", url))
    // Android 13+ shows its own clipboard confirmation.
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
        Toast.makeText(context, "Link copied", Toast.LENGTH_SHORT).show()
    }
}

fun copyText(context: Context, label: String, text: String) {
    val cm = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
    cm.setPrimaryClip(ClipData.newPlainText(label, text))
}

fun shareUrl(context: Context, url: String, title: String) {
    val send = Intent(Intent.ACTION_SEND).apply {
        type = "text/plain"
        putExtra(Intent.EXTRA_TEXT, url)
        if (title.isNotBlank()) putExtra(Intent.EXTRA_SUBJECT, title)
    }
    context.startActivity(Intent.createChooser(send, null).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
}

/** Tombstones now, then offers Undo for 6 s (never a dialog). */
fun deleteWithUndo(scope: CoroutineScope, repo: AppRepository, snackbar: SnackbarHostState, url: String, title: String) =
    deleteWithUndo(scope, repo, snackbar, listOf(url to title))

/** Tombstones each (url, title) in order; Undo restores only the ones that were deleted. */
fun deleteWithUndo(scope: CoroutineScope, repo: AppRepository, snackbar: SnackbarHostState, items: List<Pair<String, String>>) {
    if (items.isEmpty()) return
    scope.launch {
        val deleted = mutableListOf<Pair<String, String>>()
        var failure: Exception? = null
        for (item in items) {
            try {
                repo.delete(item.first)
                deleted += item
            } catch (e: Exception) {
                failure = failure ?: e
            }
        }
        if (deleted.isEmpty()) {
            failure?.let { snackbar.showSnackbar(it.userMessage()) }
            return@launch
        }
        val message = when {
            items.size == 1 -> "Deleted “${items[0].second.ifBlank { items[0].first }}”"
            deleted.size == items.size -> "Deleted ${items.size} bookmarks"
            else -> "Deleted ${deleted.size} of ${items.size} bookmarks"
        }
        val result = withTimeoutOrNull(6_000) {
            snackbar.showSnackbar(message, actionLabel = "Undo", duration = SnackbarDuration.Indefinite)
        }
        if (result == null) snackbar.currentSnackbarData?.dismiss()
        if (result == SnackbarResult.ActionPerformed) {
            for ((url, _) in deleted) runCatching { repo.restore(url) }
        }
    }
}

/** A validated internet-capable network is up. */
fun isOnline(context: Context): Boolean {
    val cm = context.getSystemService(Context.CONNECTIVITY_SERVICE) as android.net.ConnectivityManager
    val caps = cm.getNetworkCapabilities(cm.activeNetwork) ?: return false
    return caps.hasCapability(android.net.NetworkCapabilities.NET_CAPABILITY_VALIDATED)
}
