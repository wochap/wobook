package dev.wochap.wobook.ui.share

import android.widget.Toast
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.core.app.NotificationManagerCompat
import com.adamglin.PhosphorIcons
import com.adamglin.phosphoricons.Regular
import com.adamglin.phosphoricons.regular.BookmarkSimple
import com.adamglin.phosphoricons.regular.CaretUp
import com.adamglin.phosphoricons.regular.Check
import dev.wochap.wobook.data.AppRepository
import dev.wochap.wobook.data.Settings
import dev.wochap.wobook.data.userMessage
import dev.wochap.wobook.domain.Formatting
import dev.wochap.wobook.domain.UrlDisplay
import dev.wochap.wobook.ffi.Bookmark
import dev.wochap.wobook.ffi.UpdateRequest
import dev.wochap.wobook.ffi.normalizeUrl
import dev.wochap.wobook.ffi.parseTags
import dev.wochap.wobook.ui.components.ButtonTone
import dev.wochap.wobook.ui.components.FetchState
import dev.wochap.wobook.ui.components.FetchStateNote
import dev.wochap.wobook.ui.components.TagEditor
import dev.wochap.wobook.ui.components.TagSuggestions
import dev.wochap.wobook.ui.components.WbButton
import dev.wochap.wobook.ui.form.FormArgs
import dev.wochap.wobook.ui.isOnline
import dev.wochap.wobook.ui.theme.Dimens
import dev.wochap.wobook.ui.theme.WobookType
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

private sealed interface Target {
    data object Loading : Target
    data class Invalid(val message: String) : Target
    data class Fresh(val url: String) : Target
    data class Saved(val bookmark: Bookmark) : Target
}

/**
 * Share-sheet receiver (design D6): fresh URL saves in one tap without
 * waiting on the network; an existing URL offers "Update tags".
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ShareSheet(
    repo: AppRepository,
    settings: Settings,
    sharedText: String,
    subject: String,
    onMore: (FormArgs) -> Unit,
    onFinish: () -> Unit,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    var target by remember { mutableStateOf<Target>(Target.Loading) }
    var title by remember { mutableStateOf("") }
    var description by remember { mutableStateOf("") }
    var chips by remember { mutableStateOf<List<String>>(emptyList()) }
    var pending by remember { mutableStateOf("") }
    var fetch by remember { mutableStateOf(FetchState.None) }
    var known by remember { mutableStateOf<List<Pair<String, Long>>>(emptyList()) }
    var savedInline by remember { mutableStateOf(false) }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    val tagFocus = remember { FocusRequester() }

    LaunchedEffect(Unit) {
        val raw = UrlDisplay.extractFirst(sharedText) ?: sharedText.trim()
        val url = runCatching { normalizeUrl(raw) }.getOrNull()
        if (url == null) {
            target = Target.Invalid("No link in the shared text")
            return@LaunchedEffect
        }
        val sub = subject.trim().takeUnless { it.isBlank() || it == raw || it == url }.orEmpty()
        val existing = runCatching { repo.get(url) }.getOrNull()
        known = runCatching { repo.tags().map { it.tag to it.count } }.getOrDefault(emptyList())
        if (existing != null && !existing.deleted) {
            target = Target.Saved(existing)
            title = existing.title
            chips = existing.tags
        } else {
            target = Target.Fresh(url)
            title = sub
            if (!isOnline(context)) {
                fetch = FetchState.Offline
            } else if (settings.current().autoFetch) {
                fetch = FetchState.Fetching
                // Background fetch; Save never waits for it.
                scope.launch {
                    fetch = try {
                        val meta = repo.fetchMetadata(url)
                        if (title.isBlank()) title = meta.title.orEmpty()
                        description = meta.description.orEmpty()
                        if (description.isBlank()) FetchState.Failed else FetchState.Fetched
                    } catch (e: Exception) {
                        FetchState.Failed
                    }
                }
            }
        }
        runCatching { tagFocus.requestFocus() }
    }

    fun allTags() = (chips + parseTags(pending)).distinct()

    fun finishSaved(tags: List<String>) {
        val text = if (tags.isEmpty()) "Saved to wobook" else "Saved to wobook · ${tags.joinToString(", ")}"
        if (NotificationManagerCompat.from(context).areNotificationsEnabled()) {
            Toast.makeText(context.applicationContext, text, Toast.LENGTH_SHORT).show()
            onFinish()
        } else {
            savedInline = true
            scope.launch {
                delay(600)
                onFinish()
            }
        }
    }

    fun save() {
        if (busy) return
        busy = true
        val tags = allTags()
        scope.launch {
            try {
                when (val t = target) {
                    is Target.Fresh -> repo.add(
                        t.url, title.ifBlank { null }, description.ifBlank { null }, tags, fetch = false, merge = true,
                    )
                    is Target.Saved -> repo.update(UpdateRequest(t.bookmark.url, null, null, null, tags, null))
                    else -> return@launch
                }
                finishSaved(tags)
            } catch (e: Exception) {
                error = e.userMessage()
                busy = false
            }
        }
    }

    fun more() {
        val url = when (val t = target) {
            is Target.Fresh -> t.url
            is Target.Saved -> t.bookmark.url
            else -> return
        }
        onMore(FormArgs(url = url, title = title, description = description, tags = allTags()))
    }

    ModalBottomSheet(
        onDismissRequest = onFinish,
        sheetState = sheetState,
        containerColor = MaterialTheme.colorScheme.surfaceContainerLow,
        contentWindowInsets = { androidx.compose.foundation.layout.WindowInsets(0) },
        modifier = Modifier.semantics { testTagsAsResourceId = true }.testTag("share-sheet"),
    ) {
        Column(
            Modifier.widthIn(max = Dimens.maxContent).fillMaxWidth().navigationBarsPadding().imePadding()
                .padding(start = 20.dp, end = 20.dp, bottom = 12.dp)
                .testTag(frameTag(target, fetch, savedInline)),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            when (val t = target) {
                Target.Loading -> Text("…", style = MaterialTheme.typography.bodyMedium)
                is Target.Invalid -> {
                    Text(t.message, style = MaterialTheme.typography.titleMedium)
                    WbButton("Close", onFinish, Modifier.fillMaxWidth(), tone = ButtonTone.Secondary)
                }
                is Target.Fresh, is Target.Saved -> {
                    val url = if (t is Target.Fresh) t.url else (t as Target.Saved).bookmark.url
                    if (t is Target.Saved) {
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.testTag("share-already")) {
                            Icon(PhosphorIcons.Regular.BookmarkSimple, null, Modifier.size(14.dp), tint = MaterialTheme.colorScheme.primary)
                            Text(
                                "Already saved on ${Formatting.date(t.bookmark.createdMs)}",
                                style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.primary,
                            )
                        }
                    }
                    Row(verticalAlignment = Alignment.Top, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                            Text(
                                title.ifBlank { UrlDisplay.format(url) },
                                style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis,
                                modifier = Modifier.testTag("share-title"),
                            )
                            Text(
                                UrlDisplay.format(url), style = WobookType.mono, color = MaterialTheme.colorScheme.onSurfaceVariant,
                                maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.testTag("share-url"),
                            )
                        }
                        Row(
                            Modifier.clickable(onClick = ::more).padding(horizontal = 10.dp, vertical = 8.dp).testTag("share-more"),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(4.dp),
                        ) {
                            Text(if (t is Target.Saved) "Edit" else "More", style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
                            Icon(PhosphorIcons.Regular.CaretUp, null, Modifier.size(14.dp), tint = MaterialTheme.colorScheme.primary)
                        }
                    }
                    when (fetch) {
                        FetchState.Fetching -> FetchStateNote(fetch, "Fetching description in the background")
                        FetchState.Fetched -> FetchStateNote(fetch, "Description fetched")
                        FetchState.Failed -> FetchStateNote(fetch, "Couldn't fetch a description — saving without one")
                        FetchState.Offline -> FetchStateNote(fetch, "Offline — saved on this device, syncs later")
                        FetchState.None -> Unit
                    }
                    TagEditor(
                        chips = chips, onChipsChange = { chips = it },
                        pending = pending, onPendingChange = { pending = it },
                        normalize = { parseTags(it) },
                        focusRequester = tagFocus,
                        onDone = ::save,
                    )
                    TagSuggestions(pending, known, chips, onPick = { chips = chips + it; pending = "" })
                    error?.let { Text(it, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.error) }
                    if (savedInline) {
                        WbButton("Saved", {}, Modifier.fillMaxWidth().testTag("share-saved"), tone = ButtonTone.Success, icon = PhosphorIcons.Regular.Check)
                    } else {
                        WbButton(
                            if (t is Target.Saved) "Update tags" else "Save", ::save,
                            Modifier.fillMaxWidth().testTag("share-save"), enabled = !busy,
                        )
                    }
                }
            }
        }
    }
}

private fun frameTag(target: Target, fetch: FetchState, savedInline: Boolean): String = when {
    savedInline -> "share-saved-inline"
    target is Target.Saved -> "share-dup"
    fetch == FetchState.Offline -> "share-offline"
    fetch == FetchState.Failed -> "share-failed"
    else -> "share-fresh"
}
