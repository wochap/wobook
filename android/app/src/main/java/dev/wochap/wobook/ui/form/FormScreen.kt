package dev.wochap.wobook.ui.form

import android.content.Intent
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import com.adamglin.PhosphorIcons
import com.adamglin.phosphoricons.Regular
import com.adamglin.phosphoricons.regular.BookmarkSimple
import com.adamglin.phosphoricons.regular.X
import dev.wochap.wobook.data.AppRepository
import dev.wochap.wobook.data.userMessage
import dev.wochap.wobook.domain.Formatting
import dev.wochap.wobook.ffi.Bookmark
import dev.wochap.wobook.ffi.UpdateRequest
import dev.wochap.wobook.ffi.normalizeUrl
import dev.wochap.wobook.ffi.parseTags
import dev.wochap.wobook.ui.LocalSnackbar
import dev.wochap.wobook.ui.components.AppBar
import dev.wochap.wobook.ui.components.ButtonTone
import dev.wochap.wobook.ui.components.ContentColumn
import dev.wochap.wobook.ui.components.FetchState
import dev.wochap.wobook.ui.components.FetchStateNote
import dev.wochap.wobook.ui.components.LabeledField
import dev.wochap.wobook.ui.components.ShimmerBar
import dev.wochap.wobook.ui.components.TagEditor
import dev.wochap.wobook.ui.components.TagHelper
import dev.wochap.wobook.ui.components.TagSuggestions
import dev.wochap.wobook.ui.components.UrlField
import dev.wochap.wobook.ui.components.WbButton
import dev.wochap.wobook.ui.components.WbTextButton
import dev.wochap.wobook.ui.deleteWithUndo
import dev.wochap.wobook.ui.isOnline
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/** Form entry: from Home (empty or prefilled URL), Edit, or the share sheet's "More". */
data class FormArgs(
    val url: String = "",
    val title: String = "",
    val description: String = "",
    val tags: List<String> = emptyList(),
    val edit: Boolean = false,
) {
    fun toIntent(intent: Intent): Intent = intent
        .putExtra(EXTRA_URL, url).putExtra(EXTRA_TITLE, title)
        .putExtra(EXTRA_DESCRIPTION, description).putExtra(EXTRA_TAGS, tags.toTypedArray())

    companion object {
        const val EXTRA_URL = "dev.wochap.wobook.form.URL"
        const val EXTRA_TITLE = "dev.wochap.wobook.form.TITLE"
        const val EXTRA_DESCRIPTION = "dev.wochap.wobook.form.DESCRIPTION"
        const val EXTRA_TAGS = "dev.wochap.wobook.form.TAGS"

        fun fromIntent(intent: Intent?): FormArgs? {
            val url = intent?.getStringExtra(EXTRA_URL) ?: return null
            return FormArgs(
                url = url,
                title = intent.getStringExtra(EXTRA_TITLE).orEmpty(),
                description = intent.getStringExtra(EXTRA_DESCRIPTION).orEmpty(),
                tags = intent.getStringArrayExtra(EXTRA_TAGS)?.toList().orEmpty(),
            )
        }
    }
}

private fun normalizeOrNull(url: String): String? = runCatching { normalizeUrl(url.trim()) }.getOrNull()

@Composable
fun FormScreen(
    repo: AppRepository,
    args: FormArgs,
    autoFetch: Boolean,
    onClose: () -> Unit,
    onSaved: (String) -> Unit,
    onDeleted: () -> Unit,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val snackbar = LocalSnackbar.current

    var url by remember { mutableStateOf(args.url) }
    var title by remember { mutableStateOf(args.title) }
    var description by remember { mutableStateOf(args.description) }
    var chips by remember { mutableStateOf(args.tags) }
    var pending by remember { mutableStateOf("") }
    var existing by remember { mutableStateOf<Bookmark?>(null) }
    var cameFromAdd by remember { mutableStateOf(!args.edit) }
    var changingUrl by remember { mutableStateOf(false) }
    var fetch by remember { mutableStateOf(FetchState.None) }
    var fetchJob by remember { mutableStateOf<Job?>(null) }
    var known by remember { mutableStateOf<List<Pair<String, Long>>>(emptyList()) }
    var saving by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    val tagFocus = remember { FocusRequester() }
    val urlFocus = remember { FocusRequester() }

    fun runFetch(target: String) {
        fetchJob?.cancel()
        if (!isOnline(context)) {
            fetch = FetchState.Offline
            return
        }
        fetch = FetchState.Fetching
        fetchJob = scope.launch {
            fetch = try {
                val meta = repo.fetchMetadata(target)
                if (title.isBlank()) title = meta.title.orEmpty()
                if (description.isBlank()) description = meta.description.orEmpty()
                FetchState.Fetched
            } catch (e: Exception) {
                FetchState.Failed
            }
        }
    }

    LaunchedEffect(Unit) {
        known = runCatching { repo.tags().map { it.tag to it.count } }.getOrDefault(emptyList())
        if (args.url.isBlank()) runCatching { urlFocus.requestFocus() }
    }

    // URL is the identity: an existing live URL switches the form to Edit.
    LaunchedEffect(url) {
        if (existing != null) return@LaunchedEffect
        delay(400)
        val normalized = normalizeOrNull(url) ?: return@LaunchedEffect
        val found = runCatching { repo.get(normalized) }.getOrNull()
        if (found != null && !found.deleted) {
            fetchJob?.cancel()
            fetch = FetchState.None
            existing = found
            url = found.url
            if (title.isBlank()) title = found.title
            if (description.isBlank()) description = found.description
            chips = (found.tags + chips).distinct()
            if (args.edit) cameFromAdd = false
        } else if (autoFetch && title.isBlank() && fetch == FetchState.None) {
            runFetch(normalized)
        }
    }

    val editing = existing != null
    fun save() {
        if (saving) return
        val committed = parseTags(pending)
        val tags = (chips + committed).distinct()
        saving = true
        error = null
        scope.launch {
            try {
                val target = normalizeUrl(url.trim())
                val old = existing
                val savedUrl = if (old == null) {
                    repo.add(target, title.ifBlank { null }, description.ifBlank { null }, tags, fetch = false, merge = true).bookmark.url
                } else {
                    val current = if (target != old.url) repo.rename(old.url, target).url else old.url
                    repo.update(UpdateRequest(current, title, description, tags, null, null)).url
                }
                onSaved(savedUrl)
            } catch (e: Exception) {
                error = e.userMessage()
                saving = false
            }
        }
    }

    val frame = when {
        editing -> "form-edit"
        fetch == FetchState.Fetching -> "form-fetching"
        fetch == FetchState.Fetched -> "form-fetched"
        fetch == FetchState.Failed -> "form-failed"
        else -> "form"
    }

    ContentColumn(Modifier.statusBarsPadding().navigationBarsPadding().imePadding().testTag(frame)) {
        AppBar(if (editing) "Edit bookmark" else "Add bookmark", PhosphorIcons.Regular.X, onClose, navigationLabel = "Close") {
            WbTextButton("Save", ::save, Modifier.testTag("form-save-top"))
        }
        Column(
            Modifier.weight(1f).fillMaxWidth().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            val old = existing
            if (old != null && cameFromAdd) {
                Row(
                    Modifier.fillMaxWidth().background(MaterialTheme.colorScheme.primaryContainer, MaterialTheme.shapes.small).padding(12.dp).testTag("form-banner"),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Icon(PhosphorIcons.Regular.BookmarkSimple, null, Modifier.size(18.dp), tint = MaterialTheme.colorScheme.primary)
                    Text(
                        buildAnnotatedString {
                            append("Already saved on ")
                            withStyle(SpanStyle(fontWeight = FontWeight.Medium)) { append(Formatting.date(old.createdMs)) }
                            append(", editing it")
                        },
                        style = MaterialTheme.typography.bodyMedium,
                    )
                }
            }
            if (old != null) {
                UrlField(url, { url = it }, changingUrl, onChange = { changingUrl = true }, readOnlyLabel = true)
            } else {
                LabeledField(
                    "URL", url, { url = it; if (fetch != FetchState.Fetching) fetch = FetchState.None },
                    mono = true, focusRequester = urlFocus, tag = "form-url", placeholder = "https://",
                )
            }
            if (fetch == FetchState.Fetching) {
                LabeledField("Title", "", {}, body = { ShimmerBar(0.7f) })
                LabeledField("Description", "", {}, body = { ShimmerBar(0.92f) })
            } else {
                LabeledField("Title", title, { title = it }, tag = "form-title")
                LabeledField(
                    "Description", description, { description = it }, singleLine = false,
                    placeholder = "Optional", tag = "form-description", imeAction = ImeAction.Default,
                )
            }
            when (fetch) {
                FetchState.Fetching -> FetchStateNote(fetch, "Fetching page details…")
                FetchState.Fetched -> FetchStateNote(fetch, "Filled from the page · edit anything")
                FetchState.Failed -> FetchStateNote(fetch, "Couldn't reach the page — fill in the title yourself") {
                    normalizeOrNull(url)?.let(::runFetch)
                }
                FetchState.Offline -> FetchStateNote(fetch, "Offline — saved on this device, syncs later")
                FetchState.None -> Unit
            }
            TagEditor(
                chips = chips, onChipsChange = { chips = it },
                pending = pending, onPendingChange = { pending = it },
                normalize = { parseTags(it) },
                focusRequester = tagFocus,
            )
            TagSuggestions(pending, known, chips, onPick = { chips = chips + it; pending = "" })
            TagHelper()
            error?.let { Text(it, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.error, modifier = Modifier.testTag("form-error")) }
            if (old != null) {
                WbButton(
                    "Delete",
                    {
                        deleteWithUndo(scope, repo, snackbar, old.url, old.title)
                        onDeleted()
                    },
                    Modifier.testTag("form-delete"),
                    tone = ButtonTone.Destructive,
                    height = 40.dp,
                )
            }
        }
        WbButton(
            "Save", ::save,
            Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp).testTag("form-save"),
            enabled = !saving && url.isNotBlank(),
        )
    }
}
