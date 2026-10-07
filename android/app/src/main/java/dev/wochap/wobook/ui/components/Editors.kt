package dev.wochap.wobook.ui.components

import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import dev.wochap.wobook.domain.TagRules
import dev.wochap.wobook.ui.theme.Dimens
import dev.wochap.wobook.ui.theme.Wb
import dev.wochap.wobook.ui.theme.WobookType

/** Zero-width sentinel so a backspace on "empty" input is observable. */
private const val SENTINEL = "​"

/**
 * Chip editor (design D5): comma or Enter commits, space is part of the tag,
 * pasted text splits on commas, backspace on empty input re-opens the last
 * chip. [normalize] is the FFI `parse_tags`.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun TagEditor(
    chips: List<String>,
    onChipsChange: (List<String>) -> Unit,
    pending: String,
    onPendingChange: (String) -> Unit,
    normalize: (String) -> List<String>,
    modifier: Modifier = Modifier,
    focusRequester: FocusRequester = remember { FocusRequester() },
    background: Color = MaterialTheme.colorScheme.background,
    onDone: () -> Unit = {},
) {
    val scheme = MaterialTheme.colorScheme
    var focused by remember { mutableStateOf(false) }
    val shape = MaterialTheme.shapes.small
    val field = TextFieldValue(SENTINEL + pending, selection = TextRange(SENTINEL.length + pending.length))

    fun commit(tags: List<String>) {
        val normalized = tags.flatMap { normalize(it) }
        if (normalized.isNotEmpty()) onChipsChange(TagRules.merge(chips, normalized))
    }

    Column(
        modifier.fillMaxWidth().heightIn(min = Dimens.field).clip(shape).background(background)
            .border(1.dp, if (focused) scheme.primary else scheme.outlineVariant, shape)
            .padding(start = 14.dp, end = 14.dp, top = 6.dp, bottom = 8.dp)
            .testTag("tag-editor"),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Text("Tags", style = MaterialTheme.typography.labelMedium, color = if (focused) scheme.primary else scheme.onSurfaceVariant)
        FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            chips.forEach { tag -> RemovableChip(tag, onRemove = { onChipsChange(chips - tag) }) }
            Box(Modifier.widthIn(min = 80.dp).heightIn(min = 28.dp), contentAlignment = Alignment.CenterStart) {
                if (pending.isEmpty() && chips.isEmpty()) {
                    Text("Add tag", style = MaterialTheme.typography.bodyLarge, color = Wb.colors.textFaint)
                }
                BasicTextField(
                    value = field,
                    onValueChange = { v ->
                        if (!v.text.startsWith(SENTINEL)) {
                            // Backspace on empty input: re-open the last chip.
                            val popped = TagRules.onBackspace(chips, pending)
                            if (popped != null) {
                                onChipsChange(popped.first)
                                onPendingChange(popped.second)
                            } else {
                                onPendingChange(v.text.replace(SENTINEL, ""))
                            }
                            return@BasicTextField
                        }
                        val edit = TagRules.onInput(v.text.removePrefix(SENTINEL).replace(SENTINEL, ""))
                        commit(edit.committed)
                        onPendingChange(edit.pending)
                    },
                    singleLine = true,
                    textStyle = MaterialTheme.typography.bodyLarge.copy(color = scheme.onSurface),
                    cursorBrush = SolidColor(scheme.primary),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done, capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false),
                    keyboardActions = KeyboardActions(onDone = {
                        if (pending.isBlank()) {
                            onDone()
                        } else {
                            commit(TagRules.onEnter(pending).committed)
                            onPendingChange("")
                        }
                    }),
                    modifier = Modifier.widthIn(min = 80.dp).focusRequester(focusRequester)
                        .onFocusChanged { focused = it.isFocused }.testTag("tag-input"),
                )
            }
        }
    }
}

/** Suggestions row under the editor: matched part highlighted, most used first, max 8. */
@Composable
fun TagSuggestions(
    query: String,
    known: List<Pair<String, Long>>,
    chosen: List<String>,
    onPick: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val list = TagRules.suggestions(query, known, chosen)
    if (list.isEmpty()) return
    Row(
        modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).testTag("tag-suggestions"),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        list.forEach { tag -> SuggestionChip(tag, TagRules.matchRange(tag, query)) { onPick(tag) } }
    }
}

/** Helper line under editors. */
@Composable
fun TagHelper() {
    Text(
        "Comma or Enter adds a tag · spaces are allowed",
        style = MaterialTheme.typography.labelMedium,
        color = Wb.colors.textFaint,
        modifier = Modifier.padding(start = 14.dp),
    )
}

/**
 * URL in Edit mode: read-only (surface, muted mono) with "Change"; when
 * [editing] it renders an editable field and the rename note.
 */
@Composable
fun UrlField(
    url: String,
    onUrlChange: (String) -> Unit,
    editing: Boolean,
    onChange: () -> Unit,
    readOnlyLabel: Boolean,
) {
    val scheme = MaterialTheme.colorScheme
    if (!editing) {
        Row(
            Modifier.fillMaxWidth().heightIn(min = Dimens.field).clip(MaterialTheme.shapes.small)
                .background(scheme.surfaceContainer).padding(start = 14.dp, end = 4.dp, top = 6.dp, bottom = 6.dp)
                .testTag("url-readonly"),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(if (readOnlyLabel) "URL · read-only" else "URL", style = MaterialTheme.typography.labelMedium, color = scheme.onSurfaceVariant)
                Text(url, style = WobookType.mono, color = scheme.onSurfaceVariant, maxLines = 2)
            }
            WbTextButton("Change", onChange, Modifier.testTag("url-change"))
        }
    } else {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            LabeledField("URL", url, onUrlChange, mono = true, tag = "url-input", imeAction = ImeAction.Next)
            InfoNote("Changing the URL replaces this bookmark; tags and description are kept.")
        }
    }
}
