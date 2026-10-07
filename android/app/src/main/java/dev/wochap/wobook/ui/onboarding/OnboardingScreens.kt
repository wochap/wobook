package dev.wochap.wobook.ui.onboarding

import android.os.Build
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
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
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import com.adamglin.PhosphorIcons
import com.adamglin.phosphoricons.Regular
import com.adamglin.phosphoricons.regular.ArrowRight
import com.adamglin.phosphoricons.regular.BookmarkSimple
import com.adamglin.phosphoricons.regular.QrCode
import dev.wochap.wobook.data.AppRepository
import dev.wochap.wobook.data.Settings
import dev.wochap.wobook.data.userMessage
import dev.wochap.wobook.ui.components.ContentColumn
import dev.wochap.wobook.ui.components.LabeledField
import dev.wochap.wobook.ui.components.WbButton
import dev.wochap.wobook.ui.theme.Wb
import kotlinx.coroutines.launch

@Composable
private fun Brand() {
    Text("wobook", style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.primary)
}

/** First launch, screen 1: name this device (prefilled with the model). */
@Composable
fun OnboardingNameScreen(repo: AppRepository, onContinue: () -> Unit) {
    val scope = rememberCoroutineScope()
    var name by remember { mutableStateOf(Build.MODEL.orEmpty().lowercase().replace(' ', '-')) }
    var error by remember { mutableStateOf<String?>(null) }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    fun next() {
        scope.launch {
            runCatching { repo.setDeviceName(name) }
                .onSuccess { onContinue() }
                .onFailure { error = it.userMessage() }
        }
    }
    ContentColumn(Modifier.statusBarsPadding().navigationBarsPadding().imePadding().testTag("onb-1")) {
        Column(Modifier.weight(1f).fillMaxWidth().padding(24.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Brand()
            Spacer(Modifier.height(16.dp))
            Text("Name this device", style = MaterialTheme.typography.headlineMedium)
            Text(
                "Other devices will see it when you pair and in their sync history.",
                style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            LabeledField("Device name", name, { name = it; error = null }, focusRequester = focus, imeAction = ImeAction.Done, onDone = ::next, tag = "onb-name")
            error?.let { Text(it, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.error) }
        }
        Row(Modifier.fillMaxWidth().padding(16.dp), horizontalArrangement = Arrangement.End) {
            WbButton("Continue", ::next, Modifier.testTag("onb-continue"), icon = null, enabled = name.isNotBlank())
        }
    }
}

/** First launch, screen 2: pair or start fresh. */
@Composable
fun OnboardingChooseScreen(repo: AppRepository, settings: Settings, onPair: () -> Unit, onFresh: () -> Unit) {
    val scope = rememberCoroutineScope()
    var name by remember { mutableStateOf("") }
    val keyboard = LocalSoftwareKeyboardController.current
    LaunchedEffect(Unit) { keyboard?.hide() }
    LaunchedEffect(Unit) { name = runCatching { repo.thisDevice().name }.getOrDefault("") }
    ContentColumn(Modifier.statusBarsPadding().navigationBarsPadding().testTag("onb-2")) {
        Column(Modifier.weight(1f).fillMaxWidth().padding(24.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Brand()
            Spacer(Modifier.height(16.dp))
            Text("Already have bookmarks somewhere?", style = MaterialTheme.typography.headlineMedium)
            Text(
                "Pair with a desktop or another phone and this one fills up over the local network. No account, nothing leaves your devices.",
                style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.height(8.dp))
            ChoiceRow(PhosphorIcons.Regular.QrCode, "Pair with an existing device", "Scan the QR code it shows you", "onb-pair") {
                // Pairing completes onboarding (see WobookNav); start fresh also does.
                scope.launch { settings.setOnboardingDone(true); onPair() }
            }
            ChoiceRow(PhosphorIcons.Regular.BookmarkSimple, "Start fresh", "You can pair any time from Devices", "onb-fresh") {
                scope.launch { settings.setOnboardingDone(true); onFresh() }
            }
        }
        if (name.isNotEmpty()) {
            Text(
                "Named $name",
                style = MaterialTheme.typography.labelMedium, color = Wb.colors.textFaint,
                modifier = Modifier.padding(24.dp),
            )
        }
    }
}

@Composable
private fun ChoiceRow(icon: ImageVector, title: String, body: String, tag: String, onClick: () -> Unit) {
    val scheme = MaterialTheme.colorScheme
    Row(
        Modifier.fillMaxWidth().border(1.dp, scheme.outlineVariant, MaterialTheme.shapes.small)
            .clickable(onClick = onClick).padding(16.dp).testTag(tag),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Icon(icon, null, Modifier.size(24.dp), tint = scheme.primary)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(title, style = MaterialTheme.typography.titleMedium)
            Text(body, style = MaterialTheme.typography.bodyMedium, color = scheme.onSurfaceVariant)
        }
        Icon(PhosphorIcons.Regular.ArrowRight, null, Modifier.size(20.dp), tint = scheme.onSurfaceVariant)
    }
}
