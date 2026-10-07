package dev.wochap.wobook.ui.pairing

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
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
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.adamglin.PhosphorIcons
import com.adamglin.phosphoricons.Regular
import com.adamglin.phosphoricons.regular.ArrowLeft
import com.adamglin.phosphoricons.regular.Copy
import com.adamglin.phosphoricons.regular.HandPalm
import com.adamglin.phosphoricons.regular.Scan
import com.adamglin.phosphoricons.regular.ShieldCheck
import com.adamglin.phosphoricons.regular.Timer
import com.adamglin.phosphoricons.regular.WarningCircle
import com.adamglin.phosphoricons.regular.WifiSlash
import com.adamglin.phosphoricons.regular.X
import dev.wochap.wobook.data.AppRepository
import dev.wochap.wobook.data.userMessage
import dev.wochap.wobook.domain.Formatting
import dev.wochap.wobook.ffi.PairingConfirmation
import dev.wochap.wobook.ffi.PairingEvent
import dev.wochap.wobook.ffi.PairingOffer
import dev.wochap.wobook.ui.PairingUi
import dev.wochap.wobook.ui.components.AppBar
import dev.wochap.wobook.ui.components.ButtonTone
import dev.wochap.wobook.ui.components.ContentColumn
import dev.wochap.wobook.ui.components.Fingerprint
import dev.wochap.wobook.ui.components.QrTile
import dev.wochap.wobook.ui.components.WbButton
import dev.wochap.wobook.ui.components.platformIcon
import dev.wochap.wobook.ui.copyText
import dev.wochap.wobook.ui.theme.Wb
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/** "Show my QR": 120 s window, QR on a light tile, countdown ring, copy as text. */
@Composable
fun ShowQrScreen(repo: AppRepository, onBack: () -> Unit) {
    val context = LocalContext.current
    var offer by remember { mutableStateOf<PairingOffer?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
    var generation by remember { mutableStateOf(0) }
    LaunchedEffect(generation) {
        offer = null
        error = null
        runCatching { repo.startPairingOffer() }.onSuccess { offer = it }.onFailure { error = it.userMessage() }
        while (true) {
            now = System.currentTimeMillis()
            delay(250)
        }
    }
    val o = offer
    val remaining = if (o == null) 0L else (o.expiresAtMs - now).coerceAtLeast(0)
    val expired = o != null && remaining == 0L

    ContentColumn(Modifier.statusBarsPadding().navigationBarsPadding().testTag(if (expired) "pair-expired" else "pair-qr")) {
        AppBar("My pairing code", PhosphorIcons.Regular.ArrowLeft, onBack)
        Column(
            Modifier.weight(1f).fillMaxWidth().verticalScroll(rememberScrollState()).padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            when {
                error != null -> Text(error!!, color = MaterialTheme.colorScheme.error)
                o == null -> Unit
                expired -> {
                    StateBody(PhosphorIcons.Regular.Timer, "This code has expired", "Pairing codes are valid for two minutes. Show a fresh one and scan it again.")
                    WbButton("Show again", { generation++ }, Modifier.testTag("qr-again"), icon = PhosphorIcons.Regular.Scan)
                }
                else -> {
                    QrTile(o.qrPayloadJson, 248.dp, progress = remaining / 120_000f)
                    Text(o.deviceName, style = MaterialTheme.typography.titleMedium, modifier = Modifier.testTag("qr-device"))
                    Text(
                        "Code expires in ${Formatting.countdown(remaining / 1000)}",
                        style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.testTag("qr-countdown"),
                    )
                    WbButton(
                        "Copy as text", { copyText(context, "wobook pairing code", o.qrPayloadJson) },
                        Modifier.testTag("qr-copy"), tone = ButtonTone.Secondary, icon = PhosphorIcons.Regular.Copy,
                    )
                    Text(
                        "Paste it into “Paste code instead” on the other phone",
                        style = MaterialTheme.typography.labelMedium, color = Wb.colors.textFaint, textAlign = TextAlign.Center,
                    )
                }
            }
        }
    }
}

/** Fingerprint confirmation: both devices show the same four groups. */
@Composable
fun ConfirmScreen(repo: AppRepository, pairing: PairingUi, id: String, onRejected: () -> Unit) {
    val scope = rememberCoroutineScope()
    var confirmation by remember { mutableStateOf<PairingConfirmation?>(pairing.confirmations.firstOrNull { it.id == id }) }
    var waiting by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(id) {
        if (confirmation == null) {
            confirmation = runCatching { repo.pendingConfirmations().firstOrNull { it.id == id } }.getOrNull()
        }
    }
    val c = confirmation ?: return
    ContentColumn(Modifier.statusBarsPadding().navigationBarsPadding().testTag("pair-confirm")) {
        AppBar("Confirm device", null, {})
        Column(
            Modifier.weight(1f).fillMaxWidth().verticalScroll(rememberScrollState()).padding(horizontal = 24.dp, vertical = 16.dp),
            verticalArrangement = Arrangement.spacedBy(20.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                Icon(platformIcon(c.peerPlatform), null, Modifier.size(40.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                Column {
                    Text(c.peerName, style = MaterialTheme.typography.titleLarge, modifier = Modifier.testTag("confirm-peer"))
                    Text("${Formatting.platformLabel(c.peerPlatform)} · wants to pair", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("Fingerprint", style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Fingerprint(c.fingerprintGroups)
            }
            Text(
                "${c.peerName} shows the same four groups. If they differ, someone on the network is in the middle — reject.",
                style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            if (waiting) Text("Waiting for ${c.peerName} to confirm…", style = MaterialTheme.typography.labelMedium, color = Wb.colors.textFaint, modifier = Modifier.testTag("confirm-waiting"))
            error?.let { Text(it, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.error) }
        }
        Row(Modifier.fillMaxWidth().padding(16.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            WbButton(
                "Reject",
                {
                    scope.launch {
                        runCatching { repo.confirmPairing(c.id, false) }
                        pairing.confirmations.removeAll { it.id == c.id }
                        onRejected()
                    }
                },
                Modifier.weight(1f).testTag("confirm-reject"), tone = ButtonTone.Secondary, enabled = !waiting,
            )
            WbButton(
                "Trust",
                {
                    waiting = true
                    scope.launch {
                        runCatching { repo.confirmPairing(c.id, true) }.onFailure { error = it.userMessage(); waiting = false }
                    }
                },
                Modifier.weight(1f).testTag("confirm-trust"), icon = PhosphorIcons.Regular.ShieldCheck, enabled = !waiting,
            )
        }
    }
}

/** Expired / Rejected / Unreachable / Failed, mapped 1:1 from `PairingEvent`. */
@Composable
fun PairingResultScreen(pairing: PairingUi, onClose: () -> Unit, onScanAgain: () -> Unit) {
    val event = pairing.result
    val peer = pairing.peerName ?: "the other device"
    val tag = when (event) {
        is PairingEvent.Expired -> "pair-expired"
        is PairingEvent.Rejected -> "pair-rejected"
        is PairingEvent.Unreachable -> "pair-unreachable"
        else -> "pair-failed"
    }
    ContentColumn(Modifier.statusBarsPadding().navigationBarsPadding().testTag(tag)) {
        AppBar("Pairing", PhosphorIcons.Regular.X, onClose, navigationLabel = "Close")
        Column(Modifier.weight(1f).fillMaxWidth().padding(24.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            when (event) {
                is PairingEvent.Expired -> {
                    StateBody(PhosphorIcons.Regular.Timer, "This code has expired", "Pairing codes are valid for two minutes. Ask $peer to show a fresh one and scan again.")
                    Row { WbButton("Scan again", onScanAgain, Modifier.testTag("result-scan-again"), icon = PhosphorIcons.Regular.Scan, height = 40.dp) }
                }
                is PairingEvent.Rejected -> {
                    StateBody(PhosphorIcons.Regular.HandPalm, "$peer rejected the pairing", "Nothing was shared. If you didn't expect this, check that the fingerprints matched on both screens.")
                    Row { WbButton("Back to Devices", onClose, Modifier.testTag("result-devices"), tone = ButtonTone.Secondary, height = 40.dp) }
                }
                is PairingEvent.Unreachable -> {
                    val tried = event.tried.map { it.substringBeforeLast(':').trim('[', ']') }
                    val list = when (tried.size) {
                        0 -> "every address in the code"
                        1 -> tried[0]
                        else -> tried.dropLast(1).joinToString(", ") + " and " + tried.last()
                    }
                    StateBody(PhosphorIcons.Regular.WifiSlash, "Can't reach $peer", "Tried $list. Both devices need to be on the same Wi-Fi or in the same tailnet.")
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        WbButton("Retry", {
                            pairing.retry = true
                            onScanAgain()
                        }, Modifier.testTag("result-retry"), height = 40.dp)
                        WbButton("Cancel", onClose, Modifier.testTag("result-cancel"), tone = ButtonTone.Secondary, height = 40.dp)
                    }
                }
                is PairingEvent.Failed -> {
                    StateBody(PhosphorIcons.Regular.WarningCircle, "Pairing failed", "Something went wrong (${event.message}). Nothing was stored; try again.")
                    Row { WbButton("Scan again", onScanAgain, height = 40.dp) }
                }
                else -> Unit
            }
        }
    }
}

@Composable
private fun StateBody(icon: ImageVector, title: String, body: String) {
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Icon(icon, null, Modifier.size(32.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
        Text(title, style = MaterialTheme.typography.titleLarge)
        Text(body, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.height(4.dp))
    }
}
