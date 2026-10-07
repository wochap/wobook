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
fun deleteWithUndo(scope: CoroutineScope, repo: AppRepository, snackbar: SnackbarHostState, url: String, title: String) {
    scope.launch {
        try {
            repo.delete(url)
        } catch (e: Exception) {
            snackbar.showSnackbar(e.userMessage())
            return@launch
        }
        val label = title.ifBlank { url }
        val result = withTimeoutOrNull(6_000) {
            snackbar.showSnackbar("Deleted “$label”", actionLabel = "Undo", duration = SnackbarDuration.Indefinite)
        }
        if (result == null) snackbar.currentSnackbarData?.dismiss()
        if (result == SnackbarResult.ActionPerformed) {
            runCatching { repo.restore(url) }
        }
    }
}

/** A validated internet-capable network is up. */
fun isOnline(context: Context): Boolean {
    val cm = context.getSystemService(Context.CONNECTIVITY_SERVICE) as android.net.ConnectivityManager
    val caps = cm.getNetworkCapabilities(cm.activeNetwork) ?: return false
    return caps.hasCapability(android.net.NetworkCapabilities.NET_CAPABILITY_VALIDATED)
}
